import { useEffect, useRef } from 'react'

interface ChatFindBarProps {
  query: string
  onQueryChange: (q: string) => void
  onClose: () => void
  totalMatches: number
  currentIndex: number
  onNext: () => void
  onPrev: () => void
}

export function ChatFindBar({
  query,
  onQueryChange,
  onClose,
  totalMatches,
  currentIndex,
  onNext,
  onPrev,
}: ChatFindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    inputRef.current?.focus()
    inputRef.current?.select()
  }, [])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (e.shiftKey) {
        onPrev()
      } else {
        onNext()
      }
    }
  }

  return (
    <div className="chat-find-bar">
      <span className="chat-find-icon">🔍</span>
      <input
        ref={inputRef}
        className="chat-find-input"
        placeholder="Find in conversation..."
        value={query}
        onChange={e => onQueryChange(e.target.value)}
        onKeyDown={handleKeyDown}
      />
      <span className="chat-find-count">
        {query ? (totalMatches > 0 ? `${currentIndex + 1} of ${totalMatches}` : '0 results') : ''}
      </span>
      <div className="chat-find-buttons">
        <button
          className="chat-find-btn"
          onClick={onPrev}
          disabled={totalMatches === 0}
          title="Previous match (Shift+Enter)"
        >
          ▲
        </button>
        <button
          className="chat-find-btn"
          onClick={onNext}
          disabled={totalMatches === 0}
          title="Next match (Enter)"
        >
          ▼
        </button>
        <button className="chat-find-btn close" onClick={onClose} title="Close (Escape)">
          ✕
        </button>
      </div>
    </div>
  )
}
