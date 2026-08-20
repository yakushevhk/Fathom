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
  const scrollRef = useRef<HTMLDivElement>(null)
  const [autoScroll, setAutoScroll] = useState(true)

  // SSE connection
  useEffect(() => {
    const ctrl = new AbortController()
    let reconnecting = false

    const connect = async () => {
      try {
        const res = await fetch(api.events.globalUrl(), {
          signal: ctrl.signal,
          headers: { Accept: 'text/event-stream', ...apiKeyHeaders() },
        })
        setConnected(true)
        const reader = res.body?.getReader()
        if (!reader) return
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
              const raw = JSON.parse(line.slice(6)) as AgentEvent
              setEvents(prev => {
                const display = eventToDisplay(raw)
                // Deduplicate by id
                if (display.id && prev.some(e => e.id === display.id)) return prev
                return [...prev, display]
              })
            }
          }
        }
      } catch {
        // aborted or connection lost
      }
      if (!ctrl.signal.aborted) {
        setConnected(false)
        // Reconnect after 2s
        if (!reconnecting) {
          reconnecting = true
          setTimeout(() => { reconnecting = false; connect() }, 2000)
        }
      }
    }

    connect()
    return () => { ctrl.abort() }
  }, [])

  // Auto-scroll
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [events, autoScroll])

  const handleScroll = () => {
    const el = scrollRef.current
    if (!el) return
    setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 70)
  }

  const handleClear = () => setEvents([])

  return (
    <div className="flex-1 flex flex-col min-w-0">
      {/* Header */}
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        <span className="text-gray-500 mr-2">Global Events</span>
        <span className={`w-1.5 h-1.5 rounded-full mr-1.5 ${connected ? 'bg-green-500' : 'bg-red-500'}`} />
        <span className="text-gray-500">{connected ? 'Connected' : 'Disconnected'}</span>
        <span className="ml-auto text-gray-500">{events.length} events</span>
        {events.length > 0 && (
          <button
            onClick={handleClear}
            className="ml-3 px-2 py-0.5 rounded text-[10px] bg-white/[0.06] hover:bg-white/[0.1] text-gray-400 hover:text-gray-200 transition-colors"
          >
            Clear
          </button>
        )}
      </div>

      {/* Event list */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto"
      >
        {events.length === 0 && (
          <div className="flex items-center justify-center py-12 text-gray-600 text-xs">
            {connected ? 'Waiting for events...' : 'Connecting...'}
          </div>
        )}
        <div className="divide-y divide-white/[0.03]">
          {events.map(e => (
            <div key={e.id} className="px-4 py-2.5 hover:bg-white/[0.01] transition-colors animate-fade-in">
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
                <span className="ml-auto text-[10px] text-gray-600">
                  {e.timestamp.toLocaleTimeString()}
                </span>
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
  const id = `${event.type}-${event.id ?? event.agent_id ?? event.request_id ?? 'event'}`
  const sessionId = event.session_id as string | undefined
  const lifecycleEvent = event.type === 'agent_spawned' || event.type === 'agent_state_changed' || event.type === 'agent_completed' || event.type === 'agent_failed'
  const agentId = (event.agent_id ?? (lifecycleEvent ? event.id : undefined)) as string | undefined

  let summary = ''
  let detail = ''

  switch (event.type) {
    case 'session_started':
      summary = `Session started: ${String(event.query ?? '').slice(0, 200)}`
      break
    case 'session_completed':
      summary = `Session completed — ${event.total_agents ?? 0} agents, ${event.total_tokens ?? 0} tokens`
      break
    case 'session_failed':
      summary = `Session failed: ${event.error ?? ''}`
      break
    case 'agent_spawned':
      summary = `Agent spawned: ${event.role as string}`
      detail = String(event.task ?? '').slice(0, 300)
      break
    case 'agent_completed':
      summary = `Agent completed: ${String(event.id ?? '').slice(0, 12)}`
      detail = String(event.summary ?? '').slice(0, 300)
      break
    case 'agent_failed':
      summary = `Agent failed: ${String(event.id ?? '').slice(0, 12)}`
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

  return { id, type: event.type, sessionId, agentId, summary, detail, timestamp: new Date() }
}