'use client'

import { useEffect, useMemo, useState } from 'react'
import { api, type ActionContext, type AuditEvent, type Decision, type PolicyRule } from '@/lib/api'

const secretKey = /token|secret|password|passwd|api[_-]?key|authorization|cookie|credential|private[_-]?key/i

function redact(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(redact)
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, secretKey.test(key) ? '[REDACTED]' : redact(item)]))
  }
  return value
}

function normaliseRules(value: { policy?: { rules?: PolicyRule[] } } | PolicyRule[]): PolicyRule[] {
  return Array.isArray(value) ? value : value.policy?.rules ?? []
}
function normaliseAudit(value: AuditEvent[]): AuditEvent[] {
  return value
}

function decisionAllowed(value: Decision): boolean {
  return typeof value === 'string' ? value === 'allow' : value.allowed
}

function decisionReason(value: Decision): string {
  if (typeof value === 'string') return value === 'allow' ? 'Matched allow rule' : 'No matching allow rule'
  return value.reason || (value.allowed ? 'Matched allow rule' : 'No matching allow rule')
}

function RuleRow({ rule, onChange, onRemove }: { rule: PolicyRule; onChange: (rule: PolicyRule) => void; onRemove: () => void }) {
  return (
    <div className="grid grid-cols-[1fr_92px_1fr_28px] gap-2 items-center border-b border-white/[0.06] py-2.5 last:border-0">
      <input aria-label="Rule tool" value={rule.tool ?? ''} onChange={e => onChange({ ...rule, tool: e.target.value })} placeholder="filesystem.read" className="ops-input font-mono" />
      <select aria-label="Rule effect" value={rule.effect} onChange={e => onChange({ ...rule, effect: e.target.value as PolicyRule['effect'] })} className="ops-input uppercase">
        <option value="allow">allow</option><option value="deny">deny</option>
      </select>
      <input aria-label="Rule path" value={rule.path ?? ''} onChange={e => onChange({ ...rule, path: e.target.value })} placeholder="path or *" className="ops-input font-mono" />
      <button onClick={onRemove} aria-label={`Remove ${rule.tool ?? 'policy'} rule`} className="text-gray-600 hover:text-red-300 transition-colors">×</button>
    </div>
  )
}

