import { useEffect, useRef, useState } from 'react'
import type { SessionSummary } from '../lib/api'

export interface PaletteItem {
  id: string
  title: string
  subtitle?: string
  category: 'Sessions' | 'Navigation' | 'Actions' | 'Controls'
  badge?: string
  action: () => void
}

interface CommandPaletteProps {
  open: boolean
  onClose: () => void
  sessions: SessionSummary[]
  activeSession: SessionSummary | null
  onSelectSession: (session: SessionSummary) => void
  onNewSession: () => void
  onToggleSettings: () => void
  onToggleRightPane: () => void
  onOpenChannel: (channel: 'computer' | 'governance' | 'overview' | 'workers') => void
}

export function CommandPalette({
  open,
  onClose,
  sessions,
  activeSession,
  onSelectSession,
  onNewSession,
  onToggleSettings,
  onToggleRightPane,
  onOpenChannel,
}: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (open) {
      setQuery('')
      setSelectedIndex(0)
      setTimeout(() => inputRef.current?.focus(), 50)
    }
  }, [open])

  // Build items list
  const items: PaletteItem[] = []

  // Quick actions
  items.push({
    id: 'new-session',
    title: 'New Worker Session',
    subtitle: 'Start a new autonomous AI worker task',
    category: 'Actions',
    badge: '⌘N',
    action: () => {
      onNewSession()
      onClose()
    },
  })

  items.push({
    id: 'toggle-settings',
    title: 'Open Settings',
    subtitle: 'Configure LLM models, engines, and API keys',
    category: 'Actions',
    badge: '⌘,',
    action: () => {
      onToggleSettings()
      onClose()
    },
  })

  items.push({
    id: 'toggle-computer',
    title: 'Open Computer & Browser Control',
    subtitle: 'Inspect live Playwright / browser automation',
    category: 'Navigation',
    action: () => {
      onOpenChannel('computer')
      onClose()
    },
  })

  items.push({
    id: 'toggle-governance',
    title: 'Open Guardrails & Policy Console',
    subtitle: 'Review approvals, audit logs, and security policies',
    category: 'Navigation',
    action: () => {
      onOpenChannel('governance')
      onClose()
    },
  })

  items.push({
    id: 'toggle-right-pane',
    title: 'Toggle Side Inspector Pane',
    subtitle: 'Expand or collapse the control room pane',
    category: 'Controls',
    badge: '⌘B',
    action: () => {
      onToggleRightPane()
      onClose()
    },
  })

  // Add sessions
  sessions.forEach(s => {
    const isCurrent = activeSession?.id === s.id
    items.push({
      id: `session-${s.id}`,
      title: s.query || `Session ${s.id.slice(0, 8)}`,
      subtitle: `${s.status.toUpperCase()} · ${s.total_agents} workers · ${s.total_tokens} tokens`,
      category: 'Sessions',
      badge: isCurrent ? 'Active' : undefined,
      action: () => {
        onSelectSession(s)
        onClose()
      },
    })
  })

  // Filter items
  const q = query.trim().toLowerCase()
  const filtered = q
    ? items.filter(
        item =>
          item.title.toLowerCase().includes(q) ||
          item.subtitle?.toLowerCase().includes(q) ||
          item.category.toLowerCase().includes(q)
      )
    : items

  // Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex(i => (filtered.length ? (i + 1) % filtered.length : 0))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex(i => (filtered.length ? (i - 1 + filtered.length) % filtered.length : 0))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (filtered[selectedIndex]) {
        filtered[selectedIndex].action()
      }
    }
  }

  // Auto-scroll selected into view
  useEffect(() => {
    const el = listRef.current?.children[selectedIndex] as HTMLElement
    el?.scrollIntoView({ block: 'nearest' })
  }, [selectedIndex])

  if (!open) return null

  return (
    <div className="palette-backdrop" onClick={onClose}>
      <div className="palette-dialog" onClick={e => e.stopPropagation()}>
        <div className="palette-input-row">
          <span className="palette-search-icon">🔍</span>
          <input
            ref={inputRef}
            className="palette-input"
            placeholder="Search sessions, tools, or commands... (Type or navigate with ↑↓)"
            value={query}
            onChange={e => {
              setQuery(e.target.value)
              setSelectedIndex(0)
            }}
            onKeyDown={handleKeyDown}
          />
          <kbd className="palette-kbd">ESC</kbd>
        </div>

        <div className="palette-list" ref={listRef}>
          {filtered.length === 0 ? (
            <div className="palette-empty">No results found for &ldquo;{query}&rdquo;</div>
          ) : (
            filtered.map((item, idx) => (
              <div
                key={item.id}
                className={`palette-item ${idx === selectedIndex ? 'selected' : ''}`}
                onClick={() => item.action()}
                onMouseEnter={() => setSelectedIndex(idx)}
              >
                <div className="palette-item-main">
                  <div className="palette-item-title">{item.title}</div>
                  {item.subtitle && <div className="palette-item-subtitle">{item.subtitle}</div>}
                </div>
                <div className="palette-item-meta">
                  <span className="palette-category-chip">{item.category}</span>
                  {item.badge && <kbd className="palette-item-badge">{item.badge}</kbd>}
                </div>
              </div>
            ))
          )}
        </div>

        <div className="palette-footer">
          <span><kbd className="palette-key">↑↓</kbd> navigate</span>
          <span><kbd className="palette-key">↵</kbd> select</span>
          <span><kbd className="palette-key">esc</kbd> dismiss</span>
        </div>
      </div>
    </div>
  )
}
