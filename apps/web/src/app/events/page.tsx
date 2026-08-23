'use client'

import { useEffect, useRef, useState } from 'react'
import { api, apiKeyHeaders, type AgentEvent } from '@/lib/api'

interface DisplayEvent {
  id: string
  type: string
  sessionId?: string
  agentId?: string
  summary: string
  detail: string
  timestamp: Date
}

export default function EventsPage() {
  const [events, setEvents] = useState<DisplayEvent[]>([])
  const [connected, setConnected] = useState(false)
  const [connectionError, setConnectionError] = useState<string | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const reconnectTimer = useRef<number | null>(null)
  const [autoScroll, setAutoScroll] = useState(true)
  const [paused, setPaused] = useState(false)
  const [liveAnnouncement, setLiveAnnouncement] = useState('')
  const pausedRef = useRef(false)
  const lastAnnouncedEventRef = useRef<string | null>(null)
  const announcementTimer = useRef<number | null>(null)
  const pendingAnnouncement = useRef<{ count: number; latest: string }>({ count: 0, latest: '' })
  const MAX_EVENTS = 500

  const queueLiveAnnouncement = (event: DisplayEvent) => {
    pendingAnnouncement.current = {
      count: pendingAnnouncement.current.count + 1,
      latest: event.summary,
    }
    if (announcementTimer.current !== null) return
    announcementTimer.current = window.setTimeout(() => {
      const pending = pendingAnnouncement.current
      setLiveAnnouncement(pending.count === 1
        ? `New worker event: ${pending.latest}`
        : `${pending.count} new worker events. Latest: ${pending.latest}`)
      pendingAnnouncement.current = { count: 0, latest: '' }
      announcementTimer.current = null
    }, 1500)
  }

  // SSE connection
  useEffect(() => {
    const ctrl = new AbortController()

    const connect = async (): Promise<void> => {
      if (ctrl.signal.aborted) return
      try {
        const res = await fetch(api.events.globalUrl(), {
          signal: ctrl.signal,
          headers: { Accept: 'text/event-stream', ...apiKeyHeaders() },
        })
        if (!res.ok) throw new Error(`Event stream returned ${res.status}`)
        setConnected(true)
        setConnectionError(null)
        const reader = res.body?.getReader()
        if (!reader) throw new Error('Event stream has no response body')
        const decoder = new TextDecoder()
        let buffer = ''
        for (;;) {
          const { done, value } = await reader.read()
          if (done) break
          buffer += decoder.decode(value, { stream: true })
          const lines = buffer.split('\n')
          buffer = lines.pop() ?? ''
          for (const line of lines) {
            if (!line.startsWith('data: ')) continue
            try {
              const raw = JSON.parse(line.slice(6)) as AgentEvent
              const display = eventToDisplay(raw)
              if (pausedRef.current) continue
              if (lastAnnouncedEventRef.current !== display.id) {
                lastAnnouncedEventRef.current = display.id
                queueLiveAnnouncement(display)
              }
              setEvents(prev => {
                if (prev.some(e => e.id === display.id)) return prev
                const next = [...prev, display]
                return next.length > MAX_EVENTS ? next.slice(-MAX_EVENTS) : next
              })
            } catch {
              // Ignore malformed events while keeping the stream alive.
            }
          }
        }
      } catch (e) {
        if (ctrl.signal.aborted) return
        setConnectionError(e instanceof Error ? e.message : 'Event stream unavailable')
      } finally {
        if (ctrl.signal.aborted) return
        setConnected(false)
        reconnectTimer.current = window.setTimeout(() => {
          reconnectTimer.current = null
          void connect()
        }, 2000)
      }
    }

    void connect()
    return () => {
      ctrl.abort()
      if (reconnectTimer.current !== null) window.clearTimeout(reconnectTimer.current)
      reconnectTimer.current = null
    }
  }, [])

  // Auto-scroll
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [events, autoScroll])

  useEffect(() => () => {
    if (announcementTimer.current !== null) window.clearTimeout(announcementTimer.current)
  }, [])

  const handleScroll = () => {
    const el = scrollRef.current
    if (!el) return
    setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 70)
  }

  const handleClear = () => setEvents([])
  const handlePause = () => {
    const next = !pausedRef.current
    pausedRef.current = next
    setPaused(next)
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      {/* Header */}
      <div className="ops-toolbar shrink-0 flex-wrap">
        <h1 id="events-heading" className="ops-toolbar-title">Worker events</h1>
        <span aria-hidden="true" className={`w-1.5 h-1.5 rounded-full mr-1.5 ${connected ? 'bg-green-500' : 'bg-red-500'}`} />
        <span role="status" aria-live="polite" className="text-gray-500">{paused ? 'Paused' : connected ? 'Live' : connectionError ? 'Offline' : 'Connecting'}</span>
        <span className="ops-toolbar-meta w-full sm:w-auto sm:ml-auto">{events.length} events · max {MAX_EVENTS}</span>
        <button type="button" onClick={handlePause} className="ops-button-secondary px-2 py-0.5 text-[10px]" aria-pressed={paused}>
          {paused ? 'Resume' : 'Pause'}
        </button>
        {events.length > 0 && (
          <button
            onClick={handleClear}
            type="button"
            aria-label="Clear worker events"
            className="ml-2 px-2 py-0.5 rounded text-[10px] bg-white/[0.06] hover:bg-white/[0.1] text-gray-400 hover:text-gray-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-300 transition-colors"
          >
            Clear events
          </button>
        )}
      </div>

      {/* Event list */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto"
        role="log"
        aria-labelledby="events-heading"
        aria-live="off"
        aria-relevant="additions"
        tabIndex={0}
      >
        <div className="sr-only" role="status" aria-live="polite" aria-atomic="true">{liveAnnouncement}</div>
        {events.length === 0 && (
          <div role={connectionError ? 'alert' : 'status'} className="flex items-center justify-center py-12 text-gray-600 text-xs">
            {connectionError ? `Activity unavailable: ${connectionError}` : paused ? 'Stream paused. Resume to receive new worker events.' : connected ? 'Waiting for worker events…' : 'Connecting to the live worker event stream…'}
          </div>
        )}
        <div className="divide-y divide-white/[0.03]" role="list" aria-label="Worker event entries">
          {events.map(e => (
            <div key={e.id} role="listitem" tabIndex={0} aria-label={`${e.type.replace(/_/g, ' ')}: ${e.summary}`} className="px-4 py-2.5 hover:bg-white/[0.01] transition-colors animate-fade-in focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-500">
              <div className="flex items-center gap-2 mb-1">
                <span className={`text-[10px] font-semibold uppercase tracking-wider ${
                  e.type === 'session_started' ? 'text-blue-400' :
                  e.type === 'session_completed' ? 'text-green-500' :
                  e.type === 'session_failed' ? 'text-red-500' :
                  e.type === 'agent_spawned' ? 'text-purple-400' :
                  e.type === 'agent_completed' ? 'text-green-400' :
                  e.type === 'agent_failed' ? 'text-red-400' :
                  e.type === 'tool_call_started' ? 'text-yellow-400' :
                  e.type === 'tool_call_completed' ? 'text-yellow-300' :
                  e.type === 'finding' ? 'text-cyan-400' :
                  'text-gray-500'
                }`}>
                  {e.type.replace(/_/g, ' ')}
                </span>
                {e.sessionId && (
                  <span className="text-[9px] text-gray-700 font-mono">sess:{e.sessionId.slice(0, 6)}</span>
                )}
                {e.agentId && (
                  <span className="text-[9px] text-gray-700 font-mono">agent:{e.agentId.slice(0, 6)}</span>
                )}
                <time className="ml-auto text-[10px] text-gray-600" dateTime={e.timestamp.toISOString()}>
                  {e.timestamp.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                </time>
              </div>
              <div className="text-xs text-gray-300 leading-relaxed">{e.summary}</div>
              {e.detail && (
                <div className="mt-1 text-[11px] text-gray-500 font-mono whitespace-pre-wrap truncate max-w-full">{e.detail}</div>
              )}
            </div>
          ))}
        </div>
        <div className="h-2" />
      </div>
    </div>
  )
}

function eventToDisplay(event: AgentEvent): DisplayEvent {
  const identity = event.id ?? event.request_id ?? event.agent_id ?? event.sequence ?? event.timestamp ?? event.created_at ?? JSON.stringify(event)
  const id = `${event.type}-${String(identity).slice(0, 120)}`
  const sessionId = event.session_id as string | undefined
  const lifecycleEvent = event.type === 'agent_spawned' || event.type === 'agent_state_changed' || event.type === 'agent_completed' || event.type === 'agent_failed'
  const agentId = (event.agent_id ?? (lifecycleEvent ? event.id : undefined)) as string | undefined

  let summary = ''
  let detail = ''

  switch (event.type) {
    case 'session_started':
      summary = `Session started: ${String(event.query ?? '').slice(0, 200)}`
      break
    case 'session_completed': {
      const totalAgents = event.total_agents
      const totalTokens = event.total_tokens
      const metrics = [
        typeof totalAgents === 'number' ? `${totalAgents} worker${totalAgents === 1 ? '' : 's'}` : null,
        typeof totalTokens === 'number' ? `${totalTokens.toLocaleString()} tokens` : null,
      ].filter(Boolean).join(', ')
      summary = metrics ? `Session completed — ${metrics}` : 'Session completed'
      break
    }
    case 'session_failed':
      summary = `Session failed: ${event.error ?? ''}`
      break
    case 'agent_spawned':
      summary = `Worker started: ${event.role as string}`
      detail = String(event.task ?? '').slice(0, 300)
      break
    case 'agent_completed':
      summary = `Worker completed: ${String(event.id ?? '').slice(0, 12)}`
      detail = String(event.summary ?? '').slice(0, 300)
      break
    case 'agent_failed':
      summary = `Worker failed: ${String(event.id ?? '').slice(0, 12)}`
      detail = String(event.error ?? '')
      break
    case 'tool_call_started':
      summary = `Tool call: ${event.tool as string}`
      detail = JSON.stringify(event.args, null, 2).slice(0, 300)
      break
    case 'tool_call_completed':
      summary = `Tool completed: ${event.tool as string}`
      detail = String(event.result_preview ?? '').slice(0, 300)
      break
    case 'finding':
      summary = `Finding: ${((event.finding as Record<string, unknown>)?.title ?? '') as string}`
      detail = JSON.stringify(event.finding, null, 2).slice(0, 300)
      break
    case 'question_asked':
      summary = `Question: ${event.question as string}`
      break
    case 'approval_requested':
      summary = `Approval needed: ${event.tool as string}`
      detail = String(event.args_preview ?? '').slice(0, 200)
      break
    default:
      summary = `Event: ${event.type}`
      detail = JSON.stringify(event, null, 2).slice(0, 300)
  }

  const rawTimestamp = event.timestamp ?? event.created_at
  const eventTimestamp = typeof rawTimestamp === 'string' || typeof rawTimestamp === 'number'
    ? new Date(rawTimestamp)
    : new Date()
  return { id, type: event.type, sessionId, agentId, summary, detail, timestamp: Number.isNaN(eventTimestamp.getTime()) ? new Date() : eventTimestamp }
}