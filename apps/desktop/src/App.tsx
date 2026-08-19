import { useState } from 'react'
import { Sidebar } from './components/Sidebar'
import { Conversation } from './components/Conversation'
import { Composer } from './components/Composer'
import { StatusStrip } from './components/StatusStrip'
import { useEngine } from './hooks/useEngine'
import { useSessions } from './hooks/useSessions'
import { api, type SessionSummary } from './lib/api'

export default function App() {
  const { status, loading: engineLoading, error: engineError, start, stop } = useEngine()
  const { sessions, loading: sessionsLoading, create, cancel } = useSessions(status.running)
  const [activeSession, setActiveSession] = useState<SessionSummary | null>(null)
  const [showSettings, setShowSettings] = useState(false)
  const [showRightPane, setShowRightPane] = useState(false)

  const handleStartEngine = async () => {
    await start()
  }

  const handleStopEngine = async () => {
    await stop()
    setActiveSession(null)
  }

  const handleNewSession = async (query: string) => {
    const id = await create(query)
    if (id) {
      // Poll for the new session to appear
      setTimeout(() => {
        const session = sessions.find(s => s.id === id)
        if (session) setActiveSession(session)
      }, 500)
    }
  }

  return (
    <div className="app-shell">
      <div className="titlebar">
        <div className="titlebar-actions">
          <button className="titlebar-btn" onClick={() => setShowSettings(!showSettings)} title="Settings">
            &#9881;
          </button>
          <button className="titlebar-btn" onClick={() => setShowRightPane(!showRightPane)} title="Toggle right pane">
            &#9776;
          </button>
        </div>
        <div className="titlebar-drag" />
        <span className="engine-badge" data-phase={status.phase}>
          <span className={`status-dot ${status.phase === 'running' ? 'running' : status.phase === 'error' ? 'error' : 'stopped'}`} />
          {status.phase === 'running' ? 'Engine Online' : status.phase === 'starting' ? 'Starting...' : 'Engine Offline'}
        </span>
      </div>

      <div className="app-body">
        {/* Sidebar */}
        <Sidebar
          sessions={sessions}
          activeSession={activeSession}
          loading={sessionsLoading}
          onSelect={setActiveSession}
          onCancel={cancel}
          engineRunning={status.running}
          onStartEngine={handleStartEngine}
          onStopEngine={handleStopEngine}
          engineLoading={engineLoading}
          engineError={engineError}
        />

        {/* Main conversation area */}
        <div className="main-content">
          {showSettings ? (
            <SettingsPanel
              engineUrl={status.url}
              enginePort={status.port}
              onClose={() => setShowSettings(false)}
            />
          ) : (
            <>
              <Conversation
                activeSession={activeSession}
                engineUrl={status.url}
              />
              <Composer
                onSend={handleNewSession}
                activeSession={activeSession}
                onSteer={(instruction) => {
                  if (activeSession) {
                    api.sessions.steer(activeSession.id, instruction)
                  }
                }}
                onCancel={() => {
                  if (activeSession) cancel(activeSession.id)
                }}
              />
            </>
          )}
        </div>

        {/* Right pane (optional: diff, changes, terminal) */}
        {showRightPane && (
          <div className="right-pane">
            <div className="right-pane-header">
              Details
              <button className="titlebar-btn" onClick={() => setShowRightPane(false)}>&times;</button>
            </div>
            <div className="right-pane-body">
              {activeSession ? (
                <SessionDetails session={activeSession} />
              ) : (
                <p className="text-muted">Select a session to view details</p>
              )}
            </div>
          </div>
        )}
      </div>

      <StatusStrip
        status={status}
        activeSession={activeSession}
      />
    </div>
  )
}

// ── Sub-components ──────────────────────────────────────────────────────

function SettingsPanel({
  engineUrl,
  enginePort,
  onClose,
}: {
  engineUrl: string | null
  enginePort: number | null
  onClose: () => void
}) {
  return (
    <div className="conversation-view" style={{ padding: '24px' }}>
      <div className="settings-panel">
        <div className="settings-section">
          <div className="settings-section-title">Engine</div>
          <div className="settings-row">
            <label>Engine URL</label>
            <input className="settings-input" value={engineUrl ?? 'Not running'} readOnly />
          </div>
          <div className="settings-row">
            <label>Port</label>
            <input className="settings-input" value={enginePort ?? '-'} readOnly />
          </div>
        </div>

        <div className="settings-section">
          <div className="settings-section-title">Settings</div>
          <div className="settings-row">
            <label>Auto-start engine</label>
            <input type="checkbox" defaultChecked />
          </div>
          <div className="settings-row">
            <label>Notifications</label>
            <input type="checkbox" defaultChecked />
          </div>
          <div className="settings-row">
            <label>Auto-approve tool calls</label>
            <input type="checkbox" />
          </div>
        </div>

        <div style={{ marginTop: 16 }}>
          <button className="composer-btn" onClick={onClose}>Close Settings</button>
        </div>
      </div>
    </div>
  )
}

function SessionDetails({ session }: { session: SessionSummary }) {
  return (
    <div className="settings-panel">
      <div className="settings-section">
        <div className="settings-section-title">Session</div>
        <div className="settings-row">
          <label>ID</label>
          <span className="text-secondary" style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>
            {session.id.slice(0, 8)}...
          </span>
        </div>
        <div className="settings-row">
          <label>Status</label>
          <span className={`session-status ${session.status}`} />
        </div>
        <div className="settings-row">
          <label>Query</label>
          <span className="text-secondary truncate" style={{ maxWidth: 200 }}>
            {session.query}
          </span>
        </div>
        <div className="settings-row">
          <label>Output</label>
          <span className="text-secondary truncate" style={{ maxWidth: 200 }}>
            {session.output_dir}
          </span>
        </div>
      </div>
    </div>
  )
}