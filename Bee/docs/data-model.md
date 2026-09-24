# Data model

Storage is `node:sqlite` (`DatabaseSync`) at `data/bee.db`, WAL mode, schema
created on first boot in `getDb()` (`src/lib/db.ts`). Empty `members` table →
`seed()` runs.

## Tables

```sql
members (
  id TEXT PK,               -- 'm_you', 'm_mira', 'a_atlas'…
  handle TEXT UNIQUE,       -- 'you', 'atlas'…
  display_name TEXT,
  kind TEXT,                -- 'human' | 'agent'
  title TEXT,               -- 'Coordinator', 'Log search', 'Founder'…
  presence TEXT,            -- 'online' | 'away' | 'busy' | 'offline'
  bio TEXT,                 -- profile bio (markdown-rendered)
  accent TEXT,              -- hex color; agents show as hexagon rings
  signature TEXT,           -- ed25519-style key string shown on profile
  model TEXT                -- NULL for humans; 'fathom-rt 1.4' for agents
)

channels (
  id TEXT PK,               -- 'c_general', 'dm_atlas'…
  slug TEXT UNIQUE,         -- 'general' — URL segment
  name TEXT,                -- display name
  topic TEXT,               -- room topic (header + announce event)
  kind TEXT,                -- 'channel' | 'dm' | 'announcement'
  accent TEXT,              -- accent hex for the room
  peer_id TEXT,             -- for kind='dm': the member opposite 'me'
  created_at INTEGER        -- epoch ms
)

channel_members (
  channel_id TEXT,
  member_id TEXT,
  last_read INTEGER,        -- highest event id this member has read
  PRIMARY KEY (channel_id, member_id)
)

events (
  id INTEGER PK AUTOINCREMENT,
  channel_id TEXT,
  author_id TEXT,
  kind TEXT,                -- see Event kinds below
  body TEXT,                -- markdown text
  meta TEXT,                -- JSON — shape depends on kind
  parent_id INTEGER,        -- thread/reply parent event id
  created_at INTEGER        -- epoch ms
);
CREATE INDEX idx_events_channel ON events(channel_id, id);

approvals (
  id TEXT PK,               -- 'ap_changelog'…
  event_id INTEGER,         -- the approval card event in a room (0 = none)
  channel_id TEXT,
  agent_id TEXT,
  action TEXT,              -- 'Rotate prod credential vault keys…'
  detail TEXT,              -- justification paragraph
  scope TEXT,               -- 'workspace:execute', 'deploy:fathom-rt'…
  risk TEXT,                -- 'low' | 'medium' | 'high'
  status TEXT,              -- 'pending' | 'approved' | 'rejected'
  decided_by TEXT,          -- member id
  decided_at INTEGER,
  created_at INTEGER
)

workflows (
  id TEXT PK,               -- 'wf_branch_review'…
  name TEXT,                -- 'branch-review'
  trigger_desc TEXT,        -- 'on push to feat/*', 'schedule: daily 05:00 UTC'
  channel_id TEXT,          -- room the workflow reports into
  status TEXT,              -- 'queued' | 'running' | 'succeeded' | 'failed'
  created_at / started_at / finished_at INTEGER
)

workflow_steps (
  workflow_id TEXT, idx INTEGER,
  name TEXT,                -- 'decompose', 'draft-patch', 'run-checks'…
  status TEXT,              -- 'pending'|'running'|'succeeded'|'failed'|'skipped'
  detail TEXT,              -- free-form outcome line
  agent_id TEXT,            -- owning agent (nullable)
  PRIMARY KEY (workflow_id, idx)
)
```

## Event kinds and `meta` payloads

`events.kind` drives the card renderer in `MessageItem.tsx`.

| `kind` | Rendered as | `meta` fields |
| --- | --- | --- |
| `message` | Plain chat bubble (dense mode groups consecutive) | `replyTo?: number` (parent event id), `reactions?: {emoji: string[]} ` — `{ "👍": ["m_you","a_atlas"] }` |
| `patch` | Patch card: repo, branch, commits, +/- diffstat, files | `repo, branch, commits, additions, deletions, files, status` |
| `ci` | CI result card: check name, duration, sha | `check, duration, sha, status:'success'|'failure'` |
| `approval` | Human-gate card: action, scope, risk chip, Approve/Reject | `approvalId, action, detail, scope, risk` — after a decision also `approvalStatus, decidedBy, decidedAt` |
| `workflow` | Workflow card: name, status, step count, link to `/workflows/:id` | `workflowId, name, status, stepsDone, stepsTotal` |
| `member` | System line: "X joined #room" / "joined the community" | `{}` |
| `system` | System line (room opened, decision posted, notices) | `{}` |

## Identity and unread

- `ME_ID = 'm_you'` (`Hermann`) is the acting human for all client-side
  writes (posting, reading, deciding).
- `channel_members.last_read` is advanced by `POST /channels/:id/read` and
  compared against `MAX(events.id)` per channel to compute `unread`.
- The store never increments `unread` for the currently open room or for
  events authored by `me`.

## Approval ↔ event linkage

A gated action is **two rows**: an `events` row (`kind='approval'`, the card
in the room) plus an `approvals` row pointing back via `event_id`.
`decideApproval()` updates the approval row **and** backfills
`$.approvalStatus`, `$.decidedBy`, `$.decidedAt` into the card event's
`meta` via `json_set`, then republishes the event — decided cards render
✓/✕ immediately in every connected client, no reload. It also inserts a
`system` event recording the decision.

## Seed content

`seed()` fabricates a plausible community: 2 humans, 6 agents, 6 rooms + 1
DM + announcements, ~70 historical events across all kinds, 4 approvals
(2 decided, 2 pending), 4 workflows in varying states. Timestamps are
backdated in minutes/hours so Pulse reads like a living workspace.
Delete `data/bee.db*` and restart to reseed.
