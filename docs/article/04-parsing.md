# 04. Parsing: parse_html and extract_json

Before this iteration, the agent could only read web pages as flat text
(`web_fetch` converts HTML to text). For tasks like "get top-10 posts",
"extract the price table", "fetch stargazers_count from the API" this was
insufficient — structured access was needed. This is how two tools in
`pr-tools/parse.rs` (~700 lines, 13 unit tests) came to be.

## parse_html

Extracts data from HTML using a CSS selector. Source can be a URL (downloaded
via the shared secured fetcher) or a local file.

### Parameters

| Parameter | Required | Description |
|---|---|---|
| `source` | yes | URL (`https://…`) or file path |
| `selector` | yes | CSS selector (`scraper`/`cssparser`): `a.title`, `tr.item td`, `#main > p` |
| `mode` | no (default `texts`) | `texts`, `html`, `attr`, `links`, `tables` |
| `attribute` | only for `attr` | attribute name (`href`, `data-id`, …) |
| `limit` | no (default 100) | maximum number of items in the result |

### Modes: two semantics

**Target modes** (`texts`, `html`, `attr`) select matching elements across the
entire document and return them as-is:

- `texts` — text content of each match;
- `html` — inner HTML (for passing to the next parse call);
- `attr` — attribute value of each match.

**Region modes** (`links`, `tables`) take the **first** selector match
as a region and extract structure from it:

- `links` — all `<a href>` inside the region, URLs are absolutized relative
  to the source, garbage (`#`, `javascript:`) is discarded;
- `tables` — all `<table>` inside the region as arrays of rows; if the region
  itself is a table, it is also included.

If there are no matches, a structured error is returned with `err_code:
"no_match"` — the agent understands the selector needs refining, not that the
tool is broken.

### Output

```json
{
  "source": "https://news.ycombinator.com",
  "mode": "texts",
  "count": 10,
  "items": ["…", "…"]
}
```

Output is truncated at 50 KB, metadata (`source`, `title`, `count`) goes into
the harvest pipeline — every successful parse automatically becomes a
`Finding` with the source in `sources.md`.

### Example from a live session

Task: top-10 Hacker News posts.

```json
{
  "source": "https://news.ycombinator.com",
  "selector": ".titleline a",
  "mode": "texts"
}
```

Result — 10 titles with URLs, verified in the session artifact:

```
1. Launch HN: Trase (YC X25) – The open-source warehouse for AI agent workloads
   https://trase.dev
2. A Duct Tape Moment
   https://blog.jessfraz.com/post/a-duct-tape-moment/
…
```

A second call (`mode: links` on `.titleline`) obtained absolute links.
No `web_fetch` + manual text parsing needed.

## extract_json

Extracts values from JSON using a dot-path. Sources:

- URL (JSON API) — downloaded via the same secured fetcher;
- local file;
- **inline JSON** directly in the argument (if it starts with `{` or `[`).

### Path syntax

| Segment | Example | Meaning |
|---|---|---|
| key | `repo.name` | object field |
| index | `items[0]` | array element |
| wildcard | `results[*].email` | all array elements |
| numeric key on array | `store.book.1.title` | lenient indexing |

Wildcard can be combined with a further path: `items[*].meta.score`.

### Example from a live session

```json
{ "source": "https://api.github.com/repos/qdrant/qdrant", "path": "stargazers_count" }
```

→ `33835` (also `forks_count` → 2562, `language` → `"Rust"` — four calls
went out in **one parallel batch**, session peak parallelism 4).

## Shared fetch infrastructure

Both tools reuse `fetch_url_cached` (extracted from `web.rs`):

1. **Session cache** — re-fetching the same URL does not hit the network
   (agents often parse the same page multiple times).
2. **SSRF guard on every hop** — redirects do not follow into internal networks:
   `ensure_safe_url` is checked for both the original URL and each
   `Location` header individually.
3. **Body limit of 2 MB** (`FETCH_MAX_BYTES`) — read with truncation so that
   a gigabyte-sized response does not kill memory or context.
4. **Error encoding** — `blocked` / `not_found` / `rate_limited` /
   `timeout` / `http_error` with status text: the agent knows what to do
   (switch mirror, wait, refine URL).

## Performance

From [03-benchmarks.md](./03-benchmarks.md):

- `parse_html`: ~900,000 rows/s, linear up to 773 KB (13.7 ms);
- `extract_json` on a 4 MB document: 15–28 ms depending on the query;
- both tools are in the `parallel_safe` list — parsing batches execute on
  different cores via `execute_batch_spawn` (3.18× speedup on 8 parses).

## Boundaries and conscious limitations

- No JavaScript execution: SPA pages require browser tools (CDP),
  which are only registered when an endpoint is available.
- `extract_json` re-parses the document on every call — a conscious
  trade-off in favor of statelessness and parallel safety; 20–30 ms
  on 4 MB is acceptable for agent scenarios.
- Selectors are not validated upfront: an invalid selector returns a parser
  error, the agent reads it and corrects the request (verified by a live run).