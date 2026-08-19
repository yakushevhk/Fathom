# 05. Live case studies: six sessions with a real LLM

All sessions ran the release binary against a real LLM
(DeepSeek, a reasoning model) with no mocks. Each session left a full
SQLite trace — the numbers below come from the `stats` command.

The working pattern was the same every time: live run → anomaly in the trace →
diagnosis → fix with a regression test → rerun. Below are the cases in
chronological order, including the failures — they are worth more than the successes.

---

## Case 1. AI-agent market research — and an empty report

**Request**: research the AI-agent market (market size, key players, trends,
source verification). 4 parallel research agents.

**What worked**: planning, parallel spawning, batched web search.
Peak concurrency — **15 simultaneous tool calls**, saved
81 s (39% of busy time overlapped).

| | |
|---|---|
| Tool calls | 70 (web_search 26, web_fetch 32, …) |
| Peak parallelism | 15 |
| Tokens | 1 428 392 |
| Result | ❌ `summary.md` — **0 bytes** |

**Diagnosis**. Probing the API with curl showed: the reasoning model puts its
chain of thought into `reasoning_content`, and those tokens **are subtracted
from `max_tokens`**. With a budget of 8192, the model spends the reasoning up to
the limit and returns empty `content` with `finish_reason: "length"`. A naive
client gets a "empty model response"; in reality the answer is there — in the reasoning.

**Fix** (three levels):

1. `pr-llm/deepseek.rs`: parse `reasoning_content` + warn
   "content is empty while reasoning is not — budget is likely exhausted";
2. `coordinator.rs::synthesize()`: synthesis budget ≥ 16 384, retry with
   doubling up to 32 768, deterministic fallback report from findings
   (the report always exists, even if the LLM could not produce one);
3. `runtime.rs`: up to 2 loop retries with an explicit model hint
   "output budget exhausted — give a final answer".

All three behaviors are covered by tests (including a `BudgetSpy` provider
asserting `max_tokens ≥ 16384`).

## Case 2. Vector DB market — research after the fix

**Request**: competitive landscape of the 2025 vector DB market, top-5 players.

| | |
|---|---|
| Tool calls | 16 (web_search 12 × ~3.9 s, web_fetch 2) |
| Peak parallelism | 4 |
| Result | ✅ `summary.md` 12 428 bytes, findings + sources.md |

Confirmation: after the fix, synthesis reliably produces a full report.
The session shows the typical research profile: batched search of 2–4 queries,
then targeted fetches of the found pages.

## Case 3. Qdrant OSINT — contact verification and a broken database

**Request**: find and verify the public contacts of the Qdrant team.

**What worked brilliantly**:
- `verify_email` 19/19 — real SMTP verification (dialog up to `250 OK`),
  real working addresses of the `andre.zayarni@qdrant.com` level confirmed;
- `verify_social_profile` 18/18, `suggest_emails` 13/13;
- 285 tool calls, peak concurrency **13**, parallelism saved
  **912.5 seconds** — 67% of busy time ran overlapped.

| | |
|---|---|
| Tool calls | 285 |
| Peak parallelism | 13 |
| Tokens | 4 718 005 |
| Result | ⚠️ contacts found, but `save_contacts` **0/8** |

**Trace diagnosis** (`SELECT output FROM tool_results WHERE
tool_name='save_contacts'`): "Contact database is not available". Two bugs:

1. **Migration after index.** Legacy `contacts.db` without the `phone_norm`
   column; the code first ran `CREATE INDEX idx_contacts_phone_norm` and only
   then `add_column_if_missing` — opening the DB failed. Fix: migrations
   before indexes; the regression test creates the legacy schema with raw
   rusqlite and checks that opening migrates it without losing rows.
2. **Strings instead of arrays.** The model sent `notes: "text"` and
   `tags: "a; b"` as strings, while the schema expected `Vec<String>` — the
   call was rejected at deserialization. Fix: a lenient `one_or_many_strings`
   deserializer (a string is split on `;`/line breaks, an array is accepted as-is).

