# Long-term semantic memory (pr-memory)

An unlimited agent knowledge base in SQLite, built after the
**mem0 / Memora** model. Complements the
small file-based memory `MEMORY.md`/`USER.md` (see [MEMORY-SKILLS.md](MEMORY-SKILLS.md)):
file-based memory is a stable "profile" of ~2 KB in the prompt, semantic memory is
a searchable knowledge archive.

The two tiers solve different problems. The file-backed store is a compact,
human-editable identity/profile that is *always* in the prompt — cheap to read,
bounded by a character budget. The semantic store is the opposite: it is
**unbounded** (an archive that grows with every session), never read in full,
and queried on demand through hybrid search. A research agent that runs many
sessions accumulates far more facts than could ever fit a prompt, and most of
them are only relevant in a specific context — that is exactly the regime where
a searchable SQLite archive beats a fixed profile file.

Everything lives in a single SQLite database (default
`~/.fathom/memory.db`, overridable via the `PR_MEMORY_DB` environment
variable). The subsystem is exposed as the shared `Memory` struct — a handle
holding the store (`MemoryDb`), the embedding backend (`Arc<dyn Embedder>`) and
the `[memory]` configuration — which is shared across the coordinator, agent
runtimes, tools and the HTTP server, so every entry point talks to the same
store and the same embedder.

## What is stored

Each record is a **self-contained fact** (50–500 characters recommended;
hard bounds are 3–5000 enforced at write time, and facts shorter than
20 characters are flagged as "too generic" — they break dedup and retrieval):

| field | purpose |
|------|-----------|
| `content` | fact text |
| `scope` / `scope_key` | isolation: `agent` (general knowledge), `user` (about the user/client), `run` (session episode) |
| `source` | origin: `session:<id>`, URL, `user` |
| `confidence` | 0.0–1.0, tie-breaker on conflicts |
| `importance` | weight, grows via `memory_boost` |
| `tags` / `metadata` | typing (`contact`, `session-summary`, `todo`, ...) |
| `status` | `active` / `superseded` / `archived` |

On top: FTS5 index (BM25), embeddings, typed edges between records
(`supersedes`, `contradicts`, `related_to`, `implements`, `extends`,
`references`) and a history journal of all changes.

The storage layer is deliberately layered so that each access pattern has a
dedicated structure:

- **Rows** — the append-only facts themselves, each with a UUIDv7 id (the
  timestamp prefix means ids are roughly time-ordered; tools display the last
  8 characters as a short id and the store resolves them back via suffix
  matching).
- **FTS5 keyword index** — drives the BM25 half of hybrid search; the free-form
  query is tokenized, each term quoted and OR-joined (any term may hit).
- **Embeddings** — one dense vector per fact, tagged with the producing model
  so vectors from different models are never compared.
- **`memory_edges`** — typed links between memories (supersession chains,
  contradictions, generic relatedness).
- **Audit history** — every change is journaled, so at any moment you can
  answer *how* a fact evolved, not just what it currently says.

## Key principles (from mem0/Memora)

1. **Append-only.** Facts are never overwritten. A new version = a new record +
   a `supersedes` edge to the outdated one; contradictions stay visible from both
   sides via the `contradicts` edge. Read modes by id: `active`,
   `latest` (resolve the chain to the newest), `full_history` (the whole evolution).

   This is the core design decision behind the whole store: an archive must
   never lose information, because a research agent cannot know in advance which
   detail of a superseded fact will turn out to matter. Reading resolves
   supersession chains on demand: `follow=active` returns the row itself if it is
   still active, `follow=latest` walks `supersedes` edges *forward* to the newest
   version, and `follow=full_history` resolves the requested id back to the chain
   root and returns the entire evolution oldest → newest. When two absorbed
   facts genuinely disagree (e.g. a company changed its CEO), both stay alive and
   the `contradicts` edge keeps them visible from either side; confidence and
   recency break the tie downstream in ranking.

