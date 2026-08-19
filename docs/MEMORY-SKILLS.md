# Memory and Skills System

Fathom has three systems for preserving knowledge between sessions:

1. **File memory** (MEMORY.md/USER.md) — a stable "profile" of ~2 KB,
   always in the prompt (this document);
2. **Long-term semantic memory** (SQLite, crate `pr-memory`) —
   an unlimited knowledge archive with hybrid search, an entity graph, and
   fact versioning — see [MEMORY-KB.md](MEMORY-KB.md);
3. **Skills** (SKILL.md) — reusable instructions.

File memory is inspired by Hermes and OpenClaude; semantic memory by mem0 and Memora.
Role separation: `MEMORY.md` holds only hot facts that affect every
session; everything else (found contacts, research results, facts about
companies) goes into the semantic database and is retrieved as needed
(`memory_search`/`memory_digest`, with a digest in the prompt before start).

---

## Memory

### Overview

File memory lives in `~/.fathom/memory/`:
- **`MEMORY.md`** — the agent's personal notes (environment facts, conventions, tool quirks, lessons)
- **`USER.md`** — what the agent knows about the user (preferences, style, habits)

The directory is created automatically on first write. Records are separated by the `§` (U+00A7, section sign) character. Each record is free-form text (may be multi-line). The `§` delimiter is never used in content text and cleanly splits entries on load.

### Design rationale

File memory serves as a **hot cache** — the few most important facts that must influence every session. Unlike the long-term semantic store (which can hold millions of facts retrieved on demand), file memory is kept deliberately small (~2 KB total) so it can be injected into every system prompt without wasting token budget. The two files provide role separation: `MEMORY.md` holds facts about the world and how the agent operates, while `USER.md` holds facts about the person interacting with the agent. This prevents a user preference from being accidentally evicted when the agent's own notes overflow the budget.

### Limits

| File | Default limit |
|------|---------------|
| MEMORY.md | 2200 characters |
| USER.md | 1375 characters |

When a limit is exceeded, old records are removed (budget enforcement). The enforcement algorithm drops the **oldest** entries first (entries from the front of the file), preserving the most recently added records. This is because memory entries are appended at the end, so the newest entries reflect the most current knowledge. Limits are defined as constants in `crates/core/src/memory.rs` (`DEFAULT_MAX_MEMORY_CHARS`, `DEFAULT_MAX_USER_CHARS`) and can be customized via `MemoryStore::with_budgets()` for testing or alternative configurations.

### Frozen snapshot

Key design: a **frozen snapshot** of memory is injected into the system prompt at the start of the session. During the session, records are saved to files immediately, but **do not change the prompt** — this preserves the LLM prefix cache. The snapshot is updated in the next session.

This is a deliberate caching optimization: the system prompt is assembled once at session start (via `PromptBuilder` in `crates/agent/src/prompt.rs`), and `memory_store.to_system_prompt_block()` renders the memory into a `## Memory` section with `### Persistent Memory` and `### User Context` subsections. Because the prompt does not change mid-session, the LLM provider can reuse KV-cache entries across turns, reducing latency and cost. The trade-off is that facts written during a session only become visible to the agent in the **next** session — but the agent can still use the `memory_search` / `memory_digest` tools to query the semantic store for information it just wrote.

### Usage via the `memory` tool

The agent manages memory through the tool (implemented in `crates/tools/src/memory_tool.rs`):

```
memory(action="add", target="memory", content="DeepSeek works better with temperature 0.5")
memory(action="add", target="user", content="The user prefers concise answers")
memory(action="replace", target="memory", old_text="old text", content="new text")
memory(action="remove", target="memory", old_text="text to remove")
memory(action="batch", operations=[...])  # atomic operations
```

**Parameters:**

| Parameter | Description |
|-----------|-------------|
| `action` | `add` \| `replace` \| `remove` \| `batch` |
| `target` | `memory` \| `user` |
| `content` | Content (for add/replace) |
| `old_text` | Substring to search for (for replace/remove) |
| `operations` | Array of operations (for batch) |

**Notes:**
- `replace`/`remove` match on a **unique substring** of a record
- `batch` applies **atomically** against the final budget
- Drift protection: a record is rejected if the file changed externally
- Atomic write (temp file + rename)

