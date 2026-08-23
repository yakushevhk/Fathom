// ── API types matching fathom HTTP API ──────────────────────────────────

export interface Session {
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
  sessions: Session[]
  count: number
}

export type SessionCreateResponse = Pick<Session, 'id' | 'status' | 'query' | 'output_dir'>

export interface SessionResults {
  session_id: string
  status: string
  output_dir: string
  total_tokens: number
  total_agents: number
  summary: string
  findings: { file: string; content: string }[]
}

export interface Agent {
  id: string
  session_id: string
  role: string
  task: string
  status: string
  depth: number
  tokens_used: number
  created_at: string
  completed_at: string | null
}

export interface AgentListResponse {
  agents: Agent[]
  count: number
}

export interface Health {
  status: string
  active_sessions: number
}

export interface Job {
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
}

export interface JobListResponse {
  jobs: Job[]
  count: number
}

export interface Memory {
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
}

export interface MemoryListResponse {
  memories: Memory[]
  query?: string
}

export interface MemoryStats {
  embedding_model: string
  scopes: Record<string, { active: number; superseded: number; archived: number }>
  entity_graph: { nodes: number; edges: number }
}

export interface DistillReport {
  promoted: number
  skipped: number
  archived: number
  errors: number
  dry_run: boolean
}

export interface GcReport {
  expired_archived: number
  stale_archived: number
  confidence_archived: number
  confidence_decayed: number
  groups_compacted: number
  facts_compacted: number
  errors: number
  dry_run: boolean
}

export interface AgentEvent {
  type: string
  id?: string
  agent_id?: string
  session_id?: string
  request_id?: string
  [key: string]: unknown
}

export interface ObservabilitySummary {
  active_sessions: number
  sessions_total: number
  agents_spawned: number
  tool_calls: number
  tokens_used: number
  audit_events: number
  audit_denials: number
  audit_counts_truncated: boolean
}

/** Metadata returned by the credentials endpoint; the secret is never returned. */
export interface Credential {
  id: string
  name: string
  kind: string
  created_at: string
  updated_at: string
}

export interface StoreCredentialInput {
  name: string
  kind: string
  secret: string
}

export interface ReplayAction {
  id: string
  agent: string
  session: string
  tool: string
  args_redacted: string
  decision: string
  started_at: string
  completed_at: string | null
  duration_ms: number | null
  result_redacted: string | null
  screenshot_before: string | null
  screenshot_after: string | null
  policy_version: string
}

export interface ReplayListResponse {
  actions: ReplayAction[]
}

// ── Coworker operations types ───────────────────────────────────────────

export interface Coworker {
  id: string
  name: string
  title: string
  role: string
  prompt: string
  visibility: string
  active: boolean
  created_at: string
  updated_at: string
}

export interface CoworkerListResponse { coworkers: Coworker[] }
export interface CoworkerResponse { coworker: Coworker }

export interface Channel {
  id: string
  coworker_id: string
  title: string
  session_id: string | null
  created_at: string
  updated_at: string
}

export interface ChannelListResponse { channels: Channel[] }
export interface ChannelResponse { channel: Channel }

export interface Schedule {
  id: string
  coworker_id: string
  cron_expression: string
  timezone: string
  query: string
  enabled: boolean
  next_run: string
  last_run: string | null
  created_at: string
  updated_at: string
}

export interface ScheduleListResponse { schedules: Schedule[] }
export interface ScheduleResponse { schedule: Schedule }

// ── Governance and computer control types ───────────────────────────────

export type PolicyEffect = 'allow' | 'deny'

export interface PolicyRule {
  id?: string
  effect: PolicyEffect
  tool?: string
  host?: string
  path?: string
  intent?: string
  // Forward-compatible fields accepted by persisted policy implementations.
  action?: string
  resource?: string
  description?: string
  conditions?: Record<string, unknown>
  priority?: number
  enabled?: boolean
  created_at?: string
  updated_at?: string
}

export interface PolicyResponse {
  enabled: boolean
  policy: { rules: PolicyRule[] }
}

export interface ActionContext {
  agent: string
  session: string
  tool: string
  args: Record<string, unknown>
  url?: string
  element?: string
  file?: string
  intent?: string
  mcp_metadata?: Record<string, unknown>
  // Client convenience aliases accepted by older gateways.
  action?: string
  resource?: string
  actor?: string
  session_id?: string
  parameters?: Record<string, unknown>
  metadata?: Record<string, unknown>
  [key: string]: unknown
}

