---
name: fathom-memory
description: Architecture, storage engine, and query patterns for Fathom's long-term semantic memory and knowledge graph in crates/memory. Use when implementing memory tools, entity graph extraction, HNSW indexing, or secret scrubbing.
---

# Long-Term Semantic Memory & Entity Graph (`crates/memory`)

Fathom features an in-process, privacy-preserving semantic memory subsystem combining relational storage, vector similarity, and entity relationship graphs.

## 1. Core Architecture

```
crates/memory/src/
├── db.rs          # SQLite-backed storage (facts, edges, scopes, sessions)
├── absorb.rs      # Pipeline: Validate -> Secret Scan -> Consolidation -> Dedup -> Edge Linking
├── search.rs      # Hybrid search: Vector Cosine Similarity + BM25 Full-Text Search (FTS5)
├── graph.rs       # Directed entity knowledge graph (Subject-Predicate-Object triples)
├── secrets.rs     # Zero-leak regex scanner (API keys, private keys, JWTs, bearer tokens)
├── hnsw.rs        # In-process Hierarchical Navigable Small World vector index
├── distill.rs     # Session summarization and durable knowledge extraction
├── embed.rs       # Embedder trait (local fast-embed / offline TF-IDF / private API)
└── gc.rs          # Knowledge garbage collection, decay pruning, and scope compaction
```

## 2. Invariants & Security Rules

* **Secret Scrubbing on Write**:
  * Every fact passed to `absorb()` is scanned via `secrets::detect_secrets()` BEFORE storage.
  * API tokens (`sk-...`, `phc_...`, `Bearer`), private keys (`-----BEGIN PRIVATE KEY-----`), and passwords are automatically redacted or rejected.
* **Immutable Version Chaining**:
  * Memories are append-only. When an agent updates knowledge, it does NOT overwrite rows.
  * It creates a new memory node with a typed edge: `supersedes`, `contradicts`, or `refines`.
* **Scoping & Isolation**:
  * Memory records are strictly scoped: `global`, `workspace`, `agent`, or `session`.
  * Multi-tenant agents cannot query or leak facts across disparate workspace scopes.
* **Hybrid Retrieval (RRF)**:
  * Queries combine reciprocal rank fusion (RRF) of vector embeddings with SQLite FTS5 BM25 keyword matching for sub-millisecond lookups (~94 µs).
