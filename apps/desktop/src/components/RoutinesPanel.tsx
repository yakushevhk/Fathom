import { useState, useEffect } from 'react'
import { Calendar, Clock, Plus, Trash2, Play, CheckCircle2 } from 'lucide-react'

export interface Routine {
  id: string
  title: string
  cron: string
  query: string
  enabled: boolean
  lastRun?: string
  nextRun?: string
}

const DEFAULT_ROUTINES: Routine[] = [
  {
    id: 'routine-1',
    title: 'Daily Tech & GitHub Research',
    cron: '0 9 * * 1-5',
    query: 'Audit trending repositories in Rust, AI agents, and systems programming. Summarize breakthroughs into daily briefing.',
    enabled: true,
    lastRun: 'Today, 09:00 AM',
    nextRun: 'Tomorrow, 09:00 AM',
  },
  {
    id: 'routine-2',
    title: 'Hourly Sentry Error Triage',
    cron: '0 * * * *',
    query: 'Inspect new production exceptions, map root cause files with AST intelligence, and draft fixes.',
    enabled: true,
    lastRun: '15 mins ago',
    nextRun: 'in 45 mins',
  },
  {
    id: 'routine-3',
    title: 'Weekly Dependency & Security Audit',
    cron: '0 12 * * 0',
    query: 'Run security advisories audit, check for outdated crates, and verify license compliance.',
    enabled: false,
    nextRun: 'Sunday, 12:00 PM',
  },
]

interface RoutinesPanelProps {
  onTriggerRoutine?: (query: string) => void
}

export function RoutinesPanel({ onTriggerRoutine }: RoutinesPanelProps) {
  const [routines, setRoutines] = useState<Routine[]>(() => {
    const saved = localStorage.getItem('fathom_routines')
    return saved ? JSON.parse(saved) : DEFAULT_ROUTINES
  })
  const [showAddForm, setShowAddForm] = useState(false)
  const [newTitle, setNewTitle] = useState('')
  const [newCron, setNewCron] = useState('0 9 * * *')
  const [newQuery, setNewQuery] = useState('')

  useEffect(() => {
    localStorage.setItem('fathom_routines', JSON.stringify(routines))
  }, [routines])

  const handleToggle = (id: string) => {
    setRoutines(prev =>
      prev.map(r => (r.id === id ? { ...r, enabled: !r.enabled } : r))
    )
  }

  const handleDelete = (id: string) => {
    setRoutines(prev => prev.filter(r => r.id !== id))
  }

  const handleAdd = (e: React.FormEvent) => {
    e.preventDefault()
    if (!newTitle.trim() || !newQuery.trim()) return
    const routine: Routine = {
      id: `routine-${Date.now()}`,
      title: newTitle.trim(),
      cron: newCron.trim(),
      query: newQuery.trim(),
      enabled: true,
      nextRun: 'Upcoming on schedule',
    }
    setRoutines(prev => [routine, ...prev])
    setNewTitle('')
    setNewQuery('')
    setShowAddForm(false)
  }

  return (
    <div className="routines-panel">
      <div className="routines-header">
        <div>
          <div className="routines-title">Recurring Routines</div>
          <div className="routines-subtitle">Autonomous worker cron schedules</div>
        </div>
        <button
          className="routines-add-btn"
          onClick={() => setShowAddForm(!showAddForm)}
          title="Add new scheduled routine"
        >
          <Plus size={13} style={{ marginRight: 4 }} />
          New Routine
        </button>
      </div>

      {showAddForm && (
        <form className="routine-add-form" onSubmit={handleAdd}>
          <div className="routine-field">
            <label>Routine Title</label>
            <input
              placeholder="e.g. Daily Market Briefing"
              value={newTitle}
              onChange={e => setNewTitle(e.target.value)}
              required
            />
          </div>
          <div className="routine-field">
            <label>Cron Expression (Schedule)</label>
            <input
              placeholder="0 9 * * * (At 09:00 every day)"
              value={newCron}
              onChange={e => setNewCron(e.target.value)}
              required
            />
          </div>
          <div className="routine-field">
            <label>Worker Prompt / Task</label>
            <textarea
              placeholder="What instructions should the worker execute on schedule?"
              value={newQuery}
              onChange={e => setNewQuery(e.target.value)}
              rows={3}
              required
            />
          </div>
          <div className="routine-form-actions">
            <button type="button" className="routine-cancel-btn" onClick={() => setShowAddForm(false)}>
              Cancel
            </button>
            <button type="submit" className="routine-save-btn">
              Save Routine
            </button>
          </div>
        </form>
      )}

      <div className="routines-list">
        {routines.map(item => (
          <div key={item.id} className={`routine-card ${item.enabled ? 'active' : 'disabled'}`}>
            <div className="routine-card-top">
              <div className="routine-card-title-row">
                <Calendar size={13} className="routine-calendar-icon" />
                <span className="routine-card-title">{item.title}</span>
              </div>
              <div className="routine-card-controls">
                {onTriggerRoutine && (
                  <button
                    className="routine-icon-btn run"
                    onClick={() => onTriggerRoutine(item.query)}
                    title="Trigger routine right now"
                  >
                    <Play size={11} />
                  </button>
                )}
                <button
                  className={`routine-toggle-switch ${item.enabled ? 'on' : ''}`}
                  onClick={() => handleToggle(item.id)}
                  title={item.enabled ? 'Pause routine' : 'Enable routine'}
                >
                  <span className="switch-thumb" />
                </button>
                <button
                  className="routine-icon-btn delete"
                  onClick={() => handleDelete(item.id)}
                  title="Delete routine"
                >
                  <Trash2 size={11} />
                </button>
              </div>
            </div>

            <div className="routine-cron-badge">
              <Clock size={11} style={{ marginRight: 4 }} />
              <code>{item.cron}</code>
            </div>

            <div className="routine-query-preview">
              &ldquo;{item.query}&rdquo;
            </div>

            <div className="routine-card-footer">
              {item.lastRun && <span>Last: {item.lastRun}</span>}
              {item.nextRun && (
                <span className="routine-next">
                  <CheckCircle2 size={10} style={{ marginRight: 3 }} />
                  Next: {item.nextRun}
                </span>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
