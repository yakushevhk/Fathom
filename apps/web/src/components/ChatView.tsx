'use client'

import { useEffect, useRef, useState } from 'react'
import { api, apiKeyHeaders, type AgentEvent, type SessionResults } from '@/lib/api'
import { Markdown } from './Markdown'

interface ChatViewProps {
  sessionId: string
}

interface Message {
  id: string
  type: 'user' | 'assistant' | 'system' | 'tool_call'
  content: string
  timestamp: Date
  toolName?: string
  toolStatus?: 'running' | 'completed' | 'failed'
  agentId?: string
}

interface PendingQuestion {
  requestId: string
  agentId: string
  question: string
}

interface PendingApproval {
  requestId: string
  agentId: string
  tool: string
  argsPreview: string
}

export function ChatView({ sessionId }: ChatViewProps) {
  const [messages, setMessages] = useState<Message[]>([])
  const [input, setInput] = useState('')
  const [sending, setSending] = useState(false)
  const [question, setQuestion] = useState<PendingQuestion | null>(null)
  const [approval, setApproval] = useState<PendingApproval | null>(null)
  const [answer, setAnswer] = useState('')
  const [results, setResults] = useState<SessionResults | null>(null)
  const [streamStatus, setStreamStatus] = useState<'connecting' | 'live' | 'offline'>('connecting')
  const scrollRef = useRef<HTMLDivElement>(null)
  const [autoScroll, setAutoScroll] = useState(true)

  // SSE stream
  useEffect(() => {
    const ctrl = new AbortController()
    let retryTimer: number | null = null
    const connect = async (): Promise<void> => {
      if (ctrl.signal.aborted) return
      try {
        const res = await fetch(api.events.sessionUrl(sessionId), {
          signal: ctrl.signal,
          headers: { Accept: 'text/event-stream', ...apiKeyHeaders() },
        })
        if (!res.ok) throw new Error(`Event stream returned ${res.status}`)
        const reader = res.body?.getReader()
        if (!reader) throw new Error('Event stream has no response body')
        setStreamStatus('live')
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
            try { handleEvent(JSON.parse(line.slice(6)) as AgentEvent) } catch { /* keep stream alive */ }
          }
        }
      } catch {
        if (ctrl.signal.aborted) return
        setStreamStatus('offline')
      } finally {
        if (ctrl.signal.aborted) return
        setStreamStatus('offline')
        retryTimer = window.setTimeout(() => {
          retryTimer = null
          setStreamStatus('connecting')
          void connect()
        }, 3000)
      }
    }

    const handleEvent = (event: AgentEvent) => {
      switch (event.type) {
        case 'question_asked':
          setQuestion({
            requestId: event.request_id as string,
            agentId: event.agent_id as string,
            question: event.question as string,
          })
          setMessages(prev => [...prev, {
            id: `question-${event.request_id}`,
            type: 'system',
            content: `❓ Worker needs an operator answer: ${event.question as string}`,
            timestamp: new Date(),
            agentId: event.agent_id as string,
          }])
          break
        case 'approval_requested':
          setApproval({
            requestId: event.request_id as string,
            agentId: event.agent_id as string,
            tool: event.tool as string,
            argsPreview: event.args_preview as string,
          })
          setMessages(prev => [...prev, {
            id: `approval-${event.request_id}`,
            type: 'system',
            content: `🔒 ${event.tool as string} requires approval`,
            timestamp: new Date(),
            agentId: event.agent_id as string,
          }])
          break
        case 'session_completed':
        case 'session_failed':
          setMessages(prev => [...prev, {
            id: `end-${Date.now()}`,
            type: 'system',
            content: event.type === 'session_completed'
              ? `✅ Session completed — ${event.total_agents} workers, ${event.total_tokens} tokens`
              : `❌ Session failed: ${event.error as string}`,
            timestamp: new Date(),
          }])
          // Load final results
          api.sessions.results(sessionId).then(setResults).catch(() => {})
          break
        default: {
          const msg = eventToMessage(event)
          if (msg) setMessages(prev => [...prev, msg])
        }
      }
    }

    void connect()
    return () => {
      ctrl.abort()
      if (retryTimer !== null) window.clearTimeout(retryTimer)
      retryTimer = null
    }
  }, [sessionId])

  // Auto-scroll
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [messages, results, autoScroll])

  const handleScroll = () => {
    const el = scrollRef.current
    if (!el) return
    setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 70)
  }

  const handleSend = async () => {
    const text = input.trim()
    if (!text || sending) return
    setSending(true)
    setInput('')
    setMessages(prev => [...prev, {
      id: `user-${Date.now()}`,
      type: 'user',
      content: text,
      timestamp: new Date(),
    }])
    try {
      await api.sessions.steer(sessionId, text)
    } catch (e) {
      setMessages(prev => [...prev, {
        id: `error-${Date.now()}`,
        type: 'system',
        content: `Steer failed: ${e}`,
        timestamp: new Date(),
      }])
    } finally {
      setSending(false)
    }
  }

  const submitAnswer = async () => {
    if (!question) return
    const text = answer.trim()
    if (!text) return
    try {
      await api.sessions.answer(sessionId, question.requestId, text)
      setQuestion(null)
      setAnswer('')
    } catch (e) {
      setMessages(prev => [...prev, {
        id: `err-${Date.now()}`,
        type: 'system',
        content: `Answer failed: ${e}`,
        timestamp: new Date(),
      }])
    }
  }

  const submitApproval = async (approved: boolean) => {
    if (!approval) return
    try {
      await api.sessions.approve(sessionId, approval.requestId, approved)
      setApproval(null)
    } catch (e) {
      setMessages(prev => [...prev, {
        id: `err-${Date.now()}`,
        type: 'system',
        content: `Approval failed: ${e}`,
        timestamp: new Date(),
      }])
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      {/* Header */}
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        <SessionHeader sessionId={sessionId} />
        <span role="status" className="ml-3 inline-flex items-center gap-1 text-[10px] text-gray-500">
          <span aria-hidden="true" className={`h-1.5 w-1.5 rounded-full ${streamStatus === 'live' ? 'bg-green-500' : streamStatus === 'offline' ? 'bg-red-500' : 'bg-yellow-400 animate-pulse'}`} />
          {streamStatus === 'live' ? 'Live' : streamStatus === 'offline' ? 'Offline — retrying' : 'Connecting'}
        </span>
      </div>

      {/* Pending question banner */}
      {question && (
        <div className="border-b border-blue-500/20 bg-blue-500/5 p-3 animate-fade-in shrink-0">
          <div className="text-xs text-blue-300 font-medium mb-2">Worker needs your input</div>
          <div className="text-sm text-gray-200 mb-2">{question.question}</div>
          <div className="flex gap-2">
            <input
              value={answer}
              onChange={e => setAnswer(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') submitAnswer() }}
              aria-label="Answer worker question"
              placeholder="Your answer..."
              autoFocus
              className="flex-1 p-2 rounded-md bg-[#141414] border border-blue-500/30 text-sm text-gray-200 placeholder-gray-600 outline-none focus:border-blue-400"
            />
            <button type="button" onClick={submitAnswer} disabled={!answer.trim()}
              className="px-4 py-2 rounded-md bg-blue-500 text-black text-xs font-semibold hover:bg-blue-400 focus-visible:outline focus-visible:outline-2 focus-visible:outline-blue-300 disabled:opacity-30 transition-colors">
              Answer
            </button>
          </div>
        </div>
      )}

      {/* Pending approval banner */}
      {approval && (
        <div className="border-b border-yellow-500/20 bg-yellow-500/5 p-3 animate-fade-in shrink-0">
          <div className="text-xs text-yellow-300 font-medium mb-1">Worker action needs operator approval</div>
          <div className="text-sm text-gray-200 font-mono mb-1">{approval.tool}</div>
          <div className="text-xs text-gray-400 font-mono bg-black/40 rounded p-2 mb-2 whitespace-pre-wrap max-h-20 overflow-y-auto">{approval.argsPreview}</div>
          <div className="flex gap-2">
            <button type="button" aria-label={`Allow ${approval.tool}`} onClick={() => submitApproval(true)}
              className="px-4 py-1.5 rounded-md bg-green-600 text-black text-xs font-semibold hover:bg-green-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-green-300 transition-colors">
              Allow
            </button>
            <button type="button" aria-label={`Deny ${approval.tool}`} onClick={() => submitApproval(false)}
              className="px-4 py-1.5 rounded-md bg-red-600 text-white text-xs font-semibold hover:bg-red-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-red-300 transition-colors">
              Deny
            </button>
          </div>
        </div>
      )}

      {/* Messages */}
      <div ref={scrollRef} onScroll={handleScroll} className="flex-1 overflow-y-auto px-4 py-2">
        {messages.map(msg => (
          <div key={msg.id} className="py-2 animate-fade-in">
            <div className="flex items-center gap-2 mb-1">
              <span className={`text-[10px] font-semibold uppercase tracking-wider ${
                msg.type === 'user' ? 'text-blue-400' : msg.type === 'assistant' ? 'text-gray-400' : 'text-gray-600'
              }`}>
                {msg.type === 'user' ? 'You' : msg.type === 'assistant' ? 'Worker' : msg.type === 'tool_call' ? (msg.toolName ?? 'Tool') : 'System'}
              </span>
              {msg.agentId && <span className="text-[9px] text-gray-700 font-mono">{msg.agentId.slice(0, 6)}</span>}
            </div>
            {msg.type === 'tool_call' ? (
              <div className="rounded-md border border-white/[0.06] bg-[#141414] overflow-hidden">
                <div className="flex items-center gap-2 px-2.5 py-1.5 text-xs text-gray-400">
                  <span className={msg.toolStatus === 'running' ? 'text-blue-400' : 'text-green-500'}>
                    {msg.toolStatus === 'running' ? '⟳' : '✓'}
                  </span>
                  <span className="font-mono text-[11px]">{msg.toolName}</span>
                  <span className="ml-auto text-[10px]">{msg.toolStatus === 'running' ? 'Running...' : 'Done'}</span>
                </div>
                <div className="px-2.5 py-1.5 border-t border-white/[0.06] text-xs text-gray-500 font-mono whitespace-pre-wrap max-h-32 overflow-y-auto">
                  {msg.content.slice(0, 500)}
                </div>
              </div>
            ) : (
              <Markdown>{msg.content}</Markdown>
            )}
          </div>
        ))}

        {/* Results */}
        {results && (
          <div className="py-4 animate-fade-in">
            <div className="text-[10px] font-semibold uppercase tracking-wider text-green-500 mb-2">Session result</div>
            <div className="rounded-lg border border-white/[0.08] bg-[#141414] p-4">
              <Markdown className="text-sm">{results.summary}</Markdown>
              {results.findings.length > 0 && (
                <div className="mt-4 border-t border-white/[0.06] pt-3">
                  <div className="text-[10px] font-semibold uppercase tracking-wider text-gray-500 mb-2">
                    {results.findings.length} findings
                  </div>
                  <div className="space-y-2">
                    {results.findings.map(f => (
                      <details key={f.file} className="rounded-md border border-white/[0.06] bg-black/30">
                        <summary className="px-2.5 py-1.5 text-xs text-gray-400 font-mono cursor-pointer hover:text-gray-200">{f.file}</summary>
                        <div className="px-2.5 pb-2.5 text-xs text-gray-300 whitespace-pre-wrap max-h-64 overflow-y-auto">{f.content}</div>
                      </details>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
        <div className="h-2" />
      </div>

      {/* Composer */}
      <div className="border-t border-white/[0.06] p-3 shrink-0">
        <div className="flex gap-2">
          <input
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && (e.ctrlKey || e.metaKey || !e.shiftKey)) {
                e.preventDefault()
                handleSend()
              }
            }}
            aria-label="Send a message to the worker"
            placeholder="Send guidance or steer this worker…"
            className="flex-1 p-2.5 rounded-md bg-[#141414] border border-white/[0.06] text-sm text-gray-200 placeholder-gray-600 outline-none focus:border-gray-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-300 transition-colors"
          />
          <button
            type="button"
            onClick={handleSend}
            disabled={!input.trim() || sending}
            className="px-4 py-2 rounded-md bg-gray-600 text-black text-xs font-semibold hover:bg-gray-400 focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-300 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          >
            {sending ? '...' : 'Send'}
          </button>
        </div>
      </div>
    </div>
  )
}

function SessionHeader({ sessionId }: { sessionId: string }) {
  return (
    <>
      <span className="text-gray-500 mr-2">Worker session</span>
      <span className="font-mono text-gray-300">{sessionId.slice(0, 8)}</span>
      <a
        href={`${typeof window !== 'undefined' ? localStorage.getItem('fathom_base_url') || 'http://127.0.0.1:8080' : ''}/`}
        target="_blank"
        rel="noreferrer"
        className="ml-auto text-gray-500 hover:text-gray-300"
      >
        Dashboard ↗
      </a>
    </>
  )
}

function eventToMessage(event: AgentEvent): Message | null {
  switch (event.type) {
    case 'session_started':
      return { id: `start-${event.id}`, type: 'system', content: `Session started: ${event.query ?? ''}`, timestamp: new Date() }
    case 'agent_spawned':
      return { id: `spawn-${event.id}`, type: 'system', content: `Starting worker ${event.role} — ${String(event.task ?? '').slice(0, 200)}`, timestamp: new Date(), agentId: event.id as string }
    case 'agent_completed':
      return { id: `done-${event.id}`, type: 'system', content: `Worker completed: ${String(event.summary ?? '').slice(0, 300)} (${event.tokens_used ?? 0} tokens)`, timestamp: new Date(), agentId: event.id as string }
    case 'agent_failed':
      return { id: `fail-${event.id}`, type: 'system', content: `Worker failed: ${event.error ?? ''}`, timestamp: new Date(), agentId: event.id as string }
    case 'tool_call_started':
      return { id: `tool-${event.agent_id}-${event.tool}-${event.request_id ?? event.id ?? Date.now()}`, type: 'tool_call', content: JSON.stringify(event.args, null, 2), timestamp: new Date(), toolName: event.tool as string, toolStatus: 'running', agentId: event.agent_id as string }
    case 'tool_call_completed':
      return { id: `tool-done-${event.agent_id}-${event.tool}-${event.request_id ?? event.id ?? Date.now()}`, type: 'tool_call', content: String(event.result_preview ?? '').slice(0, 500), timestamp: new Date(), toolName: event.tool as string, toolStatus: 'completed', agentId: event.agent_id as string }
    case 'finding':
      return { id: `finding-${event.agent_id}-${event.id ?? event.request_id ?? Date.now()}`, type: 'system', content: `Finding: ${(event.finding as Record<string, unknown>)?.title ?? ''}`, timestamp: new Date(), agentId: event.agent_id as string }
    default:
      return null
  }
}