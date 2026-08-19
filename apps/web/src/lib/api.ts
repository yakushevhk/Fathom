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
  session_id: string | null
  task: string
  status: string
  error: string | null
  attempts: number
  max_attempts: number
  created_at: string
  updated_at: string
}

export interface JobListResponse {
  jobs: Job[]
  count: number
}

export interface Memory {
  id: string
  content: string
  type: string
  created_at: string
  archived: boolean
}

export interface MemoryListResponse {
  memories: Memory[]
  count: number
}

export interface MemoryStats {
  total_facts: number
  archived_facts: number
  cache_entries: number
}

export interface AgentEvent {
  type: string
  id?: string
  agent_id?: string
  session_id?: string
  request_id?: string
  [key: string]: unknown
}

// ── API client ──────────────────────────────────────────────────────────

const DEFAULT_BASE = 'http://127.0.0.1:8080'
export const SERVER_URL_KEY = 'fathom_base_url'
export const API_KEY_KEY = 'fathom_api_key'

function getBase(): string {
  if (typeof window !== 'undefined') {
    return localStorage.getItem(SERVER_URL_KEY) || DEFAULT_BASE
  }
  return DEFAULT_BASE
}

function getApiKey(): string | null {
  if (typeof window !== 'undefined') {
    return localStorage.getItem(API_KEY_KEY)
  }
  return null
}

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const base = getBase()
  const apiKey = getApiKey()
  const headers: Record<string, string> = {
    ...(options?.headers as Record<string, string>),
  }
  if (apiKey) {
    headers['X-Api-Key'] = apiKey
  }
  const res = await fetch(`${base}${path}`, { ...options, headers })
  if (!res.ok) {
    let body = ''
    try {
      const data = await res.json()
      body = JSON.stringify(data)
    } catch {
      body = await res.text().catch(() => '')
    }
    throw new Error(`API ${res.status}: ${body.slice(0, 300)}`)
  }
  if (res.status === 204) return undefined as T
  return res.json()
}

export const api = {
  health: () => request<Health>('/health'),

  sessions: {
    list: () => request<SessionListResponse>('/api/v1/sessions'),
    get: (id: string) => request<Session>(`/api/v1/sessions/${id}`),
    create: (query: string, output_dir?: string) =>
      request<Session>('/api/v1/sessions', {
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
    sessionUrl: (id: string) => `${getBase()}/api/v1/sessions/${id}/events`,
    globalUrl: () => `${getBase()}/api/v1/events`,
  },

  agents: {
    list: () => request<AgentListResponse>('/api/v1/agents'),
    get: (id: string) => request<Agent>(`/api/v1/agents/${id}`),
  },

  jobs: {
    list: () => request<JobListResponse>('/api/v1/jobs'),
    get: (id: string) => request<Job>(`/api/v1/jobs/${id}`),
    log: (id: string) => request<{ log: string }>(`/api/v1/jobs/${id}/log`),
    create: (task: string, attempts = 1) =>
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

  memories: {
    list: () => request<MemoryListResponse>('/api/v1/memories'),
    get: (id: string) => request<Memory>(`/api/v1/memories/${id}`),
    archive: (id: string) => request<{ archived: boolean }>(`/api/v1/memories/${id}`, { method: 'DELETE' }),
    absorb: () => request<{ absorbed: number }>('/api/v1/memories/absorb', { method: 'POST' }),
    stats: () => request<MemoryStats>('/api/v1/memories/stats'),
    distill: () => request<{ action: string }>('/api/v1/memories/distill', { method: 'POST' }),
    gc: () => request<{ removed: number }>('/api/v1/memories/gc', { method: 'POST' }),
  },

  raw: (path: string, options?: RequestInit) => request<unknown>(path, options),
}