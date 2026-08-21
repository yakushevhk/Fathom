import { useState } from 'react'
import type { SessionSummary } from '../lib/api'

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
  const [search, setSearch] = useState('')
  const normalizedSearch = search.trim().toLowerCase()
  const visibleSessions = normalizedSearch
    ? sessions.filter(session => `${session.query} ${session.id} ${session.status}`.toLowerCase().includes(normalizedSearch))
    : sessions

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

        {visibleSessions.map(s => (
          <div
            key={s.id}
            className={`session-item ${activeSession?.id === s.id ? 'active' : ''}`}
          >
            <button
              type="button"
              className="session-row-select"
              onClick={() => onSelect(s)}
              aria-current={activeSession?.id === s.id ? 'true' : undefined}
            >
              <span className={`session-status ${s.status}`} />
              <span className="session-info">
                <span className="session-title">{s.query || 'Untitled'}</span>
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
        ))}
        {visibleSessions.length === 0 && sessions.length > 0 && (
          <div className="flex-center" style={{ padding: 24, color: 'var(--fg-tertiary)', fontSize: 12 }}>
            No matching worker sessions
          </div>
        )}
      </div>
    </aside>
  )
}