export type Decision = 'allow' | 'deny' | {
  allowed: boolean
  effect?: PolicyEffect
  reason?: string
  rule_id?: string
  matched_rule_id?: string
  action?: string
  resource?: string
  evaluated_at?: string
}

export interface AuditEvent {
  id: string
  timestamp: string
  agent: string
  session: string
  tool: string
  args: string
  decision: PolicyEffect
  [key: string]: unknown
}

export interface AuditResponse {
  events: AuditEvent[]
  count?: number
  next_cursor?: string | null
}

export interface ComputerHealth {
  ok: boolean
  service?: string
  status?: string
  control?: { owner?: 'bot' | 'human'; humanSince?: number }
  [key: string]: unknown
}

export interface ComputerSession {
  /** Session responses identify the active browser through their snapshot tab. */
  tab_id?: string
  session_id?: string
  id?: string
  ok?: boolean
  status?: string
  created_at?: string
  browser?: string
  url?: string
  snapshot?: ComputerSnapshot
  control?: { owner?: 'bot' | 'human'; humanSince?: number }
}

export interface ComputerSnapshot {
  /** Opaque active-tab identity returned by the computer service. */
  tab_id?: string
  /** Legacy session identity accepted from older relay responses. */
  session_id?: string
  url?: string
  title?: string
  refs?: Record<string, { role?: string; name?: string; text?: string; tag?: string; value?: string }>
  elements?: Array<{ ref: string; role?: string; name?: string; text?: string; tag?: string; value?: string }>
  aria?: unknown
  screenshot?: string
  control?: { owner?: 'bot' | 'human'; humanSince?: number }
  [key: string]: unknown
}

export interface ComputerScreenshot {
  data?: string
  image?: string
  url?: string
  mime_type?: string
  content_type?: string
  mimeType?: string
}

export interface ComputerActionResult {
  ok?: boolean
  owner?: 'bot' | 'human'
  humanSince?: number
  control?: { owner?: 'bot' | 'human'; humanSince?: number }
  tab_id?: string
  session_id?: string
  message?: string
  [key: string]: unknown
}

// ── API client ──────────────────────────────────────────────────────────

const DEFAULT_BASE = 'http://127.0.0.1:8080'
export const SERVER_URL_KEY = 'fathom_base_url'
export const API_KEY_KEY = 'fathom_api_key'

function getBase(): string {
  if (typeof window === 'undefined') return DEFAULT_BASE
  const configured = localStorage.getItem(SERVER_URL_KEY)?.trim()
  if (!configured) return DEFAULT_BASE
  try {
    const url = new URL(configured)
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return DEFAULT_BASE
    return url.toString().replace(/\/$/, '')
  } catch {
    return DEFAULT_BASE
  }
}

function getApiKey(): string | null {
  if (typeof window !== 'undefined') return localStorage.getItem(API_KEY_KEY)
  return null
}

export function apiKeyHeaders(): Record<string, string> {
  const key = getApiKey()
  return key ? { 'X-Api-Key': key } : {}
}

export function apiBaseUrl(): string {
  return getBase().replace(/\/$/, '')
}

export class ApiError extends Error {
  readonly status: number | null
  readonly body: unknown

  constructor(message: string, status: number | null = null, body?: unknown) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.body = body
  }
}

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const base = getBase().replace(/\/$/, '')
  const apiKey = getApiKey()
  const headers: Record<string, string> = {
    Accept: 'application/json',
    ...(options?.headers as Record<string, string>),
  }
  if (apiKey) headers['X-Api-Key'] = apiKey

  let res: Response
  try {
    res = await fetch(`${base}${path}`, { ...options, headers })
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Network request failed'
    throw new ApiError(`Unable to reach Fathom server: ${message}`)
  }
  if (!res.ok) {
    let body: unknown = null
    try { body = await res.json() } catch { body = await res.text().catch(() => '') }
    const detail = typeof body === 'string' ? body : JSON.stringify(body)
    throw new ApiError(`API ${res.status}${detail ? `: ${detail.slice(0, 300)}` : ''}`, res.status, body)
  }
  if (res.status === 204) return undefined as T
  return res.json() as Promise<T>
}