## Case 4. Hetzner — saving contacts after the fix

**Request**: find 2–3 public work emails of Hetzner executives and
save the contacts.

| | |
|---|---|
| Tool calls | 106 (web_search 24, web_fetch 37, verify_email 10 …) |
| Peak parallelism | 9 |
| Saved | 345.5 s (55% busy) |
| Result | ✅ `save_contacts` 3/3, **14 contacts** in `contacts.db` |

Check: `contacts list` opens the migrated database, rows are in place,
`find_by_phone` works on the normalized phone. The session also shows how
the agent combines sources: the corporate site (`parse_corporate_site`),
directories (`search_business_directory`), news, `shell` with curl for
non-standard endpoints, `python_exec` for data normalization.

## Case 5. Writing code — a Python CLI with tests

**Request**: write `sales_report.py` (CSV with columns
date,product,category,quantity,price → revenue by category, top-3
products, MoM growth), generate a realistic CSV, write pytest tests,
run them and fix the failures.

**Agent behavior** (11 tool calls, 361 850 tokens):

1. `file_write` × 3: `sales_report.py`, `generate_sample_data.py`,
   `test_sales_report.py` — real working code, not pseudocode;
2. `shell`: data generation and first pytest run → **found 1 failing
   test**;
3. `file_edit`: a targeted fix to the calculation logic;
4. `shell`: pytest again → **23/23 passed**.

**Independent verification** (outside the agent): `pytest -q` — 23/23; running
the CLI — correct revenue table by category, top-3, MoM percentages.

**Observation for the article**: peak parallelism in this session was **1**, and
that is correct. Writing code is sequential by nature (you cannot write a test
before the module it tests). Agent parallelism shows up where the task
parallelizes: search, verification, parsing. Tool busy time — only 1.3 s out of
74.7 s of the session: in coding tasks, the LLM's thinking time dominates,
not execution.

## Case 6. Parsing — HN and the GitHub API with the new tools

**Request**: parse the Hacker News front page (`parse_html`, selector
`.titleline`), get `qdrant/qdrant` stats from the GitHub API via
`extract_json`, pull the first 5 story IDs from the Firebase API. Explicit
condition — not to use `web_fetch` for extraction.

| | |
|---|---|
| Agents | 3 (in parallel) |
| Tool calls | 11 (parse_html 4, extract_json 5, file_write 2) |
| Peak parallelism | 4 (a batch of 4 extract_json went out simultaneously) |
| Tokens | 280 803 |
| Result | ✅ all data extracted structurally |

Artifact check: 10 HN posts with titles and URLs (cross-checked against the
real page at run time), `stargazers_count: 33835`, `forks_count:
2562`, `language: Rust`, story IDs `[54249175, …]`. The `web_fetch`
condition was met.

---

## Summary of bugs found by live runs

| Bug | How found | Fix | Test |
|---|---|---|---|
| Empty synthesis from a reasoning model | 0-byte summary.md + curl probes | budget ≥16384, retries, fallback, parse reasoning_content | 5 tests |
| contacts.db migration after CREATE INDEX | `save_contacts` 0/8 in the trace | order: migrations → indexes | regression with a legacy DB |
| notes/tags sent as strings rejected | deserialization error in the trace | lenient one_or_many | unit test |
| sources.md empty | empty file after research | tool metadata + harvest branches | 4 tests |
| Root cause of errors lost | "Tool execution error" without details | `{e:#}` formatting | — |
| join_all gives no CPU parallelism | benchmark (speed-up 1.0×) | `execute_batch_spawn` | 3 tests |

Moral for the article: **the SQLite trace of every call is the main debugging
tool**. All six bugs were found with queries against `tool_results`/artifacts,
not by reading code; every fix is locked in with a regression and a repeated
live run.