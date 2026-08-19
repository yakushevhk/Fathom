import type { DaemonStatus, SessionSummary } from '../lib/api'

interface StatusStripProps {
  status: DaemonStatus
  activeSession: SessionSummary | null
}

export function StatusStrip({ status, activeSession }: StatusStripProps) {
  return (
    <div className="status-strip">
      <span className={`status-dot ${status.phase === 'running' ? 'running' : status.phase === 'error' ? 'error' : 'stopped'}`} />
      <span>
        {status.phase === 'running'
          ? `Engine on :${status.port}`
          : status.phase === 'starting'
            ? 'Starting engine...'
            : 'Engine offline'}
      </span>
      {status.version && (
        <span className="text-muted" style={{ fontSize: 10 }}>
          v{status.version}
        </span>
      )}
      <span className="status-engine">
        {activeSession ? `Session ${activeSession.id.slice(0, 8)}` : 'No active session'}
      </span>
    </div>
  )
}