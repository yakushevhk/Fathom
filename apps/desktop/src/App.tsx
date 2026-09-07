import { useEffect, useState } from 'react'
import { Sidebar } from './components/Sidebar'
import { Conversation } from './components/Conversation'
import { Composer } from './components/Composer'
import { StatusStrip } from './components/StatusStrip'
import { LiveScreen } from './components/LiveScreen'
import { GovernancePanel } from './components/GovernancePanel'
import { CoworkerRail } from './components/CoworkerRail'
import { CommandPalette } from './components/CommandPalette'
import { ApprovalModeSelector, type ApprovalMode } from './components/ApprovalModeSelector'
import { ModelPicker, type EffortLevel, AVAILABLE_MODELS } from './components/ModelPicker'
import { RoutinesPanel } from './components/RoutinesPanel'
import { ArtifactsDrawer } from './components/ArtifactsDrawer'
import { McpServersPanel } from './components/McpServersPanel'
import { CallView } from './components/CallView'
import { Phone } from 'lucide-react'
import { useSessions } from './hooks/useSessions'
import { api, type SessionSummary } from './lib/api'
export default function App() {
  const { status, loading: engineLoading, error: engineError, start, stop } = useEngine()
  const { sessions, loading: sessionsLoading, create, cancel } = useSessions(status.running)
  const [activeSession, setActiveSession] = useState<SessionSummary | null>(null)
  const [showSettings, setShowSettings] = useState(false)
  const [showPalette, setShowPalette] = useState(false)
  const [showRightPane, setShowRightPane] = useState(false)
  const [showCallView, setShowCallView] = useState(false)
  const [rightTab, setRightTab] = useState<'details' | 'computer' | 'governance' | 'routines' | 'artifacts' | 'mcp'>('computer')
  const [approvalMode, setApprovalMode] = useState<ApprovalMode>('auto')
  const [effortLevel, setEffortLevel] = useState<EffortLevel>('none')
  // Global keyboard shortcuts (⌘K for palette, ⌘B for sidebar/right pane, ⌘N for new task)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setShowPalette(p => !p)
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'b') {
        e.preventDefault()
        setShowRightPane(r => !r)
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === ',') {
        e.preventDefault()
        setShowSettings(s => !s)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])
  useEffect(() => {
    setActiveSession(current => current ? sessions.find(session => session.id === current.id) ?? null : current)
  }, [sessions])

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
      try {
        setActiveSession(await api.sessions.get(id))
      } catch {
        const session = sessions.find(s => s.id === id)
        if (session) setActiveSession(session)
      }
    }
  }

    <div className="app-shell">
      <div className="titlebar">
        <div className="titlebar-actions">
          <button
            className={`titlebar-btn ${showCallView ? 'active-call' : ''}`}
            onClick={() => setShowCallView(!showCallView)}
            title="Start Fullscreen Voice Call Mode"
          >
            <Phone size={14} />
          </button>
          <button className="titlebar-btn" onClick={() => setShowPalette(true)} title="Quick Search & Switcher (⌘K)">
            <span style={{ fontSize: 13, fontWeight: 'bold' }}>⌘K</span>
          </button>
          <button className="titlebar-btn" onClick={() => setShowSettings(!showSettings)} title="Settings (⌘,)">
            &#9881;
          </button>
          <button className="titlebar-btn" onClick={() => setShowRightPane(!showRightPane)} title="Toggle right pane (⌘B)">
            &#9776;
          </button>
        </div>
        <div className="titlebar-drag" />
        <span className="engine-badge" data-phase={status.phase}>
          <span className={`status-dot ${status.phase === 'running' ? 'running' : status.phase === 'error' ? 'error' : 'stopped'}`} />
          {status.phase === 'running' ? 'Engine Online' : status.phase === 'starting' ? 'Starting...' : 'Engine Offline'}
        </span>
        <div className="titlebar-mode-picker">
          <ModelPicker
            selectedModel={selectedModel}
            onSelectModel={setSelectedModel}
            effort={effortLevel}
            onSelectEffort={setEffortLevel}
            compact
          />
          <ApprovalModeSelector currentMode={approvalMode} onChange={setApprovalMode} compact />
        </div>
      </div>
      <div className="app-body">
        <CoworkerRail
          sessions={sessions}
          activeSession={activeSession}
          onSelect={session => { setActiveSession(session); setShowRightPane(true); setRightTab('details') }}
          onChannel={channel => {
            if (channel === 'computer') { setShowRightPane(true); setRightTab('computer') }
            if (channel === 'governance') { setShowRightPane(true); setRightTab('governance') }
            if (channel === 'overview') { setShowRightPane(true); setRightTab('details') }
            if (channel === 'workers') { setShowRightPane(false) }
          }}
        />
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
                    void api.sessions.steer(activeSession.id, instruction).catch(() => undefined)
                  }
                }}
                onCancel={() => {
                  if (activeSession) cancel(activeSession.id)
                }}
              />
            </>
          )}
        </div>

        {/* Governed control room */}
        {showRightPane && (
          <div className="right-pane">
            <div className="right-pane-header control-room-header">
              <div className="right-pane-tabs" role="tablist" aria-label="Control room">
                <button className={rightTab === 'computer' ? 'selected' : ''} onClick={() => setRightTab('computer')} role="tab" aria-selected={rightTab === 'computer'}>Computer</button>
                <button className={rightTab === 'governance' ? 'selected' : ''} onClick={() => setRightTab('governance')} role="tab" aria-selected={rightTab === 'governance'}>Guardrails</button>
                <button className={rightTab === 'routines' ? 'selected' : ''} onClick={() => setRightTab('routines')} role="tab" aria-selected={rightTab === 'routines'}>Routines</button>
                <button className={rightTab === 'mcp' ? 'selected' : ''} onClick={() => setRightTab('mcp')} role="tab" aria-selected={rightTab === 'mcp'}>MCP</button>
                <button className={rightTab === 'artifacts' ? 'selected' : ''} onClick={() => setRightTab('artifacts')} role="tab" aria-selected={rightTab === 'artifacts'}>Artifacts</button>
                <button className={rightTab === 'details' ? 'selected' : ''} onClick={() => setRightTab('details')} role="tab" aria-selected={rightTab === 'details'}>Session</button>
              </div>
              <button className="titlebar-btn" onClick={() => setShowRightPane(false)} aria-label="Close control room">&times;</button>
            </div>
            <div className="right-pane-body control-room-body">
              {rightTab === 'computer' && <LiveScreen />}
              {rightTab === 'governance' && <GovernancePanel />}
              {rightTab === 'routines' && <RoutinesPanel onTriggerRoutine={handleNewSession} />}
              {rightTab === 'mcp' && <McpServersPanel />}
              {rightTab === 'artifacts' && <ArtifactsDrawer session={activeSession} />}
              {rightTab === 'details' && (activeSession ? <SessionDetails session={activeSession} /> : <p className="text-muted">Select a session to view details</p>)}
            </div>
          </div>
        )}
      </div>

      <StatusStrip
        status={status}
        activeSession={activeSession}
      />
      {showCallView && (
        <CallView
          agentName={activeSession?.query?.slice(0, 30) || 'Fathom Assistant'}
          lastAssistantMessage={activeSession?.query}
          onEndCall={() => setShowCallView(false)}
          onSendUtterance={text => {
            if (activeSession) {
              void api.sessions.steer(activeSession.id, text).catch(() => undefined)
            } else {
              handleNewSession(text)
            }
          }}
        />
      )}

      <CommandPalette
        open={showPalette}
        onClose={() => setShowPalette(false)}
        sessions={sessions}
        activeSession={activeSession}
        onSelectSession={setActiveSession}
        onNewSession={() => {
          const input = prompt('Enter query for new worker session:')
          if (input) handleNewSession(input)
        }}
        onToggleSettings={() => setShowSettings(s => !s)}
        onToggleRightPane={() => setShowRightPane(r => !r)}
        onOpenChannel={channel => {
          if (channel === 'computer') { setShowRightPane(true); setRightTab('computer') }
          if (channel === 'governance') { setShowRightPane(true); setRightTab('governance') }
          if (channel === 'overview') { setShowRightPane(true); setRightTab('details') }
          if (channel === 'workers') { setShowRightPane(false) }
        }}
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