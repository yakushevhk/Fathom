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

export interface AgentEvent {
  type: string
  id?: string
  agent_id?: string
  session_id?: string
  [key: string]: unknown
}

// ── Public API (Tauri commands → Rust backend → fathom engine) ──────────

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
    results: (id: string) => invoke<unknown>('get_session_results', { id }),
  },

  agents: {
    list: () => invoke<unknown[]>('list_agents'),
    get: (id: string) => invoke<unknown>('get_agent', { id }),
  },

  jobs: {
    list: () => invoke<unknown[]>('list_jobs'),
    create: (task: string, attempts = 3) =>
      invoke<unknown>('create_job', { task, attempts }),
    cancel: (id: string) => invoke<void>('cancel_job', { id }),
  },

  memories: {
    list: () => invoke<unknown[]>('list_memories'),
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
  baseUrl: string,
  endpoint: string,
  onEvent: (data: unknown) => void,
  onError?: (err: string) => void,
): AbortController {
  const controller = new AbortController()

  const start = async () => {
    try {
      const response = await fetch(`${baseUrl}${endpoint}`, {
        signal: controller.signal,
        headers: { Accept: 'text/event-stream' },
      })
      if (!response.ok) {
        onError?.(`SSE connection failed: ${response.status}`)
        return
      }
      const reader = response.body?.getReader()
      if (!reader) {
        onError?.('No response body')
        return
      }
      const decoder = new TextDecoder()
      let buffer = ''
      for (;;) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        buffer = lines.pop() ?? ''
        for (const line of lines) {
          if (line.startsWith('data: ')) {
            try {
              onEvent(JSON.parse(line.slice(6)))
            } catch {
              // skip malformed frames
            }
          }
        }
      }
    } catch (err) {
      if (!controller.signal.aborted) {
        onError?.(String(err))
      }
    }
  }

  start()
  return controller
}