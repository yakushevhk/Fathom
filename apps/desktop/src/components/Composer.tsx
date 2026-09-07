import { useState, useRef, useEffect } from 'react'
import type { SessionSummary } from '../lib/api'
import { QueuedComposerMessages, type QueuedMessage } from './QueuedComposerMessages'

interface ComposerProps {
  onSend: (query: string) => void
  activeSession: SessionSummary | null
  onSteer: (instruction: string) => void
  onCancel: () => void
}

export function Composer({ onSend, activeSession, onSteer, onCancel }: ComposerProps) {
  const [input, setInput] = useState('')
  const [expanded, setExpanded] = useState(false)
  const [queuedMessages, setQueuedMessages] = useState<QueuedMessage[]>([])
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const isRunning = activeSession?.status === 'running'
  const wasRunningRef = useRef(isRunning)
  useEffect(() => {
    if (activeSession && !isRunning) {
      textareaRef.current?.focus()
    }
  }, [activeSession?.id, isRunning])

  // Auto-drain queue when active session finishes running
  useEffect(() => {
    if (wasRunningRef.current && !isRunning && queuedMessages.length > 0) {
      const [head, ...rest] = queuedMessages
      setQueuedMessages(rest)
      if (head) {
        onSend(head.text)
      }
    }
    wasRunningRef.current = isRunning
  }, [isRunning, queuedMessages, onSend])

  const handleSubmit = () => {
    const text = input.trim()
    if (!text) return

    if (isRunning) {
      // If running, queue the message with instant Steer option (OpenMausBot pattern)
      setQueuedMessages(prev => [
        ...prev,
        { id: `queue-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`, text, timestamp: new Date() },
      ])
    } else {
      onSend(text)
    }
    setInput('')
    setExpanded(false)
  }
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSubmit()
    }
  }

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value)
    // Auto-expand
    if (e.target.value.length > 80) {
      setExpanded(true)
    }
  }

  const handleFocus = () => {
    if (input.length > 80) {
      setExpanded(true)
    }
  }

  const handleCancel = () => {
    onCancel()
  }

  return (
    <div className="composer-container">
      {queuedMessages.length > 0 && (
        <QueuedComposerMessages
          items={queuedMessages}
          isRunning={!!isRunning}
          onSteerNow={(text) => {
            onSteer(text)
            setQueuedMessages(prev => prev.filter(m => m.text !== text))
          }}
          onSteerAll={() => {
            const combined = queuedMessages.map(m => m.text).join('\n---\n')
            onSteer(combined)
            setQueuedMessages([])
          }}
          onRemove={(id) => {
            setQueuedMessages(prev => prev.filter(m => m.id !== id))
          }}
          onClear={() => setQueuedMessages([])}
        />
      )}
      <div className="composer">
        <div className="composer-input-wrapper">
          <textarea
            ref={textareaRef}
            className={`composer-input ${expanded ? 'expanded' : ''}`}
            value={input}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            onFocus={handleFocus}
            placeholder={isRunning ? 'Steer your worker...' : 'Give your worker a task...'}
            rows={expanded ? 4 : 1}
          />
        </div>
        <div className="composer-actions">
          {isRunning ? (
            <>
              <button className="composer-btn primary" onClick={handleSubmit} disabled={!input.trim()}>
                Steer
              </button>
              <button className="composer-btn danger" onClick={handleCancel}>
                Stop
              </button>
            </>
          ) : (
            <button className="composer-btn primary" onClick={handleSubmit} disabled={!input.trim()}>
              Send
            </button>
          )}
        </div>
      </div>
    </div>
  )
}