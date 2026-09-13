---
name: fathom-swarm
description: Multi-agent coordination, Tokio JoinSet DAG execution trees, hierarchical delegation, and steering in crates/agent. Use when implementing agent roles, coordinator logic, doom-loop protection, compaction, or subagent trees.
---

# Multi-Agent Swarm Orchestration & DAG Runtime (`crates/agent`)

Fathom coordinates specialized digital coworkers organized in hierarchical DAG trees with real-time human-in-the-loop steering.

## 1. Coordinator & Execution Lifecycle (`crates/agent/src/coordinator.rs`)

The `Coordinator` manages the full execution lifecycle of a high-level task:

1. **Decomposition & Planning**:
   * Evaluates input goal using the Lead LLM model.
   * Emits a directed acyclic graph (DAG) of subtasks with declared dependencies.
2. **Parallel Swarm Fan-Out**:
   * Spawns worker subagents into a `tokio::task::JoinSet`.
   * Agents execute with bounded concurrency and strict per-turn token budgets.
3. **Execution Loop (`AgentRuntime::run()`)**:
   * 14-step iterative loop:
     1. Ingest mid-flight user steering instructions.
     2. Collect background child results.
     3. Check session cancellation token.
     4. Context compaction (deduplication + micro-summarization).
     5. Query LLM provider with cached prompt prefixes.
     6. Parse and validate tool calls against typed JSON schemas.
     7. Enforce fail-closed governance policy rules.
     8. Execute tools: parallel-safe concurrently, stateful sequentially.
     9. Enforce read-before-write invariants via `ReadTracker`.
     10. Detect doom loops (3 identical calls -> nudge -> hard abort).
     11. Absorb session findings into semantic memory.
     12. Stream live SSE telemetry and thinking chunks.
     13. Check turn budget and depth limits.
     14. Complete or transition to next subtask.
4. **Synthesis & Deliverables**:
   * Collects outputs, generates cryptographic verification receipts, and formats deliverables.

## 2. Invariants for AI Coding in `crates/agent/`

* **No Async Recursion without Boxing**:
  * Any recursive agent spawning MUST use `Box::pin` to prevent stack overflow in Tokio runtimes.
* **Steering Priority**:
  * Steering messages arriving on `steer_rx` take immediate priority over scheduled tool invocations.
* **Cascade Cancellation**:
  * If a critical shell or compile tool fails in an agent subtree, all sibling tasks sharing the dependency MUST cancel immediately.
