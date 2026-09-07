import { useState, useRef, useEffect } from 'react'
import type { SessionSummary } from '../lib/api'
import { QueuedComposerMessages, type QueuedMessage } from './QueuedComposerMessages'
import { ComposerAttachments, type ComposerAttachment } from './ComposerAttachments'
import { Paperclip, Mic, MicOff } from 'lucide-react'
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
  const [attachments, setAttachments] = useState<ComposerAttachment[]>([])
  const [isDragging, setIsDragging] = useState(false)
  const [isListening, setIsListening] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const recognitionRef = useRef<unknown>(null)
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

  // Initialize Web Speech Recognition for hands-free dictation
  useEffect(() => {
    if (typeof window !== 'undefined') {
      interface SpeechWindow extends Window {
        SpeechRecognition?: new () => {
          continuous: boolean
          interimResults: boolean
          lang: string
          start: () => void
          stop: () => void
          onresult: (e: { resultIndex: number; results: Array<Array<{ transcript: string }> & { isFinal?: boolean }> }) => void
          onerror: () => void
          onend: () => void
        }
        webkitSpeechRecognition?: new () => {
          continuous: boolean
          interimResults: boolean
          lang: string
          start: () => void
          stop: () => void
          onresult: (e: { resultIndex: number; results: Array<Array<{ transcript: string }> & { isFinal?: boolean }> }) => void
          onerror: () => void
          onend: () => void
        }
      }
      const win = window as unknown as SpeechWindow
      const SpeechConstructor = win.SpeechRecognition || win.webkitSpeechRecognition
      if (SpeechConstructor) {
        const recognition = new SpeechConstructor()
        recognition.continuous = true
        recognition.interimResults = true
        recognition.lang = 'en-US'

        recognition.onresult = (e) => {
          let final = ''
          for (let i = e.resultIndex; i < e.results.length; ++i) {
            if (e.results[i]?.isFinal) {
              final += e.results[i]?.[0]?.transcript || ''
            }
          }
          if (final) {
            setInput(prev => (prev ? `${prev} ${final.trim()}` : final.trim()))
          }
        }

        recognition.onerror = () => setIsListening(false)
        recognition.onend = () => setIsListening(false)
        recognitionRef.current = recognition
      }
    }
    return () => {
      const rec = recognitionRef.current as { stop: () => void } | null
      rec?.stop()
    }
  }, [])

  const toggleListening = () => {
    const rec = recognitionRef.current as { start: () => void; stop: () => void } | null
    if (!rec) return
    if (isListening) {
      rec.stop()
      setIsListening(false)
    } else {
      try {
        rec.start()
        setIsListening(true)
      } catch {
        setIsListening(false)
      }
    }
  }

  const handleFiles = (files: FileList | File[]) => {
    Array.from(files).forEach(file => {
      const id = `att-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`
      if (file.type.startsWith('image/')) {
        const reader = new FileReader()
        reader.onload = () => {
          setAttachments(prev => [
            ...prev,
            { id, name: file.name, type: file.type, size: file.size, dataUrl: reader.result as string },
          ])
        }
        reader.readAsDataURL(file)
      } else {
        setAttachments(prev => [
          ...prev,
          { id, name: file.name, type: file.type, size: file.size },
        ])
      }
    })
  }

  const handlePaste = (e: React.ClipboardEvent) => {
    if (e.clipboardData.files && e.clipboardData.files.length > 0) {
      e.preventDefault()
      handleFiles(e.clipboardData.files)
    }
  }

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault()
    setIsDragging(false)
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      handleFiles(e.dataTransfer.files)
    }
  }

  const handleSubmit = () => {
    const text = input.trim()
    if (!text && attachments.length === 0) return

    const attSummary = attachments.length > 0
      ? `\n\n[Attachments: ${attachments.map(a => a.name).join(', ')}]`
      : ''
    const fullPrompt = `${text}${attSummary}`

    if (isRunning) {
      setQueuedMessages(prev => [
        ...prev,
        { id: `queue-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`, text: fullPrompt, timestamp: new Date() },
      ])
    } else {
      onSend(fullPrompt)
    }
    setInput('')
    setAttachments([])
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
    <div
      className={`composer-container ${isDragging ? 'dragging' : ''}`}
      onDragOver={e => { e.preventDefault(); setIsDragging(true) }}
      onDragLeave={() => setIsDragging(false)}
      onDrop={handleDrop}
    >
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
      <ComposerAttachments
        attachments={attachments}
        onRemove={(id) => setAttachments(prev => prev.filter(a => a.id !== id))}
      />
      <div className="composer">
        <input
          ref={fileInputRef}
          type="file"
          multiple
          style={{ display: 'none' }}
          onChange={e => e.target.files && handleFiles(e.target.files)}
        />
        <button
          type="button"
          className="composer-attach-btn"
          onClick={() => fileInputRef.current?.click()}
          title="Attach files or screenshots (or drag & drop / paste)"
        >
          <Paperclip size={16} />
        </button>
        <button
          type="button"
          className={`composer-mic-btn ${isListening ? 'listening' : ''}`}
          onClick={toggleListening}
          title={isListening ? 'Stop voice dictation' : 'Start voice dictation (mic)'}
        >
          {isListening ? <MicOff size={16} /> : <Mic size={16} />}
        </button>
        <div className="composer-input-wrapper">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleInput}
            onFocus={handleFocus}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            placeholder={isListening ? "Listening... speak your prompt..." : isRunning ? "Steer running agent with new instruction..." : "What would you like Fathom to accomplish? (Enter to send, Shift+Enter for newlines)"}
            rows={expanded ? 4 : 1}
          />
        </div>
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