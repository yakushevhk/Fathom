import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// ── Tauri IPC bridge types ──────────────────────────────────────────────

export interface DaemonStatus {
  running: boolean
  url: string | null
  port: number | null
  binary: string | null
  version: string | null
  phase: 'starting' | 'running' | 'stopped' | 'error'
  error: string | null
}

export interface SessionSummary {
  id: string
  query: string
  status: string
  output_dir: string | null
  total_tokens: number
  total_agents: number
  created_at: string
  updated_at: string
  active: boolean
}

export interface SessionListResponse {
  sessions: SessionSummary[]
  count: number
}

export interface AgentSummary {
  id: string
  session_id: string
  parent_id?: string | null
  role?: string
  task?: string
  status?: string
  tokens_used?: number
  created_at?: string
  completed_at?: string | null
  [key: string]: unknown
}

export interface AgentListResponse {
  agents: AgentSummary[]
  count: number
}

export interface JobRow {
  id: string
  task: string
  status: string
  attempt: number
  max_attempts: number
  output_dir: string
  error: string | null
  pid: number | null
  created_at: string
  updated_at: string
  started_at: string | null
  completed_at: string | null
  [key: string]: unknown
}

export interface JobListResponse {
  jobs: JobRow[]
  count: number
}

export interface MemoryDto {
  id: string
  content: string
  scope: string
  scope_key: string
  status: string
  source: string
  confidence: number
  importance: number
  tags: string[]
  created_at: string
  expires_at?: string
  score?: number
  [key: string]: unknown
}

export interface MemoryListResponse {
  memories: MemoryDto[]
}

export interface AgentEvent {
  type: string
  id?: string
  agent_id?: string
  session_id?: string
  [key: string]: unknown
}

export interface ComputerHealth {
  ok: boolean
  status?: string
  url?: string
  control?: { owner?: 'bot' | 'human'; humanSince?: string }
  control_owner?: string | null
  [key: string]: unknown
}

export interface ComputerSnapshot {
  url?: string
  title?: string
  screenshot?: string
  control?: { owner?: 'bot' | 'human'; humanSince?: string }
  control_owner?: string | null
  refs?: Record<string, unknown>
  [key: string]: unknown
}

export interface PolicyRule {
  id?: string
  effect: 'allow' | 'deny'
  tool?: string
  host?: string
  path?: string
  intent?: string
}

export interface GovernanceStatus {
  enabled: boolean
  mode: 'enabled' | 'disabled' | 'unknown'
  status: 'active' | 'quiet' | 'unavailable'
  rules?: PolicyRule[]
  [key: string]: unknown
}

export interface AuditEvent {
  id: string
  agent: string
  session: string
  tool: string
  decision: 'allow' | 'deny' | string
  args?: unknown
  url?: string | null
  file?: string | null
  intent?: string | null
  created_at?: string
  timestamp?: string
  [key: string]: unknown
}

// ── Public API (Tauri commands → Rust backend → fathom engine) ──────────

async function rawRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const method = init?.method ?? 'GET'
  let body: unknown = undefined
  if (init?.body) {
    try { body = JSON.parse(String(init.body)) } catch { body = String(init.body) }
  }
  return invoke<T>('engine_request', { method, path, body })
}

