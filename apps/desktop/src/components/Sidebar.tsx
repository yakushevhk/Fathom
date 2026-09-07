import { useState, useEffect } from 'react'
import type { SessionSummary } from '../lib/api'
import { SidebarContextMenu } from './SidebarContextMenu'

interface SidebarProps {
  sessions: SessionSummary[]
  activeSession: SessionSummary | null
  loading: boolean
  onSelect: (session: SessionSummary) => void
  onCancel: (id: string) => void
  engineRunning: boolean
  onStartEngine: () => void
  onStopEngine: () => void
  engineLoading: boolean
  engineError: string | null
}

export function Sidebar({
  sessions,
  activeSession,
  loading,
  onSelect,
  onCancel,
  engineRunning,
  onStartEngine,
  onStopEngine,
  engineLoading,
  engineError,
}: SidebarProps) {
  const [pinnedIds, setPinnedIds] = useState<string[]>(() => {
    const saved = localStorage.getItem('fathom_pinned_sessions')
    return saved ? JSON.parse(saved) : []
  })
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; session: SessionSummary } | null>(null)

  useEffect(() => {
    localStorage.setItem('fathom_pinned_sessions', JSON.stringify(pinnedIds))
  }, [pinnedIds])

  const togglePin = (id: string) => {
    setPinnedIds(prev => (prev.includes(id) ? prev.filter(x => x !== id) : [id, ...prev]))
  }

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-title">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" fill="none" />
            <path d="M5 8h6M8 5v6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          </svg>
          Fathom
        </div>
        <input
          className="sidebar-search"
          type="search"
          value={search}
          onChange={event => setSearch(event.target.value)}
          placeholder="Search worker sessions..."
          aria-label="Search worker sessions"
        />
      </div>

      <div className="sidebar-actions">
        {engineRunning ? (
          <button onClick={onStopEngine} disabled={engineLoading}>
            {engineLoading ? 'Stopping...' : 'Stop Engine'}
          </button>
        ) : (
          <button onClick={onStartEngine} disabled={engineLoading} style={{ borderColor: 'var(--accent)', color: 'var(--accent)' }}>
            {engineLoading ? 'Starting...' : 'Start Engine'}
          </button>
        )}
      </div>

      {engineError && (
        <div style={{ padding: '8px 12px', fontSize: 11, color: 'var(--danger)' }}>
          {engineError}
        </div>
      )}

      <div className="session-list">
        {loading && sessions.length === 0 && (
          <div className="flex-center" style={{ padding: 24, color: 'var(--fg-tertiary)' }}>
            <div className="spinner" />
          </div>
        )}

        {!loading && sessions.length === 0 && engineRunning && (
          <div className="flex-center" style={{ padding: 24, color: 'var(--fg-tertiary)', fontSize: 12 }}>
            No worker sessions yet
          </div>
        )}

        {!engineRunning && (
          <div className="flex-center" style={{ padding: 24, color: 'var(--fg-tertiary)', fontSize: 12, textAlign: 'center', lineHeight: 1.6 }}>
            Engine is offline.<br />Start it to manage sessions.
          </div>
        )}

        {/* Pinned / Sorted sessions */}
        {(() => {
          const pinned = visibleSessions.filter(s => pinnedIds.includes(s.id))
          const unpinned = visibleSessions.filter(s => !pinnedIds.includes(s.id))
          const sorted = [...pinned, ...unpinned]

          return sorted.map(s => {
            const isPinned = pinnedIds.includes(s.id)
            return (
              <div
                key={s.id}
                className={`session-item ${activeSession?.id === s.id ? 'active' : ''} ${isPinned ? 'pinned' : ''}`}
                onContextMenu={e => {
                  e.preventDefault()
                  setContextMenu({ x: e.clientX, y: e.clientY, session: s })
                }}
              >
                <button
                  type="button"
                  className="session-row-select"
                  onClick={() => onSelect(s)}
                  aria-current={activeSession?.id === s.id ? 'true' : undefined}
                >
                  <span className={`session-status ${s.status}`} />
                  <span className="session-info">
                    <span className="session-title">
                      {isPinned && <span className="pin-indicator" title="Pinned session">📌 </span>}
                      {s.query || 'Untitled'}
                    </span>
                    <span className="session-meta">{s.id.slice(0, 8)}</span>
                  </span>
                </button>
                <button
                  className="titlebar-btn"
                  onClick={() => onCancel(s.id)}
                  title="Cancel worker session"
                  aria-label={`Cancel ${s.query || 'worker session'}`}
                  style={{ opacity: 0.5, fontSize: 12 }}
                >
                  &times;
                </button>
              </div>
            )
          })
        })()}
        {visibleSessions.length === 0 && sessions.length > 0 && (
          <div className="flex-center" style={{ padding: 24, color: 'var(--fg-tertiary)', fontSize: 12 }}>
            No matching worker sessions
          </div>
        )}
      </div>
      {contextMenu && (
        <SidebarContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          session={contextMenu.session}
          isPinned={pinnedIds.includes(contextMenu.session.id)}
          onPin={togglePin}
          onRename={(id) => {
            const newName = prompt('Enter new session title:', contextMenu.session.query)
            if (newName) {
              contextMenu.session.query = newName
            }
          }}
          onCopyId={(id) => {
            navigator.clipboard.writeText(id)
          }}
          onExport={(id) => {
            const text = `# Fathom Session ${id}\n\nTask: ${contextMenu.session.query}\nStatus: ${contextMenu.session.status}\n`
            const blob = new Blob([text], { type: 'text/markdown' })
            const url = URL.createObjectURL(blob)
            const a = document.createElement('a')
            a.href = url
            a.download = `fathom-session-${id.slice(0, 8)}.md`
            a.click()
            URL.revokeObjectURL(url)
          }}
          onDelete={onCancel}
          onClose={() => setContextMenu(null)}
        />
      )}
    </aside>
  )
}