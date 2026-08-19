# Memory and Skills System

Parallel Research has three systems for preserving knowledge between sessions:

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

File memory lives in `~/.parallel-research/memory/`:
- **`MEMORY.md`** — the agent's personal notes (environment facts, conventions, tool quirks, lessons)
- **`USER.md`** — what the agent knows about the user (preferences, style, habits)

Records are separated by the `§` (section sign) character. Each record is free-form text (may be multi-line).

### Limits

| File | Default limit |
|------|---------------|
| MEMORY.md | 2200 characters |
| USER.md | 1375 characters |

When a limit is exceeded, old records are removed (budget enforcement).

### Frozen snapshot

Key design: a **frozen snapshot** of memory is injected into the system prompt at the start of the session. During the session, records are saved to files immediately, but **do not change the prompt** — this preserves the LLM prefix cache. The snapshot is updated in the next session.

### Usage via the `memory` tool

The agent manages memory through the tool:

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

### Typed memories

Memory supports typing (YAML frontmatter):

| Type | Description |
|------|-------------|
| `user` | User's role, goals, preferences |
| `feedback` | Corrections and confirmed approaches (Why / How to apply) |
| `project` | Current work, goals, incidents |
| `reference` | Pointers to external resources |

---

## Skills

### Overview

Skills are reusable instructions in `SKILL.md` format, stored in `~/.parallel-research/skills/`. They are discovered by recursive scanning.

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
~/.parallel-research/skills/
├── research/
│   ├── web-scraping/
│   │   └── SKILL.md
│   └── data-extraction/
│       └── SKILL.md
└── osint/
    └── lead-generation/
        └── SKILL.md
```

### Usage

Automatic discovery:
- `SkillRegistry::discover()` scans `skills/` recursively
- Skills are injected into the system prompt (volatile tier)
- The agent sees the list of available skills and can apply them

Creation from experience:
- `SkillRegistry::create_from_experience(task, approach)`
- Generates a SKILL.md from the task description and approach
- Slugified directory name

### Skill fields

| Field | Description |
|-------|-------------|
| `name` | Name (lowercase-hyphenated) |
| `description` | One-line description (for routing) |
| `content` | Full SKILL.md content |
| `file_path` | Path to the file |
| `created_at` | Creation date |

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

---

## Configuration

Memory and skills use standard paths:
- Memory: `~/.parallel-research/memory/`
- Skills: `~/.parallel-research/skills/`

Memory limits are set in code (`DEFAULT_MAX_MEMORY_CHARS`, `DEFAULT_MAX_USER_CHARS`).

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

### Remembering a user preference

```
Agent: memory(
  action="add",
  target="user",
  content="The user works in B2B SaaS and is interested in the EU market."
)
```

### Creating a skill from a successful research

```
Agent: create_skill(
  task="Finding CEO contacts via EGRUL aggregators",
  approach="1. Find companies on rusprofile by OKVED. 2. Extract INN/CEO. 3. Parse official websites for contacts."
)
```

---

## Testing

- **Memory**: 22 unit tests (add/load/replace/remove/batch/budget/frontmatter/atomic-write)
- **Skills**: 14 unit tests (parsing/discovery/nested-dirs/creation/case-insensitive-lookup/slugify)