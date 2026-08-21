'use client'

import { useCallback, useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { api, type Credential, type ObservabilitySummary, type ReplayAction } from '@/lib/api'

function formatNumber(value: number) {
  return new Intl.NumberFormat().format(value)
}

function formatDate(value: string | null) {
  if (!value) return 'In progress'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString()
}

function errorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback
}

export default function ObservabilityPage() {
  const [summary, setSummary] = useState<ObservabilitySummary | null>(null)
  const [replay, setReplay] = useState<ReplayAction[]>([])
  const [credentials, setCredentials] = useState<Credential[]>([])
  const [summaryError, setSummaryError] = useState<string | null>(null)
  const [replayError, setReplayError] = useState<string | null>(null)
  const [credentialsError, setCredentialsError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [credentialName, setCredentialName] = useState('')
  const [credentialKind, setCredentialKind] = useState('api_key')
  const [credentialSecret, setCredentialSecret] = useState('')
  const [credentialBusy, setCredentialBusy] = useState(false)
  const [credentialNotice, setCredentialNotice] = useState<string | null>(null)

  const load = useCallback(async (isRefresh = false) => {
    if (isRefresh) setRefreshing(true)
    else setLoading(true)
    setSummaryError(null)
    setReplayError(null)
    setCredentialsError(null)
    const [summaryResult, replayResult, credentialsResult] = await Promise.allSettled([
      api.observability.summary(),
      api.replay.list({ limit: 50 }),
      api.credentials.list(),
    ])

    if (summaryResult.status === 'fulfilled') setSummary(summaryResult.value)
    else setSummaryError(errorMessage(summaryResult.reason, 'Observability summary is unavailable'))
    if (replayResult.status === 'fulfilled') setReplay(replayResult.value.actions ?? [])
    else setReplayError(errorMessage(replayResult.reason, 'Replay history is unavailable'))
    if (credentialsResult.status === 'fulfilled') setCredentials(credentialsResult.value ?? [])
    else setCredentialsError(errorMessage(credentialsResult.reason, 'Credential metadata is unavailable'))
    setLoading(false)
    setRefreshing(false)
  }, [])

  useEffect(() => {
    void (async () => {
      await Promise.resolve()
      await load()
    })()
  }, [load])

  const createCredential = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (!credentialName.trim() || !credentialKind.trim() || !credentialSecret || credentialBusy) return
    setCredentialBusy(true)
    setCredentialNotice(null)
    setCredentialsError(null)
    try {
      const created = await api.credentials.create({
        name: credentialName.trim(),
        kind: credentialKind.trim(),
        secret: credentialSecret,
      })
      setCredentials((current) => [created, ...current.filter((item) => item.id !== created.id)])
      setCredentialName('')
      setCredentialSecret('')
      setCredentialNotice(`Stored metadata for ${created.name}. The secret is not displayed.`)
    } catch (error) {
      setCredentialsError(errorMessage(error, 'Unable to store credential'))
    } finally {
      setCredentialBusy(false)
    }
  }

  const deleteCredential = async (credential: Credential) => {
    if (!window.confirm(`Delete credential metadata for “${credential.name}”?`)) return
    setCredentialBusy(true)
    setCredentialNotice(null)
    setCredentialsError(null)
    try {
      await api.credentials.delete(credential.id)
      setCredentials((current) => current.filter((item) => item.id !== credential.id))
      setCredentialNotice(`Deleted ${credential.name}.`)
    } catch (error) {
      setCredentialsError(errorMessage(error, 'Unable to delete credential'))
    } finally {
      setCredentialBusy(false)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="h-9 flex items-center justify-between gap-3 px-4 border-b border-white/[0.06] shrink-0">
        <span className="text-xs text-gray-400">Observability &amp; controls</span>
        <button type="button" className="ops-button-secondary" onClick={() => void load(true)} disabled={loading || refreshing}>
          {refreshing ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      <div className="p-4 sm:p-6 overflow-y-auto flex-1 space-y-5">
        <header>
          <p className="ops-kicker">Worker operations</p>
          <h1 className="mt-2 text-xl font-medium tracking-tight text-gray-100">Runtime visibility</h1>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-gray-500">
            Live process counters, recorded governed actions, and encrypted credential metadata. Values come directly from the connected Fathom server.
          </p>
        </header>

        <section aria-labelledby="summary-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Live snapshot</p>
              <h2 id="summary-heading" className="mt-1">Runtime summary</h2>
            </div>
            {summary?.audit_counts_truncated && <span className="ops-status ops-status-deny" title="The server capped the audit sample at its safety limit">Audit sample capped</span>}
          </div>
          {summaryError ? (
            <div className="ops-alert" role="alert"><span>SERVER UNAVAILABLE</span>{summaryError}</div>
          ) : loading && !summary ? (
            <p className="ops-empty" aria-live="polite">Loading runtime summary…</p>
          ) : summary ? (
            <dl className="grid grid-cols-2 gap-px overflow-hidden border border-white/[0.06] bg-white/[0.06] sm:grid-cols-4">
              {[
                ['Active sessions', summary.active_sessions],
                ['Sessions total', summary.sessions_total],
                ['Workers spawned', summary.agents_spawned],
                ['Tool calls', summary.tool_calls],
                ['Tokens used', summary.tokens_used],
                ['Audit events', summary.audit_events],
                ['Audit denials', summary.audit_denials],
              ].map(([label, value]) => (
                <div className="bg-[#0e1010] p-3" key={label as string}>
                  <dt className="ops-label">{label}</dt>
                  <dd className="mt-2 text-lg font-medium tabular-nums text-gray-200">{formatNumber(value as number)}</dd>
                </div>
              ))}
            </dl>
          ) : null}
        </section>

        <section className="ops-panel" aria-labelledby="replay-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Recorded execution</p>
              <h2 id="replay-heading" className="mt-1">Replay timeline</h2>
            </div>
            <span className="text-[10px] text-gray-600">Newest 50</span>
          </div>
          {replayError ? (
            <div className="ops-alert" role="alert"><span>REPLAY UNAVAILABLE</span>{replayError}</div>
          ) : loading && replay.length === 0 ? (
            <p className="ops-empty" aria-live="polite">Loading recorded actions…</p>
          ) : replay.length === 0 ? (
            <p className="ops-empty">No recorded governed actions yet.</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full min-w-[680px] text-left text-xs">
                <caption className="sr-only">Recorded governed actions</caption>
                <thead className="border-b border-white/[0.08] text-[10px] uppercase tracking-[.08em] text-gray-600">
                  <tr><th className="pb-2 pr-3 font-medium">Started</th><th className="pb-2 pr-3 font-medium">Worker / session</th><th className="pb-2 pr-3 font-medium">Tool</th><th className="pb-2 pr-3 font-medium">Decision</th><th className="pb-2 font-medium">Duration</th></tr>
                </thead>
                <tbody>
                  {replay.map((action) => (
                    <tr className="border-b border-white/[0.05] last:border-0" key={action.id}>
                      <td className="py-3 pr-3 whitespace-nowrap text-gray-500">{formatDate(action.started_at)}</td>
                      <td className="max-w-[220px] py-3 pr-3"><div className="truncate text-gray-300" title={action.agent}>{action.agent}</div><div className="truncate text-[10px] text-gray-600" title={action.session}>{action.session}</div></td>
                      <td className="py-3 pr-3 font-mono text-[11px] text-gray-400">{action.tool}</td>
                      <td className="py-3 pr-3"><span className={action.decision.toLowerCase() === 'deny' ? 'ops-status ops-status-deny' : 'ops-status ops-status-allow'}>{action.decision}</span></td>
                      <td className="py-3 text-gray-500">{action.duration_ms == null ? '—' : `${formatNumber(action.duration_ms)} ms`}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        <section className="ops-panel" aria-labelledby="credentials-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Secret storage</p>
              <h2 id="credentials-heading" className="mt-1">Credential metadata</h2>
            </div>
            <span className="text-[10px] text-gray-600">Secrets never shown</span>
          </div>
          <p className="mb-4 max-w-2xl text-xs leading-5 text-gray-500">Store a secret for server-side worker use. This surface only receives its name, kind, and timestamps.</p>
          <form className="grid gap-3 border-b border-white/[0.06] pb-4 sm:grid-cols-[1fr_1fr_1.4fr_auto] sm:items-end" onSubmit={createCredential}>
            <label><span className="ops-label mb-1">Name</span><input className="ops-input" value={credentialName} onChange={(event) => setCredentialName(event.target.value)} placeholder="Provider key" required /></label>
            <label><span className="ops-label mb-1">Kind</span><input className="ops-input" value={credentialKind} onChange={(event) => setCredentialKind(event.target.value)} placeholder="api_key" required /></label>
            <label><span className="ops-label mb-1">Secret</span><input className="ops-input" type="password" value={credentialSecret} onChange={(event) => setCredentialSecret(event.target.value)} autoComplete="new-password" required /></label>
            <button className="ops-button-primary" type="submit" disabled={credentialBusy}>{credentialBusy ? 'Saving…' : 'Store secret'}</button>
          </form>
          {credentialNotice && <p className="ops-notice mt-3" role="status">{credentialNotice}</p>}
          {credentialsError ? <div className="ops-alert mt-3" role="alert"><span>CREDENTIALS UNAVAILABLE</span>{credentialsError}</div> : credentials.length === 0 ? <p className="ops-empty">No credential metadata stored.</p> : (
            <ul className="mt-3 divide-y divide-white/[0.05]" aria-label="Stored credential metadata">
              {credentials.map((credential) => <li className="flex items-center justify-between gap-3 py-3" key={credential.id}><div className="min-w-0"><p className="truncate text-xs text-gray-300">{credential.name}</p><p className="mt-1 text-[10px] text-gray-600">{credential.kind} · updated {formatDate(credential.updated_at)}</p></div><button className="ops-button-secondary shrink-0" type="button" onClick={() => void deleteCredential(credential)} disabled={credentialBusy}>Delete</button></li>)}
            </ul>
          )}
        </section>
      </div>
    </div>
  )
}
