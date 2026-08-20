import type { SessionSummary } from '../lib/api'

interface CoworkerRailProps {
  sessions: SessionSummary[]
  activeSession: SessionSummary | null
  onSelect: (session: SessionSummary) => void
  onChannel?: (channel: string) => void
}

const channels = [
  { id: 'overview', label: 'Overview', glyph: '◈' },
  { id: 'research', label: 'Research', glyph: '⌁' },
  { id: 'computer', label: 'Computer', glyph: '▣' },
  { id: 'governance', label: 'Guardrails', glyph: '⊙' },
]

export function CoworkerRail({ sessions, activeSession, onSelect, onChannel }: CoworkerRailProps) {
  const running = sessions.filter(session => session.status === 'running')

  return (
    <aside className="coworker-rail" aria-label="Coworker channels">
      <div className="rail-mark">F</div>
      <div className="rail-divider" />
      <nav className="channel-list" aria-label="Channels">
        {channels.map(channel => (
          <button className={`channel-button ${channel.id === 'research' ? 'active' : ''}`} key={channel.id} title={channel.label} aria-label={channel.label} onClick={() => onChannel?.(channel.id)}>
            <span>{channel.glyph}</span>
          </button>
        ))}
      </nav>
      <div className="rail-divider" />
      <div className="rail-label">ACTIVE</div>
      <div className="coworker-list">
        {running.length === 0 ? (
          <span className="rail-empty">—</span>
        ) : running.slice(0, 4).map(session => (
          <button
            className={`coworker-avatar ${activeSession?.id === session.id ? 'selected' : ''}`}
            key={session.id}
            onClick={() => onSelect(session)}
            title={session.query || 'Untitled coworker'}
            aria-label={`Open ${session.query || 'untitled coworker'}`}
          >
            {(session.query || 'C').trim().charAt(0).toUpperCase()}
            <span className="avatar-pulse" />
          </button>
        ))}
      </div>
      <div className="rail-footer" title="Governed workspace">◎</div>
    </aside>
  )
}
