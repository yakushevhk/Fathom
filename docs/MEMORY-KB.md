# Long-term semantic memory (pr-memory)

An unlimited agent knowledge base in SQLite, built after the
**mem0 / Memora** model (see `docs/mem0/` and `docs/memora/`). Complements the
small file-based memory `MEMORY.md`/`USER.md` (see [MEMORY-SKILLS.md](MEMORY-SKILLS.md)):
file-based memory is a stable "profile" of ~2 KB in the prompt, semantic memory is
a searchable knowledge archive.

## What is stored

Each record is a **self-contained fact** (50–500 characters recommended):

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

## Key principles (from mem0/Memora)

1. **Append-only.** Facts are never overwritten. A new version = a new record +
   a `supersedes` edge to the outdated one; contradictions stay visible from both
   sides via the `contradicts` edge. Read modes by id: `active`,
   `latest` (resolve the chain to the newest), `full_history` (the whole evolution).
2. **Absorb instead of create.** Write pipeline:
   validation → **secret detection** (API keys, tokens, PEM — rejected) →
   consolidation of close facts in a batch (N→1, threshold 0.85) → dedup by hash →
   candidate search by cosine → classification (LLM if available, otherwise
   heuristic by threshold 0.97) → one of 6 outcomes:
   `duplicate` (skip) / `supersede` / `contradict` / `coexist` / `related` / `new`.
   There is `dry_run` — a plan without writing.
3. **Hybrid search.** `score = w·cosine + (1−w)·BM25` (default `w=0.7`),
   then linear freshness decay `score × max(0, 1 − decay·days)`
   (default `decay=0.01/day`), then a gentle boost for reinforcement
   (`min(access_count/10, 1.0)`) and confidence — the formula:
   `score × (0.8 + 0.1·reinforcement + 0.1·confidence)`.
4. **Digest before start.** The top agent (depth 0) receives a deterministic
   digest in the system prompt: relevant memories + open TODOs +
   recent records — with real ids for verification via `memory_search`.
5. **Rerank (optional).** `[memory] rerank = true`: a second LLM pass
   reorders the expanded results (top_k×3) by relevance.

### MemoryClass — durability classes

When absorbing, a fact can be marked with a durability class via the `memory_class` field:

| class | behavior |
|-------|-----------|
| `durable` | (default) stored forever, not archived by TTL |
| `ephemeral` | automatically in scope `run` — archived after distill |
| `expiring` | default TTL 90 days (or `metadata.expires_at`) |

### Cross-call consolidation

If LLM classification is disabled, the absorb heuristic merges similar
facts from different calls: cosine ≥ 0.85 + common subject → merge into the
existing row (update content/tags/confidence, no new record).

## Embeddings

- `auto` (default): OpenAI-compatible `/embeddings` when a key is present,
  otherwise offline TF-IDF (hashing trick, 512 dims, no network and no models).
- On HTTP endpoint failure (e.g., chat gateway without `/embeddings` → 404)
  automatic fallback to TF-IDF until the end of the process.
- `openai` / `tfidf` — forced backend choice.
- Vectors of different models are not mixed (each is stored with its own `model`);
  after a model change — `memory rebuild`.

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

Tool `memory_graph`: `add` (entities/relations), `query` (name, depth),
`list` (entity_type).

## Agent tools

| tool | purpose |
|-----|-----------|
| `memory_absorb` | save facts (facts[], source, scope, dry_run) |
| `memory_search` | hybrid search or read by id with follow mode |
| `memory_digest` | context digest by topic |
| `memory_boost` | raise/lower the importance of a useful record |
| `memory_link` | typed edge between two records |
| `memory_graph` | entity graph operations |

## Automatic records (without model participation)

- **Contacts**: successful `extract_contacts` / `find_leads` deterministically
  absorb the extracted emails/phones/leads into scope `agent`
  (confidence 0.6, tags `contact`/`lead`), together with saving to the contacts
  database;
- **Session digest**: `finalize_session` saves the session summary
  (tag `session-summary`, confidence 0.7).

Both paths use heuristic classification without LLM calls.

## CLI

```bash
parallel-research memory search "CEO email at Acme" [--top-k 10] [--scope agent|user|run|all]
parallel-research memory list [--scope ...] [--status active|superseded|archived|all] [-n 20]
parallel-research memory get <id> [--follow active|latest|full_history]
parallel-research memory stats
parallel-research memory rebuild
parallel-research memory distill [--session <key>] [--dry-run]
parallel-research memory gc [--ttl-days <N>] [--dry-run]
parallel-research memory nuke --scope run --yes
```

## Distillation

`distill` — an analogue of nightly consolidation (openclaude `/dream`): run facts
(session episodes) are run through absorb into scope `agent`; duplicates are
filtered out, unique knowledge is pinned, the original run records are archived.
Nothing is deleted — the archive is accessible by id. It is launched manually
(`memory distill`), scheduled via `jobs submit`, or will be invoked by a nightly job.

## GC (TTL archiving, confidence decay, and compaction)

`gc` — a conservative offline cleanup in four stages (only archives, never deletes):

1. **expired** — active rows with a passed `expires_at` are archived;
2. **stale** — run facts older than `[memory] gc_ttl_days` (default 30), which
   were never accessed (`access_count = 0`) and were not
  _boosted (`importance < 0.75`), are archived: valuable content should have
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

### Automatic GC

With `[memory] gc_auto = true`, a background task (hourly) is started:
`gc()` + `distill()` — cleanup and transfer of run facts to the agent scope.

Like distill, it is available via CLI (`memory gc --dry-run`) and HTTP
(`POST /api/v1/memories/gc`).

## HTTP API

`GET/POST /api/v1/memories`, `/memories/absorb`, `/memories/stats`,
`/memories/distill`, `/memories/gc`, `GET/DELETE /api/v1/memories/:id` —
see [HTTP-API.md](HTTP-API.md).

## Configuration

Section `[memory]` — the full list of fields in
[CONFIGURATION.md](CONFIGURATION.md).

## Performance

Benchmark: `parallel-research bench -s memory` (release, macOS 10 cores):

| operation | result |
|---|---|
| absorb | 49–521 µs/fact |
| re-absorb 100 facts (dedup) | 2.2 ms |
| hybrid search (up to 1K records) | 0.6–0.9 ms |
| search @ 5K / 10K records | 8.6 / 17.5 ms |
| digest | ~2.0 ms |