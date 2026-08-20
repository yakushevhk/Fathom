'use client'

import { useEffect, useState } from 'react'
import { api, apiBaseUrl, apiKeyHeaders, type Health, type AgentEvent } from '@/lib/api'
import { useSessions } from '@/hooks/useSessions'
import Link from 'next/link'

export default function HomePage() {
  const [health, setHealth] = useState<Health | null>(null)
  const [healthLoading, setHealthLoading] = useState(true)
  const [healthError, setHealthError] = useState<string | null>(null)
  const [events, setEvents] = useState<AgentEvent[]>([])
  const { sessions, loading: sessionsLoading } = useSessions()

  // Health check (poll every 10s)
  useEffect(() => {
    const load = async () => {
      try {
        const h = await api.health()
        setHealth(h)
        setHealthError(null)
      } catch (e) {
        setHealthError(String(e))
        setHealth(null)
      }
      setHealthLoading(false)
    }
    load()
    const interval = setInterval(load, 10000)
    return () => clearInterval(interval)
  }, [])

  // SSE global events (last 5)
  useEffect(() => {
    const ctrl = new AbortController()
    let reconnectTimer: ReturnType<typeof setTimeout> | undefined
    const connect = async () => {
      try {
        const response = await fetch(`${apiBaseUrl()}/api/v1/events`, { signal: ctrl.signal, headers: { Accept: 'text/event-stream', ...apiKeyHeaders() } })
        const reader = response.body?.getReader()
        if (!reader) return
        const decoder = new TextDecoder()
        let buffer = ''
        for (;;) {
          const { done, value } = await reader.read()
          if (done) break
          buffer += decoder.decode(value, { stream: true })
          const lines = buffer.split('\n')
          buffer = lines.pop() ?? ''
          for (const line of lines) if (line.startsWith('data: ')) {
            try { setEvents(prev => [JSON.parse(line.slice(6)) as AgentEvent, ...prev].slice(0, 5)) } catch { /* ignore malformed event */ }
          }
        }
      } catch { /* aborted or disconnected */ }
      if (!ctrl.signal.aborted) reconnectTimer = setTimeout(connect, 3000)
    }
    connect()
    return () => { ctrl.abort(); clearTimeout(reconnectTimer) }
  }, [])

  const runningSessions = sessions.filter(s => s.status === 'running' || s.active)

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        Dashboard
      </div>
      <div className="p-6 overflow-y-auto flex-1 space-y-6">

        {/* ── Health Check ───────────────────────────────────────────── */}
        <section>
          <h2 className="text-xs text-gray-500 font-medium uppercase tracking-wider mb-3">
            System Health
          </h2>
          <div className="rounded-md border border-white/[0.06] bg-[#141414] p-4">
            {healthLoading ? (
              <div className="flex items-center gap-2 text-xs text-gray-500">
                <div className="w-3 h-3 rounded-full border border-white/10 border-t-white/60 animate-spin" />
                Checking...
              </div>
            ) : healthError ? (
              <div className="flex items-center gap-2 text-xs text-red-400">
                <span className="w-2 h-2 rounded-full bg-red-500" />
                Offline — {healthError}
              </div>
            ) : health ? (
              <div className="flex items-center gap-6 text-xs">
                <div className="flex items-center gap-2">
                  <span className={`w-2 h-2 rounded-full ${health.status === 'ok' ? 'bg-green-500' : 'bg-yellow-500'}`} />
                  <span className="text-gray-300 font-medium">
                    {health.status === 'ok' ? 'Online' : health.status}
                  </span>
                </div>
                <div className="text-gray-500">
                  Active sessions:{' '}
                  <span className="text-gray-300 font-mono">{health.active_sessions}</span>
                </div>
              </div>
            ) : null}
          </div>
        </section>

        {/* ── Active Sessions ───────────────────────────────────────── */}
        <section>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-xs text-gray-500 font-medium uppercase tracking-wider">
              Active Sessions
            </h2>
            <Link href="/" className="text-[10px] text-gray-600 hover:text-gray-400">
              View all
            </Link>
          </div>
          {sessionsLoading ? (
            <div className="flex items-center justify-center py-6">
              <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
            </div>
          ) : runningSessions.length === 0 ? (
            <div className="rounded-md border border-white/[0.06] bg-[#141414] p-4 text-xs text-gray-600">
              No active sessions. Start a new research task from the sidebar.
            </div>
          ) : (
            <div className="space-y-2">
              {runningSessions.map(s => (
                <Link
                  key={s.id}
                  href={`/chat/${s.id}`}
                  className="block rounded-md border border-white/[0.06] bg-[#141414] p-3 hover:bg-white/[0.02] transition-colors"
                >
                  <div className="flex items-center gap-2 mb-1">
                    <span className="w-2 h-2 rounded-full bg-blue-400 animate-pulse" />
                    <span className="text-xs text-gray-200 font-medium truncate">
                      {s.query || 'Untitled'}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 text-[10px] text-gray-500">
                    <span className="font-mono">{s.id.slice(0, 8)}</span>
                    {s.total_agents > 0 && <span>{s.total_agents} agents</span>}
                    {s.total_tokens > 0 && <span>{s.total_tokens} tok</span>}
                    <span className="ml-auto">
                      {new Date(s.created_at).toLocaleString()}
                    </span>
                  </div>
                </Link>
              ))}
            </div>
          )}
        </section>

        {/* ── Recent Events ─────────────────────────────────────────── */}
        <section>
          <h2 className="text-xs text-gray-500 font-medium uppercase tracking-wider mb-3">
            Recent Events
          </h2>
          <div className="rounded-md border border-white/[0.06] bg-[#141414] min-h-[80px]">
            {events.length === 0 ? (
              <div className="flex items-center justify-center py-6 text-xs text-gray-600">
                <span className="inline-block w-2 h-2 rounded-full border border-white/20 border-t-white/60 animate-spin mr-2" />
                Waiting for events...
              </div>
            ) : (
              <div className="divide-y divide-white/[0.03]">
                {events.map((ev, i) => (
                  <div key={i} className="px-4 py-2.5 flex items-start gap-3 text-xs">
                    <span
                      className={`w-1.5 h-1.5 rounded-full mt-1 shrink-0 ${
                        ev.type === 'agent_completed' || ev.type === 'session_completed'
                          ? 'bg-green-500'
                          : ev.type === 'session_failed' || ev.type === 'agent_failed'
                          ? 'bg-red-500'
                          : ev.type === 'agent_spawned' || ev.type === 'session_started'
                          ? 'bg-blue-400'
                          : 'bg-gray-600'
                      }`}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-gray-300 font-mono text-[10px]">{ev.type}</span>
                        {ev.session_id && (
                          <span className="text-gray-600 font-mono text-[10px]">
                            {ev.session_id.slice(0, 8)}
                          </span>
                        )}
                      </div>
                      {ev.agent_id && (
                        <div className="text-gray-500 text-[10px] mt-0.5">
                          agent: {ev.agent_id.slice(0, 8)}
                        </div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </section>

      </div>
    </div>
  )
}