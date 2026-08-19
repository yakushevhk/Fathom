import { useState, useRef, useEffect } from 'react'
import type { SessionSummary } from '../lib/api'

interface ComposerProps {
  onSend: (query: string) => void
  activeSession: SessionSummary | null
  onSteer: (instruction: string) => void
  onCancel: () => void
}

export function Composer({ onSend, activeSession, onSteer, onCancel }: ComposerProps) {
  const [input, setInput] = useState('')
  const [expanded, setExpanded] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const isRunning = activeSession?.status === 'running'

  useEffect(() => {
    if (activeSession && !isRunning) {
      textareaRef.current?.focus()
    }
  }, [activeSession?.id, isRunning])

  const handleSubmit = () => {
    const text = input.trim()
    if (!text) return

    if (isRunning) {
      onSteer(text)
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
      <div className="composer">
        <div className="composer-input-wrapper">
          <textarea
            ref={textareaRef}
            className={`composer-input ${expanded ? 'expanded' : ''}`}
            value={input}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            onFocus={handleFocus}
            placeholder={isRunning ? 'Steer the agent...' : 'Research anything...'}
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