export const api = {
  health: () => request<Health>('/health'),

  sessions: {
    list: () => request<SessionListResponse>('/api/v1/sessions'),
    get: (id: string) => request<Session>(`/api/v1/sessions/${id}`),
    create: (query: string, output_dir?: string) =>
      request<SessionCreateResponse>('/api/v1/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query, ...(output_dir ? { output_dir } : {}) }),
      }),
    cancel: (id: string) =>
      request<{ id: string; status: string }>(`/api/v1/sessions/${id}`, { method: 'DELETE' }),
    steer: (id: string, message: string) =>
      request<{ id: string; steered: boolean }>(`/api/v1/sessions/${id}/steer`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message }),
      }),
    answer: (id: string, requestId: string, text: string) =>
      request<{ answered: boolean; request_id: string }>(`/api/v1/sessions/${id}/answer`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ request_id: requestId, text }),
      }),
    approve: (id: string, requestId: string, approved: boolean) =>
      request<{ approved: boolean; request_id: string }>(`/api/v1/sessions/${id}/approve`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ request_id: requestId, approved }),
      }),
    results: (id: string) => request<SessionResults>(`/api/v1/sessions/${id}/results`),
  },

  events: {
    sessionUrl: (id: string) => `${apiBaseUrl()}/api/v1/sessions/${id}/events`,
    globalUrl: () => `${apiBaseUrl()}/api/v1/events`,
  },

  observability: {
    summary: () => request<ObservabilitySummary>('/api/v1/observability/summary'),
  },

  credentials: {
    list: () => request<Credential[]>('/api/v1/credentials'),
    create: (input: StoreCredentialInput) =>
      request<Credential>('/api/v1/credentials', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(input),
      }),
    delete: (id: string) =>
      request<void>(`/api/v1/credentials/${encodeURIComponent(id)}`, { method: 'DELETE' }),
  },

  replay: {
    list: (options?: { session?: string; agent?: string; limit?: number }) => {
      const params = new URLSearchParams()
      if (options?.session) params.set('session', options.session)
      if (options?.agent) params.set('agent', options.agent)
      if (options?.limit) params.set('limit', String(options.limit))
      const query = params.toString()
      return request<ReplayListResponse>(`/api/v1/replay${query ? `?${query}` : ''}`)
    },
  },

  agents: {
    list: () => request<AgentListResponse>('/api/v1/agents'),
    get: (id: string) => request<Agent>(`/api/v1/agents/${id}`),
  },

  coworkers: {
    list: () => request<CoworkerListResponse>('/api/v1/coworkers'),
    get: (id: string) => request<CoworkerResponse>(`/api/v1/coworkers/${encodeURIComponent(id)}`),
    create: (input: Omit<Coworker, 'id' | 'created_at' | 'updated_at'>) =>
      request<CoworkerResponse>('/api/v1/coworkers', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(input),
      }),
    update: (id: string, input: Omit<Coworker, 'id' | 'created_at' | 'updated_at'>) =>
      request<CoworkerResponse>(`/api/v1/coworkers/${encodeURIComponent(id)}`, {
        method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(input),
      }),
    archive: (id: string) =>
      request<{ deleted: boolean }>(`/api/v1/coworkers/${encodeURIComponent(id)}`, { method: 'DELETE' }),
  },

  channels: {
    list: (coworkerId: string) => request<ChannelListResponse>(`/api/v1/channels?coworker_id=${encodeURIComponent(coworkerId)}`),
    create: (input: { coworker_id: string; title: string; session_id?: string }) =>
      request<ChannelResponse>('/api/v1/channels', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(input),
      }),
    delete: (id: string) => request<{ deleted: boolean }>(`/api/v1/channels/${encodeURIComponent(id)}`, { method: 'DELETE' }),
  },

  schedules: {
    list: () => request<ScheduleListResponse>('/api/v1/schedules'),
    create: (input: { coworker_id: string; cron_expression: string; timezone: string; query: string; enabled: boolean }) =>
      request<ScheduleResponse>('/api/v1/schedules', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(input),
      }),
    delete: (id: string) => request<{ deleted: boolean }>(`/api/v1/schedules/${encodeURIComponent(id)}`, { method: 'DELETE' }),
  },

  jobs: {
    list: () => request<JobListResponse>('/api/v1/jobs'),
    get: (id: string) => request<Job>(`/api/v1/jobs/${id}`),
    log: (id: string) => request<{ lines: string[]; total_lines: number; returned: number }>(`/api/v1/jobs/${id}/log`),
    create: (task: string, attempts = 3) =>
      request<Job>('/api/v1/jobs', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ task, attempts }),
      }),
    cancel: (id: string) =>
      request<{ id: string; status: string }>(`/api/v1/jobs/${id}`, { method: 'DELETE' }),
    rerun: (id: string) =>
      request<Job>(`/api/v1/jobs/${id}/rerun`, { method: 'POST' }),
  },

  governance: {
    policy: () => request<PolicyResponse>('/api/v1/governance/policy'),
    savePolicy: (rules: PolicyRule[]) =>
      request<PolicyResponse>('/api/v1/governance/policy', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rules }),
      }),
    audit: (options?: { limit?: number; decision?: string; agent?: string; session?: string }) => {
      const params = new URLSearchParams()
      if (options?.limit) params.set('limit', String(options.limit))
      if (options?.decision) params.set('decision', options.decision)
      if (options?.agent) params.set('agent', options.agent)
      if (options?.session) params.set('session', options.session)
      const query = params.toString()
      return request<AuditEvent[] | AuditResponse>(`/api/v1/governance/audit${query ? `?${query}` : ''}`)
    },
    decide: (context: ActionContext) =>
      request<Decision>('/api/v1/governance/decide', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(context),
      }),
  },

  computers: {
    createSession: (options?: { url?: string }) =>
      request<ComputerSession>('/api/v1/computers/session', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(options ?? {}),
      }),
    health: () => request<ComputerHealth>('/api/v1/computers/health'),
    snapshot: () => request<ComputerSnapshot>('/api/v1/computers/snapshot'),
    navigate: (url: string) =>
      request<ComputerSnapshot>('/api/v1/computers/navigate', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url }),
      }),
    click: (ref: string) =>
      request<ComputerSnapshot>('/api/v1/computers/click', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ ref }),
      }),
    secret: (ref: string, secret: string) =>
      request<ComputerSnapshot>('/api/v1/computers/secret', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ ref, secret }),
      }),
    type: (ref: string, text: string, submit = false) =>
      request<ComputerSnapshot>('/api/v1/computers/type', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ ref, text, submit }),
      }),
    key: (key: string, ref?: string) =>
      request<ComputerSnapshot>('/api/v1/computers/key', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ key, ...(ref ? { ref } : {}) }),
      }),
    takeControl: () => request<ComputerActionResult>('/api/v1/computers/control/take', { method: 'POST' }),
    releaseControl: () => request<ComputerActionResult>('/api/v1/computers/control/release', { method: 'POST' }),
    screenshot: async (): Promise<string> => {
      const base = apiBaseUrl()
      const response = await fetch(`${base}/api/v1/computers/screenshot`, { headers: { Accept: 'image/*', ...apiKeyHeaders() } })
      if (!response.ok) throw new ApiError(`API ${response.status}`, response.status)
      return URL.createObjectURL(await response.blob())
    },
  },

  memories: {
    list: () => request<MemoryListResponse>('/api/v1/memories'),
    get: (id: string) => request<{ memories: Memory[] }>(`/api/v1/memories/${encodeURIComponent(id)}`),
    archive: (id: string) => request<{ archived: string }>(`/api/v1/memories/${encodeURIComponent(id)}`, { method: 'DELETE' }),
    absorb: (content: string, source = 'web') => request<unknown>('/api/v1/memories/absorb', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ facts: [{ content, metadata: {} }], source }),
    }),
    stats: () => request<MemoryStats>('/api/v1/memories/stats'),
    distill: (options?: { session?: string; dryRun?: boolean }) => {
      const params = new URLSearchParams()
      if (options?.session) params.set('session', options.session)
      if (options?.dryRun !== undefined) params.set('dry_run', String(options.dryRun))
      const query = params.toString()
      return request<DistillReport>(`/api/v1/memories/distill${query ? `?${query}` : ''}`, { method: 'POST' })
    },
    gc: (options?: { ttlDays?: number; dryRun?: boolean }) => {
      const params = new URLSearchParams()
      if (options?.ttlDays !== undefined) params.set('ttl_days', String(options.ttlDays))
      if (options?.dryRun !== undefined) params.set('dry_run', String(options.dryRun))
      const query = params.toString()
      return request<GcReport>(`/api/v1/memories/gc${query ? `?${query}` : ''}`, { method: 'POST' })
    },
  },

  raw: (path: string, options?: RequestInit) => request<unknown>(path, options),
}