2. **Absorb instead of create.** Write pipeline:
   validation → **secret detection** (API keys, tokens, PEM — rejected) →
   consolidation of close facts in a batch (N→1, threshold 0.85) → dedup by hash →
   candidate search by cosine → classification (LLM if available, otherwise
   heuristic by threshold 0.97) → one of 6 outcomes:
   `duplicate` (skip) / `supersede` / `contradict` / `coexist` / `related` / `new`.
   There is `dry_run` — a plan without writing.

   The full pipeline, stage by stage:

   - **Validation** — enforces the hard size bounds and warns on too-generic
     facts (a standalone fact, not "went well" fragments).
   - **Secret detection** — scans for API keys, tokens, PEM blocks and similar
     patterns with a regex table; any hit rejects the record at write time.
     Research agents routinely see credentials in fetched pages, config files
     and shell output — if such text were absorbed it would persist and later
     leak into prompts, digests and exports.
   - **Batch consolidation (N→1)** — facts closer than cosine 0.85 to each other
     within one batch are merged into a single record before anything else.
   - **Dedup by hash** — a stable, dependency-free content hash gives a cheap
     pre-LLM exact-duplicate check.
   - **Candidate search** — the embedded fact is compared by cosine against
     stored rows; the top candidates (up to 5, threshold 0.55) are presented to
     classification. The text embedded for comparison includes tags/metadata, so
     typing influences the vector.
   - **Classification** — the LLM (when `llm_classify` is on and an LLM is
     available) returns a JSON verdict per candidate, parsed out of possibly
     chatty model output; without an LLM, a heuristic applies at the 0.97
     similarity threshold.
   - **Apply verdict** — each outcome maps to a specific write (or no-write):
     `duplicate` skips, `supersede` creates a new row + `supersedes` edge,
     `contradict` keeps both versions with a `contradicts` edge, `coexist` keeps
     both when they apply to different contexts, `related` adds a `related_to`
     edge, `new` stores the fact standalone. Every write also appends to the
     history journal.

   `dry_run` runs the whole pipeline and returns the plan (which verdict each
   fact would get) without touching the database — useful for auditing and
   testing before going live.

3. **Hybrid search.** `score = w·cosine + (1−w)·BM25` (default `w=0.7`),
   then linear freshness decay `score × max(0, 1 − decay·days)`
   (default `decay=0.01/day`), then a gentle boost for reinforcement
   (`min(access_count/10, 1.0)`) and confidence — the formula:
   `score × (0.8 + 0.1·reinforcement + 0.1·confidence)`.

   Two independent signals are fused: the dense cosine similarity (semantic
   paraphrase matching) and the sparse BM25 keyword score from the FTS5 index
   (exact term hits, good for names, ids, URLs). The default weight `w=0.7`
   slightly favors semantics, but BM25 still rescues queries with precise
   vocabulary. The freshness term linearly discounts older facts so stale
   knowledge loses ground to recent discoveries, and the final multiplier bakes
   in reinforcement (how often a fact was retrieved — `access_count`) and
   confidence. All three post-factors are gentle by construction, so they
   refine, never dominate, the relevance ordering.

4. **Digest before start.** The top agent (depth 0) receives a deterministic
   digest in the system prompt: relevant memories + open TODOs +
   recent records — with real ids for verification via `memory_search`.

   The digest is a **deterministic aggregator**, not generated prose: one call
   returns buckets of *real* memories (with real short ids the agent can verify
   via `memory_search`), open TODOs and recent records, rendered under a
   `## Long-term memory digest (topic: …)` header. Because it uses real ids,
   the agent can trust what it sees and pull more detail on demand instead of
   re-embedding or re-searching blindly.

5. **Rerank (optional).** `[memory] rerank = true`: a second LLM pass
   reorders the expanded results (top_k×3) by relevance.

   The fused score is lexical/vector-based; an LLM can judge topical relevance
   better on a short list. One extra LLM call asks for a JSON verdict
   (`{"order": [2, 0, 1]}` — candidate indexes most → least relevant) and the
   hits are reordered accordingly. Any failure — parse error, network hiccup,
   missing model — returns the original order unchanged: search must never
   break because a rerank call failed.

### Digest in the agent prompt

How the digest actually reaches the model, in the agent runtime:

- **Only top-level agents (depth 0)** get a digest block (gated on
  `[memory] auto_digest`); child agents receive an explicit context handoff
  from their parent instead. A per-sub-agent digest would multiply embedding
  calls in a fleet that already fans out to dozens of workers, so the cost is
  paid exactly once per session.