### How the `memory` tool works internally

The `MemoryTool` struct implements the `Tool` trait. On each call, it creates a fresh `MemoryStore` instance rooted at `~/.fathom/memory/`. The `execute` method deserializes the `MemoryParams` (action, target, content, old_text, operations) and dispatches to the corresponding `MemoryStore` method.

- **`add`**: reads the current file, checks for duplicate content (exact match), appends the new entry, serializes with `§` delimiters, enforces the character budget (dropping oldest entries from the front if over limit), and writes atomically.
- **`replace`**: loads all entries, finds the one whose content contains `old_substr` (via `str::contains`), replaces it with `new_content`, re-serializes, enforces budget, and writes.
- **`remove`**: loads all entries, retains only those that do NOT contain `old_substr`, serializes, and writes. Fails if no entry matched.
- **`batch`**: loads both files into memory, applies all operations in sequence (add/replace/remove), enforces budget on both files simultaneously, then writes both files atomically. If any single operation fails, the entire batch is aborted — no partial writes.

### Atomic write protocol

The `atomic_write` method writes to a `.md.tmp` temporary file, then renames it atomically over the target file (`std::fs::rename`). This prevents partial writes: if the process crashes mid-write, the original file remains intact. The rename is atomic on all major platforms (POSIX `rename()` and Windows `ReplaceFile` semantics). Parent directories are created on demand.

### Typed memories

Memory supports typing (YAML frontmatter):

| Type | Description |
|------|-------------|
| `user` | User's role, goals, preferences |
| `feedback` | Corrections and confirmed approaches (Why / How to apply) |
| `project` | Current work, goals, incidents |
| `reference` | Pointers to external resources |

Typed memories are stored as YAML frontmatter blocks delimited by `---`:

```markdown
---
name: user-preferences
description: "The user prefers B2B SaaS leads"
type: user
---
The user works in B2B SaaS and is interested in the EU market.
```

The `Frontmatter` struct (in `crates/core/src/memory.rs`) holds `name`, `description`, and `memory_type` fields. The `to_frontmatter_string()` method serializes to the YAML-like format, and `parse_frontmatter()` parses it back. The `name` field is required — if missing, the frontmatter is considered invalid. The `escape_yaml_value` function handles quoting for values containing special characters (`:`, `#`, `"`, `'`, leading/trailing spaces).

Typed entries are stored alongside regular entries in the same `MEMORY.md`/`USER.md` files, delimited by `§` like any other entry. The `TypedMemoryEntry` struct wraps a `Frontmatter` + `body` + `created_at` timestamp, and `to_entry_string()` reconstructs the full representation.

### Entries vs. typed records

The `MemoryEntry` struct is the simplest unit: `content` (the text) + `created_at` (timestamp). `TypedMemoryEntry` extends this with frontmatter metadata. Both coexist in the same file — the `parse_entries` method splits on `§` and returns `MemoryEntry` (ignoring the typing), while frontmatter provides richer metadata for agent routing decisions.

### Relationship to long-term semantic memory

File memory (MEMORY.md/USER.md) is intentionally limited to ~2 KB of the most critical facts. The **long-term semantic memory** (SQLite-backed, `pr-memory` crate) stores everything else — contacts, research findings, company profiles, technical notes — with hybrid search, entity graph, and fact versioning. The `memory_*` tools (`memory_absorb`, `memory_search`, `memory_digest`, `memory_boost`, `memory_link`, `memory_graph`) operate on the semantic store and are available via both the HTTP API (see `crates/server/src/memory_api.rs`) and the agent tool interface.

The HTTP API for semantic memory is mounted at `/api/v1/memories` and provides:
- `GET /memories` — list/search memories
- `POST /memories/absorb` — absorb facts through the full pipeline
- `GET /memories/stats` — store statistics
- `POST /memories/distill` — promote run facts into agent knowledge
- `POST /memories/gc` — garbage-collect expired/stale facts
- `GET /memories/:id` — retrieve a single memory with follow chain
- `DELETE /memories/:id` — soft-delete (archive)

---

## Skills

### Overview

Skills are reusable instructions in `SKILL.md` format, stored in `~/.fathom/skills/`. They are discovered by recursive scanning. Each skill is a directory containing a `SKILL.md` file — the directory name becomes the slug for the skill, and the file content provides the full instructions.

