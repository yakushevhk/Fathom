'use client'

import { useEffect, useState } from 'react'
import Link from 'next/link'
import { api, apiBaseUrl, apiKeyHeaders, type Health, type AgentEvent } from '@/lib/api'
import { useSessions } from '@/hooks/useSessions'

export default function HomePage() {
  const [health, setHealth] = useState<Health | null>(null)
  const [healthLoading, setHealthLoading] = useState(true)
  const [healthError, setHealthError] = useState<string | null>(null)
  const [events, setEvents] = useState<AgentEvent[]>([])
  const [eventsConnected, setEventsConnected] = useState(false)
  const [eventsConnecting, setEventsConnecting] = useState(true)
  const { sessions, loading: sessionsLoading, error: sessionsError } = useSessions()

  // Poll engine health so the overview reflects the current control plane state.
  useEffect(() => {
    const load = async () => {
      try {
        const h = await api.health()
        setHealth(h)
        setHealthError(null)
      } catch (e) {
        setHealthError(String(e))
        setHealth(null)
      } finally {
        setHealthLoading(false)
      }
    }
    load()
    const interval = setInterval(load, 10000)
    return () => clearInterval(interval)
  }, [])

  // Keep a small live activity window without making a disconnected stream look busy forever.
  useEffect(() => {
    const ctrl = new AbortController()
    let reconnectTimer: number | undefined
    const connect = async () => {
      if (ctrl.signal.aborted) return
      setEventsConnecting(true)
      try {
        const response = await fetch(`${apiBaseUrl()}/api/v1/events`, {
          signal: ctrl.signal,
          headers: { Accept: 'text/event-stream', ...apiKeyHeaders() },
        })
        if (!response.ok) throw new Error(`events endpoint returned ${response.status}`)
        const reader = response.body?.getReader()
        if (!reader) throw new Error('events stream unavailable')
        setEventsConnected(true)
        setEventsConnecting(false)
        const decoder = new TextDecoder()
        let buffer = ''
        for (;;) {
          const { done, value } = await reader.read()
          if (done) break
          buffer += decoder.decode(value, { stream: true })
          const lines = buffer.split('\n')
          buffer = lines.pop() ?? ''
          for (const line of lines) if (line.startsWith('data: ')) {
            try {
              setEvents(prev => [JSON.parse(line.slice(6)) as AgentEvent, ...prev].slice(0, 5))
            } catch {
              // Ignore malformed individual events and keep the stream alive.
            }
          }
        }
      } catch {
        // Aborted or disconnected; the status below makes this visible to operators.
      }
      if (!ctrl.signal.aborted) {
        setEventsConnected(false)
        setEventsConnecting(false)
        reconnectTimer = window.setTimeout(connect, 3000)
      }
    }
    connect()
    return () => { ctrl.abort(); clearTimeout(reconnectTimer) }
  }, [])

  const runningSessions = sessions.filter(s => s.status === 'running' || s.active)
  const activeWorkers = runningSessions.reduce((total, session) => total + session.total_agents, 0)
  const completedWork = sessions.filter(s => s.status === 'completed').length
  const fleetState = healthError ? 'Offline' : health?.status === 'ok' ? 'Online' : health?.status ?? 'Checking'

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        Fathom Operations Overview
      </div>
      <main className="p-4 sm:p-6 overflow-y-auto flex-1 space-y-6">
        <header className="max-w-3xl">
          <p className="ops-kicker">Control plane</p>
          <h1 className="text-xl sm:text-2xl text-gray-100 font-medium tracking-tight mt-2">Autonomous workers, at a glance</h1>
          <p className="text-sm text-gray-500 mt-2">Monitor remote workers, follow sessions and jobs, and intervene when governed work needs attention.</p>
        </header>

        <section aria-labelledby="fleet-status-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Worker fleet / sessions</p>
              <h2 id="fleet-status-heading">Current operating picture</h2>
            </div>
            <span className={`ops-status ${healthError ? 'ops-status-deny' : health?.status === 'ok' ? 'ops-status-allow' : 'text-gray-500 border-white/10'}`} aria-live="polite">
              {fleetState}
            </span>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
            <div className="ops-panel">
              <span className="ops-label">Workers active</span>
              <strong className="block text-2xl text-gray-100 font-medium mt-2">{sessionsLoading ? '—' : activeWorkers}</strong>
              <span className="text-[11px] text-gray-500">Across running work</span>
            </div>
            <div className="ops-panel">
              <span className="ops-label">Work in progress</span>
              <strong className="block text-2xl text-gray-100 font-medium mt-2">{sessionsLoading ? '—' : runningSessions.length}</strong>
              <span className="text-[11px] text-gray-500">Active sessions</span>
            </div>
            <div className="ops-panel">
              <span className="ops-label">Completed work</span>
              <strong className="block text-2xl text-gray-100 font-medium mt-2">{sessionsLoading ? '—' : completedWork}</strong>
              <span className="text-[11px] text-gray-500">Recorded runs</span>
            </div>
            <div className="ops-panel">
              <span className="ops-label">Live activity</span>
              <strong className="block text-2xl text-gray-100 font-medium mt-2">{events.length}</strong>
              <span className="text-[11px] text-gray-500">{eventsConnected ? 'Streaming now' : eventsConnecting ? 'Connecting' : 'Stream offline'}</span>
            </div>
          </div>
          <div className="ops-panel mt-3" role="status" aria-live="polite">
            {healthLoading ? (
              <span className="text-xs text-gray-500">Checking control plane connection…</span>
            ) : healthError ? (
              <span className="text-xs text-red-400"><span className="font-medium">Engine offline.</span> Reconnect the Fathom service to resume autonomous work.</span>
            ) : health ? (
              <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-xs">
                <span className="flex items-center gap-2 text-gray-300"><span className={`w-2 h-2 rounded-full ${health.status === 'ok' ? 'bg-green-500' : 'bg-yellow-500'}`} />{health.status === 'ok' ? 'Control plane online' : health.status}</span>
                <span className="text-gray-500">Engine reports <span className="text-gray-300 font-mono">{health.active_sessions}</span> active sessions</span>
              </div>
            ) : null}
          </div>
        </section>

        <section aria-labelledby="running-work-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Execution</p>
              <h2 id="running-work-heading">Running sessions</h2>
            </div>
            <div className="flex flex-wrap gap-3 text-[10px]">
              <Link href="/jobs" className="text-gray-500 hover:text-gray-200 underline underline-offset-4">View jobs</Link>
              <Link href="/agents" className="text-gray-500 hover:text-gray-200 underline underline-offset-4">View workers</Link>
            </div>
          </div>
          {sessionsError ? (
            <div className="ops-alert" role="alert"><span>WORK DATA UNAVAILABLE</span>{sessionsError}</div>
          ) : sessionsLoading ? (
            <div className="ops-panel text-xs text-gray-500">Loading running work…</div>
          ) : runningSessions.length === 0 ? (
            <div className="ops-panel text-xs text-gray-500">No sessions are running. Submit a job from <Link href="/jobs" className="text-gray-300 underline underline-offset-4">Work</Link> or dispatch one from the sidebar.</div>
          ) : (
            <div className="space-y-2">
              {runningSessions.map(s => (
                <Link key={s.id} href={`/chat/${s.id}`} aria-label={`Open running work: ${s.query || 'Untitled task'}`} className="block ops-panel hover:bg-white/[0.04] transition-colors">
                  <div className="flex items-start gap-3">
                    <span className="w-2 h-2 rounded-full bg-blue-400 animate-pulse mt-1.5 shrink-0" aria-hidden="true" />
                    <div className="min-w-0 flex-1">
                      <div className="text-sm text-gray-200 font-medium truncate">{s.query || 'Untitled task'}</div>
                      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] text-gray-500 mt-1">
                        <span className="font-mono">{s.id.slice(0, 8)}</span>
                        {s.total_agents > 0 && <span>{s.total_agents} workers</span>}
                        {s.total_tokens > 0 && <span>{s.total_tokens} tokens</span>}
                        <span>{new Date(s.created_at).toLocaleString()}</span>
                      </div>
                    </div>
                    <span className="text-gray-600 text-xs" aria-hidden="true">→</span>
                  </div>
                </Link>
              ))}
            </div>
          )}
        </section>

        <section aria-labelledby="activity-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Events / observability</p>
              <h2 id="activity-heading">Recent activity</h2>
            </div>
            <Link href="/events" className="text-[10px] text-gray-500 hover:text-gray-200 underline underline-offset-4">Open activity</Link>
          </div>
          <div className="ops-panel min-h-[80px]">
            {events.length === 0 ? (
              <div className="py-4 text-xs text-gray-500" role="status" aria-live="polite">
                {eventsConnecting ? 'Connecting to live activity…' : eventsConnected ? 'No activity received yet.' : 'Live activity is disconnected. Reconnecting…'}
              </div>
            ) : (
              <div className="divide-y divide-white/[0.04]">
                {events.map((ev, i) => (
                  <div key={ev.id ?? `${ev.type}-${i}`} className="px-1 py-2.5 flex items-start gap-3 text-xs">
                    <span className={`w-1.5 h-1.5 rounded-full mt-1 shrink-0 ${ev.type === 'agent_completed' || ev.type === 'session_completed' ? 'bg-green-500' : ev.type === 'session_failed' || ev.type === 'agent_failed' ? 'bg-red-500' : ev.type === 'agent_spawned' || ev.type === 'session_started' ? 'bg-blue-400' : 'bg-gray-600'}`} aria-hidden="true" />
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="text-gray-300 font-mono text-[10px]">{ev.type.replace(/_/g, ' ')}</span>
                        {ev.session_id && <span className="text-gray-600 font-mono text-[10px]">work:{ev.session_id.slice(0, 8)}</span>}
                      </div>
                      {ev.agent_id && <div className="text-gray-500 text-[10px] mt-0.5">worker: {ev.agent_id.slice(0, 8)}</div>}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </section>
      </main>
    </div>
  )
}
