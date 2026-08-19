import { useState, useEffect, useRef } from 'react'
import { connectSSE, api, type SessionSummary, type AgentEvent } from '../lib/api'

interface ConversationProps {
  activeSession: SessionSummary | null
  engineUrl: string | null
}

interface Message {
  id: string
  type: 'user' | 'assistant' | 'system' | 'tool_call'
  content: string
  timestamp: Date
  toolName?: string
  toolStatus?: 'running' | 'completed' | 'failed'
  expandable?: boolean
  expanded?: boolean
  agentId?: string
}

export function Conversation({ activeSession, engineUrl }: ConversationProps) {
  const [messages, setMessages] = useState<Message[]>([])
  const scrollRef = useRef<HTMLDivElement>(null)
  const [autoScroll, setAutoScroll] = useState(true)

  // SSE connection for live events
  useEffect(() => {
    if (!activeSession || !engineUrl) {
      setMessages([])
      return
    }

    let controller: AbortController | undefined

    // Fetch initial results
    api.sessions.results(activeSession.id).then(results => {
        if (results) {
          setMessages([{
            id: 'result',
            type: 'assistant',
            content: typeof results === 'string' ? results : JSON.stringify(results, null, 2),
            timestamp: new Date(),
          }])
        }
      }).catch(() => {
        // No results yet — stream will provide events
      })

    // SSE stream
    controller = connectSSE(
      engineUrl,
      `/api/v1/sessions/${activeSession.id}/events`,
      data => {
        const event = data as AgentEvent
        const msg = eventToMessage(event)
        if (msg) {
          setMessages(prev => [...prev, msg as Message])
        }
      },
      err => {
        setMessages(prev => [...prev, {
          id: `error-${Date.now()}`,
          type: 'system',
          content: `SSE error: ${err}`,
          timestamp: new Date(),
        }])
      },
    )

    return () => {
      controller?.abort()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeSession?.id, engineUrl])

  // Auto-scroll
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [messages, autoScroll])

  const handleScroll = () => {
    if (!scrollRef.current) return
    const el = scrollRef.current
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 70
    setAutoScroll(nearBottom)
  }

  if (!activeSession) {
    return (
      <div className="conversation-view">
        <div className="conversation-empty">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" opacity="0.3">
            <circle cx="12" cy="12" r="10" />
            <path d="M12 8v4M12 16h.01" strokeLinecap="round" />
          </svg>
          <h2>Fathom Desktop</h2>
          <p>
            Select a session from the sidebar or start a new research task.
            The Fathom engine powers your autonomous research agents.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="conversation-view" ref={scrollRef} onScroll={handleScroll}>
      {/* Session header */}
      <div className="message" style={{ opacity: 0.6 }}>
        <div className="message-header">
          <span className="message-role system">Session</span>
          <span className="text-muted" style={{ fontSize: 11 }}>
            {activeSession.id.slice(0, 8)} · {activeSession.status}
          </span>
        </div>
        <div className="message-content">
          <p>{activeSession.query}</p>
        </div>
      </div>

      {/* Messages */}
      {messages.map(msg => (
        <div key={msg.id} className="message">
          <div className="message-header">
            <span className={`message-role ${msg.type}`}>
              {msg.type === 'user' ? 'You' : msg.type === 'assistant' ? 'Agent' : msg.type === 'tool_call' ? msg.toolName : 'System'}
            </span>
            {msg.agentId && (
              <span className="text-muted" style={{ fontSize: 10, fontFamily: 'var(--font-mono)' }}>
                {msg.agentId.slice(0, 6)}
              </span>
            )}
          </div>
          {msg.type === 'tool_call' ? (
            <div className="tool-call">
              <div className="tool-call-header">
                <span className="tool-call-icon">
                  {msg.toolStatus === 'running' ? '⟳' : '✓'}
                </span>
                <span className="tool-call-name">{msg.toolName}</span>
                <span className="tool-call-status">
                  {msg.toolStatus === 'running' ? 'Running...' : msg.toolStatus === 'completed' ? 'Done' : 'Failed'}
                </span>
              </div>
              <div className="tool-call-body">
                {msg.content}
              </div>
            </div>
          ) : (
            <div className="message-content">
              {renderContent(msg.content)}
            </div>
          )}
        </div>
      ))}

      {/* Active session spinner */}
      {activeSession.status === 'running' && (
        <div className="message" style={{ opacity: 0.5 }}>
          <div className="message-header">
            <span className="message-role system">Agent working...</span>
          </div>
          <div className="flex-center gap-4" style={{ justifyContent: 'flex-start', padding: '4px 0' }}>
            <div className="spinner" />
            <span className="text-muted" style={{ fontSize: 12 }}>Researching</span>
          </div>
        </div>
      )}
    </div>
  )
}

function eventToMessage(event: AgentEvent): Message | null {
  switch (event.type) {
    case 'session_started':
      return {
        id: event.id as string || `start-${Date.now()}`,
        type: 'system',
        content: `Session started: ${event.query as string || ''}`,
        timestamp: new Date(),
      }
    case 'agent_spawned':
      return {
        id: `spawn-${event.id as string}`,
        type: 'system',
        content: `Spawning agent: ${event.role as string || ''} — ${(event.task as string || '').slice(0, 200)}`,
        timestamp: new Date(),
        agentId: event.id as string,
      }
    case 'agent_completed':
      return {
        id: `done-${event.id as string}`,
        type: 'system',
        content: `Agent completed: ${(event.summary as string || '').slice(0, 300)} (${event.tokens_used as number || 0} tokens)`,
        timestamp: new Date(),
        agentId: event.id as string,
      }
    case 'agent_failed':
      return {
        id: `fail-${event.id as string}`,
        type: 'system',
        content: `Agent failed: ${event.error as string || ''}`,
        timestamp: new Date(),
        agentId: event.id as string,
      }
    case 'tool_call_started':
      return {
        id: `tool-${event.agent_id as string}-${event.tool as string}-${Date.now()}`,
        type: 'tool_call',
        content: JSON.stringify(event.args, null, 2),
        timestamp: new Date(),
        toolName: event.tool as string,
        toolStatus: 'running',
        agentId: event.agent_id as string,
        expandable: true,
        expanded: false,
      }
    case 'tool_call_completed':
      return {
        id: `tool-done-${event.agent_id as string}-${event.tool as string}-${Date.now()}`,
        type: 'tool_call',
        content: (event.result_preview as string || '').slice(0, 500),
        timestamp: new Date(),
        toolName: event.tool as string,
        toolStatus: 'completed',
        agentId: event.agent_id as string,
        expandable: true,
        expanded: false,
      }
    case 'llm_stream_chunk':
      // Stream chunks are aggregated — skip individual chunks
      return null
    case 'finding':
      return {
        id: `finding-${event.agent_id as string}-${Date.now()}`,
        type: 'system',
        content: `Finding: ${((event.finding as Record<string, unknown>)?.title as string) || ''}`,
        timestamp: new Date(),
        agentId: event.agent_id as string,
      }
    default:
      return null
  }
}

function renderContent(content: string) {
  // Simple markdown-like rendering
  const parts = content.split(/(```[\s\S]*?```|`[^`]+`)/g)
  return parts.map((part, i) => {
    if (part.startsWith('```') && part.endsWith('```')) {
      const code = part.slice(3, -3)
      const lang = code.split('\n')[0]
      const body = code.slice(lang.length).trim()
      return (
        <pre key={i}>
          <code>{body || lang}</code>
        </pre>
      )
    }
    if (part.startsWith('`') && part.endsWith('`')) {
      return <code key={i}>{part.slice(1, -1)}</code>
    }
    // Simple paragraph splitting
    return part.split('\n\n').map((p, j) => (
      <p key={`${i}-${j}`}>{p}</p>
    ))
  })
}