- The digest is injected as a **volatile tier** of the system prompt, alongside
  role/task — the volatile tier is where short-lived, session-relevant context
  lives in `PromptBuilder`.
- It is **best-effort**: any failure (store unavailable, embedder errors, empty
  result set) yields an empty string, and the agent starts regardless — memory
  must never block an agent from starting.
- The block footer instructs the model on the loop: *use
  `memory_search`/`memory_digest` to pull more detail, `memory_boost` a memory
  id when it proved useful* — closing the reinforcement loop described in
  principle 3.
- Budget is bounded (`max_chars`): when the digest exceeds budget, entries are
  trimmed entry-by-entry, keeping the most relevant memories first, so prompt
  size stays predictable.

### MemoryClass — durability classes

When absorbing, a fact can be marked with a durability class via the `memory_class` field:

| class | behavior |
|-------|-----------|
| `durable` | (default) stored forever, not archived by TTL |
| `ephemeral` | automatically in scope `run` — archived after distill |
| `expiring` | default TTL 90 days (or `metadata.expires_at`) |

Durability classes let the caller express *how long a fact should matter* at
write time, which GC then respects: durable facts are the permanent knowledge
base, ephemeral facts are session noise that distillation promotes or archives,
and expiring facts carry an explicit shelf life (either the 30-day default or a
specific `metadata.expires_at` timestamp that GC honors).

### Cross-call consolidation

If LLM classification is disabled, the absorb heuristic merges similar
facts from different calls: cosine ≥ 0.85 + common subject → merge into the
existing row (update content/tags/confidence, no new record).

This is the no-LLM analogue of the in-batch consolidation, applied *across*
calls instead of within one: when the heuristic cannot use a classifier, it
checks whether a candidate shares both high vector similarity (≥ 0.85) and a
rough common subject (an overlap heuristic over the statement text). If so, the
new fact is folded into the existing row — content, tags and confidence are
updated — and **no new record is created**. The store thus stays deduplicated
even in fully offline/heuristic mode.

## Embeddings

- `auto` (default): OpenAI-compatible `/embeddings` when a key is present,
  otherwise offline TF-IDF (hashing trick, 512 dims, no network and no models).
- On HTTP endpoint failure (e.g., chat gateway without `/embeddings` → 404)
  automatic fallback to TF-IDF until the end of the process.
- `openai` / `tfidf` — forced backend choice.
- Vectors of different models are not mixed (each is stored with its own `model`);
  after a model change — `memory rebuild`.

The embedding layer has two backends mirroring Memora's layering:

- **TF-IDF hashing embedder** (offline fallback, model name `tfidf-hash-512`):
  tokens are lowercased alphanumeric/Cyrillic words of length ≥ 2, hashed into
  512 buckets with signed contributions, then L2-normalized. Deterministic,
  lexical-only, and completely dependency-free — no network, no model files —
  which makes it usable in tests, CI and air-gapped environments, and good
  enough for exact-ish recall.
- **OpenAI-compatible embedder**: any endpoint implementing `/v1/embeddings`
  (OpenAI, OpenRouter, Ollama, vLLM, LM Studio, …) — notably the chat gateway
  when it happens to expose an embeddings route (default model
  `text-embedding-3-small`).

The `auto` mode picks the HTTP backend when credentials exist and falls back to
TF-IDF otherwise; on a *runtime* failure (e.g. a chat gateway without
`/embeddings` that answers 404), a fallback wrapper permanently degrades to
TF-IDF for the rest of the process. Every vector is stored with its producing
model name, and search only ever loads same-model rows — vectors from different
models are never compared (doing so would silently corrupt cosine ranking).
Because the fallback switches the model name, vectors from the two regimes never
mix; `memory rebuild` re-embeds the existing rows under the current model to
resync after a model change.

## Entity graph

In parallel with facts, a graph of real-world entities is maintained
(mem0 GRAPH-MEMORY, in a SQLite implementation):

- nodes: `person`, `company`, `project`, `technology`, `role`, `location`,
  `event`, `product`, `other`; **deduplication by (name + type)** —
  "Ivan Petrov" and "ivan petrov" are one node;
