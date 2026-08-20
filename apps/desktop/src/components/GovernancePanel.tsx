import { useCallback, useEffect, useState } from 'react'
import { api, type AuditEvent, type GovernanceStatus, type PolicyRule } from '../lib/api'

interface GovernancePanelProps {
  baseUrl?: string
}

const fallbackStatus: GovernanceStatus = { enabled: false, mode: 'unknown', status: 'unavailable' }

export function GovernancePanel({ baseUrl = '/api/v1/governance' }: GovernancePanelProps) {
  const [status, setStatus] = useState<GovernanceStatus>(fallbackStatus)
  const [rules, setRules] = useState<PolicyRule[]>([])
  const [audit, setAudit] = useState<AuditEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [draft, setDraft] = useState<PolicyRule | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const [nextStatus, nextRules, nextAudit] = await Promise.all([
        api.governance.status(baseUrl),
        api.governance.rules(baseUrl),
        api.governance.audit(baseUrl),
      ])
      setStatus(nextStatus)
      setRules(nextRules.map((rule, index) => ({ ...rule, id: rule.id ?? `rule-${index}` })))
      setAudit(nextAudit)
      setError(null)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Governance service unavailable')
    } finally {
      setLoading(false)
    }
  }, [baseUrl])

  useEffect(() => {
    void refresh()
    const interval = window.setInterval(() => void refresh(), 8000)
    return () => window.clearInterval(interval)
  }, [refresh])

  const saveDraft = async () => {
    if (!draft) return
    setSaving(draft.id ?? null)
    try {
      const saved = await api.governance.updateRule(draft, baseUrl)
      setRules(current => current.map(rule => rule.id === saved.id ? saved : rule))
      setDraft(null)
      setError(null)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Could not save rule')
    } finally {
      setSaving(null)
    }
  }

  return (
    <section className="control-surface governance-panel" aria-label="Governance policy">
      <div className="surface-heading">
        <div>
          <div className="eyebrow">GOVERNANCE / POLICY PLANE</div>
          <h2>Guardrails</h2>
        </div>
        <button className="surface-button icon-button" onClick={() => void refresh()} disabled={loading} title="Refresh governance" aria-label="Refresh governance">↻</button>
      </div>

      <div className="governance-banner">
        <div>
          <span className="meta-label">POLICY MODE</span>
          <strong>{status.mode}</strong>
        </div>
        <span className={`policy-state ${status.status === 'active' ? 'active' : 'quiet'}`}>{status.status}</span>
      </div>

      {error && (
        <div className="surface-error" role="status">
          <span>{error}</span><button onClick={() => void refresh()}>Retry</button>
        </div>
      )}

      <div className="panel-section">
        <div className="section-label"><span>RULE EDITOR</span><span>{rules.length} rules</span></div>
        {loading && rules.length === 0 ? (
          <div className="surface-loading"><span className="spinner" /> Loading policy…</div>
        ) : rules.length === 0 ? (
          <div className="surface-empty">No policy rules received. Behavior follows the server governance mode.</div>
        ) : (
          <div className="rule-list">
            {rules.map(rule => (
              <div className="rule-row" key={rule.id ?? `${rule.tool ?? 'any'}-${rule.effect}`}>
                <button className="rule-main" onClick={() => setDraft(rule)} aria-label={`Edit ${rule.tool ?? 'policy'} rule`}>
                  <span className={`rule-effect ${rule.effect}`}>{rule.effect}</span>
                  <span className="rule-action">{rule.tool ?? '*'}</span>
                  <span className="rule-resource">{rule.path ?? rule.host ?? '*'}</span>
                </button>
                <span className="rule-id">{rule.id ?? 'inline'}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      {draft && (
        <div className="rule-editor">
          <div className="section-label"><span>EDIT RULE</span><button className="plain-button" onClick={() => setDraft(null)}>Close</button></div>
          <label className="editor-field">Effect<select value={draft.effect} onChange={event => setDraft({ ...draft, effect: event.target.value as PolicyRule['effect'] })}><option value="allow">allow</option><option value="deny">deny</option></select></label>
          <label className="editor-field">Tool<input value={draft.tool ?? ''} onChange={event => setDraft({ ...draft, tool: event.target.value })} /></label>
          <label className="editor-field">Path / host<input value={draft.path ?? draft.host ?? ''} onChange={event => setDraft({ ...draft, path: event.target.value })} /></label>
          <button className="surface-button primary" onClick={() => void saveDraft()} disabled={saving === draft.id}>{saving === draft.id ? 'Saving…' : 'Save rule'}</button>
        </div>
      )}

      <div className="panel-section audit-section">
        <div className="section-label"><span>AUDIT STREAM</span><span>redacted</span></div>
        {audit.length === 0 ? <div className="surface-empty">No recent decisions.</div> : (
          <div className="audit-list">
            {audit.slice(0, 12).map(event => (
              <div className="audit-row" key={event.id}>
                <span className={`audit-decision ${event.decision === 'allow' ? 'allow' : 'deny'}`}>{event.decision}</span>
                <div className="audit-detail"><strong>{event.tool}</strong><span>{event.file || event.url || 'resource redacted'}</span></div>
                <time>{formatTime(event.created_at ?? event.timestamp)}</time>
              </div>
            ))}
          </div>
        )}
      </div>
    </section>
  )
}

function formatTime(value?: string) {
  if (!value) return '—'
  const date = new Date(value)
  return Number.isNaN(date.valueOf()) ? '—' : date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}