export const api = {
  daemon: {
    status: () => invoke<DaemonStatus>('daemon_status'),
    start: (port?: number, force = false) =>
      invoke<DaemonStatus>('daemon_start', { port, force }),
    stop: () => invoke<void>('daemon_stop'),
  },

  sessions: {
    list: () => invoke<SessionListResponse>('list_sessions'),
    create: (query: string) =>
      invoke<SessionSummary>('create_session', { query }),
    get: (id: string) => invoke<SessionSummary>('get_session', { id }),
    cancel: (id: string) => invoke<void>('cancel_session', { id }),
    steer: (id: string, message: string) =>
      invoke<void>('steer_session', { id, message }),
    answer: (id: string, requestId: string, text: string) =>
      invoke<{ answered: boolean; request_id: string }>('answer_session', { id, requestId, text }),
    approve: (id: string, requestId: string, approved: boolean) =>
      invoke<{ approved: boolean; request_id: string }>('approve_session', { id, requestId, approved }),
    results: (id: string) => invoke<unknown>('get_session_results', { id }),
  },

  agents: {
    list: () => invoke<AgentListResponse>('list_agents'),
    get: (id: string) => invoke<unknown>('get_agent', { id }),
  },

  jobs: {
    list: () => invoke<JobListResponse>('list_jobs'),
    create: (task: string, attempts = 3) =>
      invoke<unknown>('create_job', { task, attempts }),
    cancel: (id: string) => invoke<void>('cancel_job', { id }),
  },

  memories: {
    list: () => invoke<MemoryListResponse>('list_memories'),
  },

  /** REST bridge used by the governed computer surfaces. */
  raw: rawRequest,

  computer: {
    health: (base = '/api/v1/computers') =>
      rawRequest<ComputerHealth>(`${base}/health`),
    snapshot: (base = '/api/v1/computers') =>
      rawRequest<ComputerSnapshot>(`${base}/snapshot`),
    screenshot: async (base = '/api/v1/computers') => {
      const payload = await invoke<{ bytes: number[]; content_type: string }>('engine_screenshot', { path: `${base}/screenshot` })
      return new Blob([new Uint8Array(payload.bytes)], { type: payload.content_type })
    },
    action: (name: 'navigate' | 'click' | 'type' | 'key' | 'secret', payload: Record<string, unknown>, base = '/api/v1/computers') =>
      rawRequest<Record<string, unknown>>(`${base}/${name}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      }),
    takeControl: (base = '/api/v1/computers') =>
      rawRequest<Record<string, unknown>>(`${base}/control/take`, { method: 'POST' }),
    releaseControl: (base = '/api/v1/computers') =>
      rawRequest<Record<string, unknown>>(`${base}/control/release`, { method: 'POST' }),
  },

  governance: {
    status: async (base = '/api/v1/governance') => {
      const response = await rawRequest<{ enabled: boolean; policy: { rules: PolicyRule[] } }>(`${base}/policy`)
      return { enabled: response.enabled, mode: response.enabled ? 'enabled' : 'disabled', status: response.enabled ? 'active' : 'quiet', rules: response.policy.rules } satisfies GovernanceStatus
    },
    rules: async (base = '/api/v1/governance') => {
      const response = await rawRequest<{ enabled: boolean; policy: { rules: PolicyRule[] } }>(`${base}/policy`)
      return response.policy.rules
    },
    decide: (context: { agent: string; session: string; tool: string; args: unknown; url?: string; element?: string; file?: string; intent?: string; mcp_metadata?: unknown }, base = '/api/v1/governance') =>
      rawRequest<{ allowed?: boolean; decision?: string }>(`${base}/decide`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(context) }),
    audit: async (base = '/api/v1/governance') =>
      rawRequest<AuditEvent[]>(`${base}/audit`),
    updateRule: async (rule: PolicyRule, base = '/api/v1/governance') => {
      const current = await rawRequest<{ enabled: boolean; policy: { rules: PolicyRule[] } }>(`${base}/policy`)
      const rules = [...current.policy.rules]
      const syntheticIndex = rule.id?.match(/^rule-(\d+)$/)?.[1]
      const index = syntheticIndex === undefined ? rules.findIndex(item =>
        item.effect === rule.effect && item.tool === rule.tool && item.host === rule.host && item.path === rule.path && item.intent === rule.intent,
      ) : Number(syntheticIndex)
      if (index >= 0 && index < rules.length) {
        const { id: _id, ...persisted } = rule
        rules[index] = persisted
      } else {
        const { id: _id, ...persisted } = rule
        rules.push(persisted)
      }
      await rawRequest(`${base}/policy`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ rules }) })
      return rule
    },
  },
}

// ── Tauri events (daemon status pushed from backend) ────────────────────

export function listenToDaemonStatus(cb: (status: DaemonStatus) => void): () => void {
  let unlisten: (() => void) | undefined
  const stopPolling = pollStatus(cb)

  // Tauri-native events take priority; the poller remains as a fallback so
  // the UI stays live even if the backend event loop hiccups.
  listen<DaemonStatus>('daemon:status', e => {
    cb(e.payload)
  }).then(fn => {
    unlisten = fn
  })

  return () => {
    stopPolling()
    unlisten?.()
  }
}

function pollStatus(cb: (status: DaemonStatus) => void): () => void {
  const interval = setInterval(async () => {
    try {
      cb(await api.daemon.status())
    } catch {
      // engine unreachable — keep polling
    }
  }, 5000)
  return () => clearInterval(interval)
}

// ── SSE stream from fathom engine (direct, for live transcript) ─────────

export function connectSSE(
  _baseUrl: string,
  endpoint: string,
  onEvent: (data: unknown) => void,
  onError?: (err: string) => void,
): AbortController {
  const controller = new AbortController()
  const streamId = `sse-${Date.now()}-${Math.random().toString(36).slice(2)}`
  let unlisten: (() => void) | undefined

  void listen<unknown>(`engine:sse:${streamId}`, event => onEvent(event.payload))
    .then(stop => { unlisten = stop; if (controller.signal.aborted) stop() })
    .catch(error => onError?.(String(error)))

  void invoke('engine_sse_start', { streamId, path: endpoint })
    .catch(error => { if (!controller.signal.aborted) onError?.(String(error)) })

  controller.signal.addEventListener('abort', () => {
    unlisten?.()
    void invoke('engine_sse_stop', { streamId })
  }, { once: true })
  return controller
}