- edges: `works_at`, `leads`, `owns`, `member_of`, `located_in`, `founded`,
  `uses`, `related_to`... (free vocabulary);
- multi-hop BFS traversal up to 4 hops — answers to questions like "who leads X",
  "who works at Kazan companies";
- auto-filling from absorb: if `metadata.facts` contains
  `entities: [{name, type}]` and `relations: [{from, to, relation}]`,
  nodes and edges are created together with the fact.

The entity graph complements the memory-to-memory edges: while `memory_edges`
relate statements to each other, the graph relates *things in the world* —
people, companies, technologies — and is tuned for multi-hop OSINT questions
("who runs companies in sector X"). Details:

- **Node dedup** keys on `(name, type)` with normalization: names are trimmed
  and whitespace-collapsed, and case-insensitivity is handled by the DB
  collation — storing "Ivan Petrov" twice reuses the same node. The entity-type
  vocabulary is an open set: known types are canonicalized, unknown ones are
  stored as-is.
- **Traversal** is BFS from a starting node up to `max_depth` hops (the CLI and
  tool default to 4), returning all simple paths found as alternating
  node/relation chains (`nodes[0] — start`; `relations[i]` connects
  `nodes[i] → nodes[i+1]`, and a trailing ⁻¹ marks an inverse edge — e.g. asking
  "who works_at X" vs "X leads whom").
- **Auto-fill** happens during absorb: when an absorbed fact carries
  `metadata.facts.entities` and `metadata.facts.relations`, nodes and edges are
  created together with the fact in one transaction. Unknown relation endpoints
  are created as `other` nodes — explicit typing wins when present.

Tool `memory_graph`: `add` (entities/relations), `query` (name, depth),
`list` (entity_type).

## Agent tools

| tool | purpose |
|-----|-----------|
| `memory_absorb` | save facts (facts[], source, scope, dry_run) — runs the full absorb pipeline (validation, secret scan, consolidation, dedup, classification) and reports per-fact verdicts |
| `memory_search` | hybrid search or read by id with follow mode — fused cosine+BM25 ranking, or direct access to an id under `active`/`latest`/`full_history` |
| `memory_digest` | context digest by topic — the same deterministic aggregator used at session start, callable mid-session to pull a bounded, budget-checked block for any topic |
| `memory_boost` | raise/lower the importance of a useful record (+0.5 by default) — importance influences ranking under tight score margins and future digests; a record that directly helped complete the task should be boosted so it wins tight races later |
| `memory_link` | typed edge between two records (default `related_to`) — lets the agent record that two facts belong together explicitly |
| `memory_graph` | entity graph operations — add/query/list over the entity graph |

All tools operate on the memory store attached to the `ToolContext` (the same
shared `Memory` handle as the runtime digest); if `[memory]` is disabled they
fail with a clear "memory subsystem not configured" error. The default scope
filter for tools is persistent memories (agent + user) plus the current run's
facts, so a session sees both long-term knowledge and its own episode records.
Memory tools are registered for every agent role (no role deny-list entry), and
the digest footer explicitly coaches agents to use `memory_absorb` for durable
findings (verified contacts, company facts) and `memory_search`/`memory_digest`
to check what is already known *before* re-researching.

## Automatic records (without model participation)

- **Contacts**: successful `extract_contacts` / `find_leads` deterministically
  absorb the extracted emails/phones/leads into scope `agent`
  (confidence 0.6, tags `contact`/`lead`), together with saving to the contacts
  database;
- **Session digest**: `finalize_session` saves the session summary
  (tag `session-summary`, confidence 0.7).

Both paths use heuristic classification without LLM calls.

These two paths are the *self-writing* part of the knowledge base: they require
no model participation and no explicit agent decision, so they work identically
in offline/heuristic mode and never consume model budget. `extract_contacts`
and `find_leads` (OSINT tools) deterministically promote every successful
extraction into long-term memory at confidence 0.6, tagged `contact`/`lead` —
in addition to the contacts database row — so a later session can recall "CEO
email at Acme" without re-running the research. `finalize_session` archive the
session's summary as a `session-summary` record at confidence 0.7, giving every
session a permanent, searchable footprint in the archive. Both go through the
heuristic classification path (no LLM round-trip), meaning they are cheap and
deterministic.