export default function GovernanceConsole() {
  const [rules, setRules] = useState<PolicyRule[]>([])
  const [events, setEvents] = useState<AuditEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [context, setContext] = useState<ActionContext>({ agent: 'web-user', session: 'governance-console', tool: 'filesystem.read', args: { path: '*' } })
  const [decision, setDecision] = useState<Decision | null>(null)
  const [deciding, setDeciding] = useState(false)

  const load = async () => {
    setLoading(true)
    setError(null)
    try {
      const [policy, audit] = await Promise.all([api.governance.policy(), api.governance.audit({ limit: 30 })])
      setRules(normaliseRules(policy))
      setEvents(normaliseAudit(audit))
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Governance service unavailable')
    } finally { setLoading(false) }
  }

  useEffect(() => { load() }, [])

  const enabledCount = rules.length
  const updateRule = (index: number, rule: PolicyRule) => setRules(current => current.map((item, i) => i === index ? rule : item))
  const addRule = () => setRules(current => [...current, { id: `local-${Date.now()}`, effect: 'deny', tool: '', path: '*' }])
  const save = async () => {
    setSaving(true); setError(null); setNotice(null)
    try { await api.governance.savePolicy(rules); setNotice('Policy published'); await load() }
    catch (e) { setError(e instanceof Error ? e.message : 'Could not save policy') }
    finally { setSaving(false) }
  }
  const decide = async (event: React.FormEvent) => {
    event.preventDefault(); setDeciding(true); setDecision(null); setError(null)
    try { setDecision(await api.governance.decide(context)) }
    catch (e) { setError(e instanceof Error ? e.message : 'Decision service unavailable') }
    finally { setDeciding(false) }
  }

  return (
    <div className="flex-1 overflow-y-auto bg-[#080909]">
      <header className="border-b border-white/[0.08] px-5 py-5 md:px-8">
        <div className="flex items-start justify-between gap-4">
          <div><p className="ops-kicker">Control plane / governance</p><h1 className="mt-1 text-xl tracking-tight text-gray-100">Policy &amp; audit</h1><p className="mt-1 max-w-xl text-xs text-gray-500">Governance evaluates configured agent actions when enabled. The server keeps governance disabled unless FATHOM_GOVERNANCE_ENABLED is set.</p></div>
          <button onClick={load} disabled={loading} className="ops-button-secondary">{loading ? 'Syncing…' : '↻ Refresh'}</button>
        </div>
      </header>
      <div className="grid gap-5 p-5 md:p-8 xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,.9fr)]">
        <div className="space-y-5">
          {error && <div role="alert" className="ops-alert"><span>CONNECTION ERROR</span><p>{error}</p></div>}
          {notice && <div className="ops-notice">{notice}</div>}
          <section className="ops-panel">
            <div className="ops-panel-head"><div><p className="ops-kicker">Active rules · {enabledCount}</p><h2>Execution policy</h2></div><button onClick={addRule} className="ops-button-secondary">+ Add rule</button></div>
            <div className="grid grid-cols-[1fr_92px_1fr_28px] gap-2 border-b border-white/[0.08] pb-2 text-[10px] uppercase tracking-widest text-gray-600"><span>Tool</span><span>Effect</span><span>Path</span><span /></div>
            {loading ? <div className="ops-empty">Loading policy…</div> : rules.length === 0 ? <div className="ops-empty">No rules published. Add a rule to begin; behavior follows the server governance mode.</div> : rules.map((rule, index) => <RuleRow key={rule.id ?? `rule-${index}`} rule={rule} onChange={next => updateRule(index, next)} onRemove={() => setRules(current => current.filter((_, i) => i !== index))} />)}
            <div className="mt-4 flex justify-end"><button onClick={save} disabled={saving || loading} className="ops-button-primary">{saving ? 'Publishing…' : 'Publish policy'}</button></div>
          </section>
          <section className="ops-panel">
            <div className="ops-panel-head"><div><p className="ops-kicker">Dry run</p><h2>Ask the policy engine</h2></div>{decision && <span className={`ops-status ${decisionAllowed(decision) ? 'ops-status-allow' : 'ops-status-deny'}`}>{decisionAllowed(decision) ? 'ALLOWED' : 'DENIED'}</span>}</div>
            <form onSubmit={decide} className="space-y-3">
              <div className="grid gap-3 sm:grid-cols-2"><label className="ops-label">Tool<input className="ops-input mt-1 font-mono" value={context.tool} onChange={e => setContext({ ...context, tool: e.target.value })} /></label><label className="ops-label">Path / resource<input className="ops-input mt-1 font-mono" value={typeof context.args.path === 'string' ? context.args.path : ''} onChange={e => setContext({ ...context, args: { ...context.args, path: e.target.value } })} /></label></div>
              <label className="ops-label">Agent / session<input className="ops-input mt-1" value={context.agent ?? context.actor ?? ''} onChange={e => setContext({ ...context, agent: e.target.value })} placeholder="operator or agent id" /></label>
              <button disabled={deciding || !context.tool?.trim()} className="ops-button-secondary" type="submit">{deciding ? 'Evaluating…' : 'Evaluate action'}</button>
              {decision && <div className="mt-3 border-l-2 border-white/20 bg-black/20 px-3 py-2 text-xs"><p className="text-gray-300">{decisionReason(decision)}</p>{typeof decision !== 'string' && decision.rule_id && <p className="mt-1 font-mono text-[10px] text-gray-600">rule {decision.rule_id}</p>}</div>}
            </form>
          </section>
        </div>
        <section className="ops-panel h-fit">
          <div className="ops-panel-head"><div><p className="ops-kicker">Immutable stream</p><h2>Audit timeline</h2></div><span className="font-mono text-[10px] text-gray-600">{events.length} events</span></div>
          {events.length === 0 ? <div className="ops-empty">No audit events yet. Decisions will appear here.</div> : <div className="relative ml-2 border-l border-white/[0.1]">{events.map((event, index) => { const allowed = event.decision === 'allow'; return <article key={event.id || index} className="relative pb-5 pl-5 last:pb-0"><span className={`absolute -left-[5px] top-1 h-2 w-2 rounded-full ${allowed ? 'bg-emerald-400' : 'bg-red-400'}`} /><div className="flex items-baseline justify-between gap-2"><span className="font-mono text-xs text-gray-300">{event.tool}</span><time className="text-[10px] text-gray-600">{new Date(event.timestamp).toLocaleString()}</time></div><p className={`mt-1 text-[10px] uppercase tracking-widest ${allowed ? 'text-emerald-400/80' : 'text-red-400/80'}`}>{allowed ? 'allowed' : 'denied'}{event.rule_id ? ` · ${event.rule_id}` : ''}</p><pre className="mt-2 max-h-24 overflow-auto whitespace-pre-wrap break-all font-mono text-[10px] text-gray-600">{JSON.stringify(redact(event.context ?? event.details ?? {}), null, 2)}</pre></article> })}</div>}
        </section>
      </div>
    </div>
  )
}
