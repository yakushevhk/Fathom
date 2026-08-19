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
        <input className="sidebar-search" type="text" placeholder="Search sessions..." />
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
            No sessions yet
          </div>
        )}

        {!engineRunning && (
          <div className="flex-center" style={{ padding: 24, color: 'var(--fg-tertiary)', fontSize: 12, textAlign: 'center', lineHeight: 1.6 }}>
            Engine is offline.<br />Start it to manage sessions.
          </div>
        )}

        {sessions.map(s => (
          <div
            key={s.id}
            className={`session-item ${activeSession?.id === s.id ? 'active' : ''}`}
            onClick={() => onSelect(s)}
          >
            <span className={`session-status ${s.status}`} />
            <div className="session-info">
              <div className="session-title">{s.query || 'Untitled'}</div>
              <div className="session-meta">{s.id.slice(0, 8)}</div>
            </div>
            <button
              className="titlebar-btn"
              onClick={e => { e.stopPropagation(); onCancel(s.id) }}
              title="Cancel"
              style={{ opacity: 0.5, fontSize: 12 }}
            >
              &times;
            </button>
          </div>
        ))}
      </div>
    </aside>
  )
}