## CLI

```bash
fathom memory search "CEO email at Acme" [--top-k 10] [--scope agent|user|run|all]
fathom memory list [--scope ...] [--status active|superseded|archived|all] [-n 20]
fathom memory get <id> [--follow active|latest|full_history]
fathom memory stats
fathom memory rebuild
fathom memory distill [--session <key>] [--dry-run]
fathom memory gc [--ttl-days <N>] [--dry-run]
fathom memory nuke --scope run --yes
```

The CLI is the direct, scriptable surface for the same store the agent and HTTP
API use: `search` for hybrid retrieval, `list`/`get` for inspection and
supersession-chain reads, `stats` for store size/health, `rebuild` to re-embed
after a model change, `distill` and `gc` for the offline maintenance passes
(both support `--dry-run` to see what would happen), and `nuke` as the explicit
run-scoped reset (note the required `--yes`).

## Distillation

`distill` — an analogue of nightly consolidation (openclaude `/dream`): run facts
(session episodes) are run through absorb into scope `agent`; duplicates are
filtered out, unique knowledge is pinned, the original run records are archived.
Nothing is deleted — the archive is accessible by id. It is launched manually
(`memory distill`), scheduled via `jobs submit`, or will be invoked by a nightly job.

Distillation exists because run-scoped episode facts accumulate during a session
and most of them are noise a week later — but a few are durable knowledge that
should outlive the session. The pass re-runs every run fact through the **full
absorb pipeline** into scope `agent`, so dedup and supersession apply exactly as
they would for a live absorb: facts already known in agent scope are skipped,
unique new knowledge is promoted/pinned, and then the run-scoped originals are
archived. It is an offline job, so facts are absorbed one by one — each outcome
can be attributed to its source row and reported. The report surface exposes
`promoted` (new knowledge in agent scope), `skipped` (already known),
`archived` (run records processed) and `errors` (facts left as-is), plus
`dry_run`. `--session <key>` limits the pass to a single session; without it,
every run-scoped fact is distilled. Nothing is deleted anywhere in this flow:
archived rows stay queryable by id / `follow=full_history`. It runs on demand,
as a scheduled durable job (`jobs submit`), or as part of the hourly automatic
maintenance when `gc_auto = true`.

## GC (TTL archiving, confidence decay, and compaction)

`gc` — a conservative offline cleanup in four stages (only archives, never deletes):

1. **expired** — active rows with a passed `expires_at` are archived;
2. **stale** — run facts older than `[memory] gc_ttl_days` (default 30), which
   were never accessed (`access_count = 0`) and were not
   boosted (`importance < 0.75`), are archived: valuable content should have
   been picked up by `distill`, the rest is noise;
3. **confidence decay** — the confidence of all active rows decreases proportionally
   to idle time: `effective = decay_rate × (1 − resistance) × (days/30)`,
   where `resistance = min(access_count × 0.05, 0.8)`. Frequently requested
   facts resist decay. Rows with confidence < `gc_confidence_threshold`
   (default 0.15) are archived — they stop competing for space in retrieval.
4. **compaction** — a scope group with the number of active rows greater than
   `[memory] gc_compact_above` (default 200) merges the oldest and
   least important redundant rows into one consolidated record
   (bulleted summary, `metadata.gc = true`), the originals are archived and
   linked with `references` edges.

Long-lived stores accumulate noise: expired facts, run-scoped episode details
nobody ever recalled, and scope groups that grow past a useful size. GC is a
conservative offline pass (like `distill`) whose invariant is **archive, never
delete** — so every row stays queryable via `follow=full_history` even after
cleanup, and GC can always be reasoned about as pure housekeeping. Stage 1
honors explicit expiry (`expires_at` from the `expiring` memory class). Stage 2
assumes distillation is the intended promotion path: run facts that were never
accessed and never boosted are pure noise and are archived. Stage 3 applies
time-based confidence decay with a resistance term — every retrieval
(`access_count`) raises resistance, so frequently requested facts effectively
stop decaying — and candidly *removes from active competition* (archives) rows
whose effective confidence drops below the floor. Stage 4 keeps scope groups
bounded: the oldest, least-important surplus rows of an overgrown
`(scope, scope_key)` group (default threshold 200 active rows) are merged into
one consolidated bulleted-summary record marked `metadata.gc = true`, with the
originals archived and linked via `references` edges — the store shrinks N→1
without losing content, keeping digests and searches fast. All stages are
tunable through `[memory]` knobs (`gc_ttl_days`, `gc_confidence_threshold`,
`gc_compact_above`), and a `--dry-run` reports exactly what would be archived
without touching anything.