### SKILL.md format

```markdown
---
name: ocr-and-documents
description: "Extract text from PDFs and scans."
version: 0.1.0
---

# OCR and Documents

## When to Use
When you need to extract text from a PDF or scanned documents...

## Prerequisites
- pandoc is installed
- ...

## How to Run
1. Use pdf_extract for extraction
2. ...

## Pitfalls
- Some PDFs use custom fonts
```

### Structure

```
~/.fathom/skills/
├── research/
│   ├── web-scraping/
│   │   └── SKILL.md
│   └── data-extraction/
│       └── SKILL.md
└── osint/
    └── lead-generation/
        └── SKILL.md
```

### Design rationale

Skills solve the problem of **prompt bloat** — if every possible instruction were injected into the system prompt, the token budget would be exhausted before any real work began. Instead, the `SkillRegistry::to_system_prompt_block()` method generates a compact index (name + one-line description + file location) that the agent sees in its prompt. When the agent decides to use a skill, it calls the `skill` tool with the skill name, which loads the full `SKILL.md` content on demand. This is a **lazy-loading** pattern: the index is always available, but the full instructions are fetched only when needed.

### Usage

Automatic discovery:
- `SkillRegistry::discover()` scans `skills/` recursively
- Skills are injected into the system prompt (volatile tier)
- The agent sees the list of available skills and can apply them

Creation from experience:
- `SkillRegistry::create_from_experience(task, approach)`
- Generates a SKILL.md from the task description and approach
- Slugified directory name

### How discovery works

The `SkillRegistry` struct (in `crates/core/src/skill.rs`) holds a `skills_dir` path and a `Vec<Skill>`. The `discover()` method clears the existing list, checks if the skills directory exists, and calls `discover_recursive()` which walks the directory tree. For each `SKILL.md` file found, it calls `Skill::from_file()` to parse the name and description from the `# Heading` and the first non-empty, non-heading line after it. If the heading is empty, the parent directory name is used as a fallback. Errors are logged as warnings but do not halt discovery of other skills.

### The `skill` tool

The `SkillTool` (in `crates/tools/src/coordination.rs`) exposes a single `skill` tool that takes a `name` parameter. On invocation, it creates a fresh `SkillRegistry`, runs discovery, and finds the skill by case-insensitive name match. If found, it returns the full `SKILL.md` content as the tool output. If not found, it returns an error listing all available skill names. This is how the agent loads full instructions on demand without bloating the prompt.

### Skill fields

| Field | Description |
|-------|-------------|
| `name` | Name (lowercase-hyphenated) |
| `description` | One-line description (for routing) |
| `content` | Full SKILL.md content |
| `file_path` | Path to the file |
| `created_at` | Creation date |

### Creating skills from experience

The `create_from_experience(task, approach)` method generates a directory slug from the task description (via `slugify()` — replaces non-alphanumeric characters with hyphens, collapses consecutive hyphens), creates the directory, writes a `SKILL.md` with the task as the heading and the approach as the body, and registers the skill in memory. This allows the agent to save successful workflows as reusable skills that will be available in future sessions.

---

## How it all works together

```
Session Start
    │
    ▼
1. MemoryStore.load_memory() → frozen snapshot
    │
    ▼
2. SkillRegistry.discover() → list of skills
    │
    ▼
3. PromptBuilder: memory + skills in the volatile tier
    │
    ▼
4. The agent works, using the memory tool to write records
    │
    ▼
5. Records are saved to files (but the prompt does not change)
    │
    ▼
6. Next session: updated snapshot
```

### Detailed flow

1. **Agent startup** (`crates/agent/src/runtime.rs`): `AgentRuntime::new()` creates a `MemoryStore::new(&home_dir)` and a `SkillRegistry::new(&home_dir)`, then calls `skill_registry.discover()` (best-effort — failure is logged but does not prevent startup).

2. **Prompt assembly** (`build_system_prompt()`): The `PromptBuilder` constructs the system prompt in three tiers:
   - **Stable tier**: model base prompt, role instructions, task description, general behavioral rules
   - **Context tier**: environment info (working directory, platform, git status, model name, date)
   - **Volatile tier**: tool schemas, memory block, semantic memory digest, skills block, profile

   The memory block is rendered by `memory_store.to_system_prompt_block()` — a `## Memory` section with bullet-point entries. The skills block is rendered by `skill_registry.to_system_prompt_block()` — a `## Available Skills` section listing each skill's name, description, and file path.