### Automatic GC

With `[memory] gc_auto = true`, a background task (hourly) is started:
`gc()` + `distill()` — cleanup and transfer of run facts to the agent scope.

This makes the store fully self-maintaining: on the hour, the background task
first distills run facts into agent knowledge (promoting the durable, archiving
the rest) and then runs the four-stage GC pass, so the archive stays compact
and current without any operator involvement. Both passes are idempotent and
archives-only, so even automatic maintenance can never destroy data.

Like distill, it is available via CLI (`memory gc --dry-run`) and HTTP
(`POST /api/v1/memories/gc`).

## HTTP API

`GET/POST /api/v1/memories`, `/memories/absorb`, `/memories/stats`,
`/memories/distill`, `/memories/gc`, `GET/DELETE /api/v1/memories/:id` —
see [HTTP-API.md](HTTP-API.md).

All routes are nested under `/api/v1`, behind auth and rate limiting, and return
the same DTOs the CLI uses. In detail:

- `GET /memories` — list with scope/status filters and a page limit (default
  status `active`, default limit 20), or hybrid search when `?q=` is present.
- `POST /memories/absorb` — absorb a batch of facts through the full pipeline.
- `GET /memories/stats` — store statistics.
- `POST /memories/distill?session=&dry_run=` — promote run facts into agent
  knowledge, optionally limited to one session and/or dry-run.
- `POST /memories/gc?ttl_days=&dry_run=` — run the GC pass, optionally with a
  custom stale-TTL and/or dry-run.
- `GET /memories/:id?follow=latest` — one memory, resolved under
  `active`/`latest`/`full_history` (default `latest`).
- `DELETE /memories/:id` — archive (soft delete); the row and its history stay
  intact and remain queryable via `follow=full_history`.

Unconfigured servers answer with a clear "memory not configured" response
rather than a confusing 500.

## Configuration

Section `[memory]` — the full list of fields in
[CONFIGURATION.md](CONFIGURATION.md).

Key `[memory]` knobs surfaced throughout this document: `enabled` (turning the
subsystem on), `embedding_model` (default `text-embedding-3-small`) and
`embedding` backend choice (`auto`/`openai`/`tfidf`), `auto_digest` (inject the
session digest into top-level agents), `llm_classify` (LLM vs heuristic
classification in absorb), `rerank` (second-pass LLM reranking),
`top_k` (default 5 for digests, 10 for searches), `temporal_decay` (freshness
rate), plus the GC knobs `gc_ttl_days` (30), `gc_confidence_threshold` (0.15),
`gc_compact_above` (200) and `gc_auto` (hourly background maintenance). The
store lives at `~/.fathom/memory.db` unless `PR_MEMORY_DB` points
elsewhere.

## Performance

Benchmark: `fathom bench -s memory` (release, macOS 10 cores):

| operation | result |
|---|---|
| absorb | 49–521 µs/fact |
| re-absorb 100 facts (dedup) | 2.2 ms |
| hybrid search (up to 1K records) | 0.6–0.9 ms |
| search @ 5K / 10K records | 8.6 / 17.5 ms |
| digest | ~2.0 ms |

The numbers show the design costs stay sub-millisecond at the scales an agent
archive actually hits: absorb with the full validation/secret/dedup pipeline
runs in tens to hundreds of microseconds per fact, re-absorbing 100 already-known
facts (the dedup path, the most common repeated operation) costs ~2.2 ms,
hybrid search is under a millisecond up to 1K records and degrades gracefully
(8.6 ms @ 5K, 17.5 ms @ 10K), and the deterministic digest that feeds the
system prompt costs ~2 ms. Nothing in the memory loop — absorb at write time,
search/digest at read time — is a bottleneck next to LLM calls.