3. **Mid-session writes**: The agent uses the `memory` tool to add, replace, or remove entries. These writes go to the disk files immediately, but the in-memory prompt is not updated (frozen snapshot optimization).

4. **Skill loading on demand**: When the agent decides to use a skill, it calls the `skill` tool, which loads the full `SKILL.md` content. This content is returned as tool output and becomes part of the conversation context for that turn only.

5. **Semantic memory integration**: The `memory_*` tools (in `crates/tools/src/memory_kb.rs`) operate on the unbounded SQLite-backed store. `memory_digest` produces a pre-session summary of relevant facts that is injected into the volatile tier alongside the file memory. `memory_search` provides ad-hoc retrieval during the session.

---

## Configuration

Memory and skills use standard paths:
- Memory: `~/.fathom/memory/`
- Skills: `~/.fathom/skills/`

Memory limits are set in code (`DEFAULT_MAX_MEMORY_CHARS`, `DEFAULT_MAX_USER_CHARS`). These can be overridden programmatically via `MemoryStore::with_budgets()`. The `SkillRegistry` accepts a custom directory via `SkillRegistry::with_dir()` for testing. There is no configuration file for memory/skill paths — they are always relative to the user's home directory.

---

## Usage examples

### Saving a lesson

```
Agent: memory(
  action="add",
  target="memory",
  content="LinkedIn returns HTTP 999 without a proxy. Use search_social with a fallback to web_search."
)
```

This appends the fact to `MEMORY.md`. In the next session, the agent will see this bullet point in the `## Memory / Persistent Memory` section of its system prompt, and will know to use a proxy for LinkedIn.

### Remembering a user preference

```
Agent: memory(
  action="add",
  target="user",
  content="The user works in B2B SaaS and is interested in the EU market."
)
```

This appends to `USER.md`. The preference is available in every subsequent session under `## Memory / User Context`, guiding the agent's research focus.

### Replacing an outdated fact

```
Agent: memory(
  action="replace",
  old_text="prefers Python",
  content="The user now prefers Rust"
)
```

The `replace` operation searches entries for the substring `"prefers Python"` and replaces the entire matching entry with the new content.

### Batch atomic update

```
Agent: memory(
  action="batch",
  operations=[
    {action: "add", target: "memory", content: "Use temperature 0.3 for code generation"},
    {action: "add", target: "user", content: "User prefers detailed explanations"},
    {action: "remove", target: "memory", old_text: "Use temperature 0.7"}
  ]
)
```

All three operations are applied atomically — if any fails, none are written. Budget is enforced after all operations are applied to both files.

### Creating a skill from a successful research

```
Agent: create_skill(
  task="Finding CEO contacts via EGRUL aggregators",
  approach="1. Find companies on rusprofile by OKVED. 2. Extract INN/CEO. 3. Parse official websites for contacts."
)
```

This creates `~/.fathom/skills/finding-ceo-contacts-via-egrul-aggregators/SKILL.md` with the task as heading and the approach as content. In future sessions, the skill appears in the `## Available Skills` prompt block and can be loaded with `skill(name="finding-ceo-contacts-via-egrul-aggregators")`.

### Loading a skill on demand

```
Agent: skill(name="ocr-and-documents")
```

Returns the full `SKILL.md` content, including prerequisites, step-by-step instructions, and pitfalls. The agent can then follow the workflow.

---

## Testing

- **Memory**: 22 unit tests (add/load/replace/remove/batch/budget/frontmatter/atomic-write)
- **Skills**: 14 unit tests (parsing/discovery/nested-dirs/creation/case-insensitive-lookup/slugify)

Tests live in `crates/core/src/memory.rs` and `crates/core/src/skill.rs` respectively, in `#[cfg(test)] mod tests` blocks. They use temporary directories with UUID-based names to avoid conflicts, and clean up via `cleanup()`. The test coverage includes edge cases: empty content rejection, duplicate entry detection, budget enforcement boundaries, nested directory discovery, fallback to directory name when heading is missing, and atomic write parent directory creation.