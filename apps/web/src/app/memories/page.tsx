'use client'

import { useEffect, useState } from 'react'
import { useSessions } from '@/hooks/useSessions'
import { api, type DistillReport, type GcReport, type Memory, type MemoryStats } from '@/lib/api'

export default function MemoriesPage() {
  const [memories, setMemories] = useState<Memory[]>([])
  const [stats, setStats] = useState<MemoryStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [actionMsg, setActionMsg] = useState('')
  const [absorbContent, setAbsorbContent] = useState('')
  const [refreshing, setRefreshing] = useState(false)
  const [busyAction, setBusyAction] = useState<string | null>(null)
  const [selectedSession, setSelectedSession] = useState('')
  const { sessions, loading: sessionsLoading, error: sessionsError } = useSessions()

  const availableSessions = sessions
  const selectedSessionExists = availableSessions.some(session => session.id === selectedSession)
  const distillReady = !sessionsLoading && !sessionsError && selectedSessionExists

  useEffect(() => {
    if (!selectedSessionExists) setSelectedSession(availableSessions[0]?.id ?? '')
  }, [availableSessions, selectedSessionExists])

  const load = async (isRefresh = false) => {
    if (isRefresh) setRefreshing(true)
    else setLoading(true)
    setError(null)
    try {
      const [mResp, sResp] = await Promise.all([
        api.memories.list(),
        api.memories.stats(),
      ])
      setMemories(mResp.memories ?? [])
      setStats(sResp)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unable to load memory')
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }

  useEffect(() => {
    void (async () => {
      await Promise.resolve()
      await load()
    })()
  }, [])

  const describeResult = (name: string, result: unknown) => {
    if (name === 'Distill') {
      const report = result as DistillReport
      return `${report.promoted} promoted, ${report.skipped} already known, ${report.archived} archived${report.errors ? `, ${report.errors} errors` : ''}${report.dry_run ? ' (dry run)' : ''}`
    }
    if (name === 'GC') {
      const report = result as GcReport
      const archived = report.expired_archived + report.stale_archived + report.confidence_archived
      return `${archived} archived, ${report.groups_compacted + report.facts_compacted} compacted${report.errors ? `, ${report.errors} errors` : ''}${report.dry_run ? ' (dry run)' : ''}`
    }
    return 'completed'
  }

  const doAction = async (name: string, fn: () => Promise<unknown>) => {
    if (busyAction) return
    setBusyAction(name)
    setActionMsg(`${name}…`)
    try {
      const result = await fn()
      setActionMsg(`${name} complete: ${describeResult(name, result)}`)
      await load(true)
    } catch (e) {
      setActionMsg(`${name} failed: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setBusyAction(null)
      window.setTimeout(() => setActionMsg(''), 5000)
    }
  }

  const doArchive = async (id: string) => {
    if (busyAction || !window.confirm('Archive this memory? It will remain available in history.')) return
    setBusyAction(`archive:${id}`)
    setActionMsg('Archiving…')
    try {
      await api.memories.archive(id)
      setActionMsg('Memory archived')
      await load(true)
    } catch (e) {
      setActionMsg(`Archive failed: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setBusyAction(null)
      window.setTimeout(() => setActionMsg(''), 3000)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="ops-toolbar shrink-0">
        <span className="ops-toolbar-title">Worker Memory Store</span>
        <button type="button" className="ops-button-secondary ml-auto" onClick={() => void load(true)} disabled={loading || refreshing}>
          {refreshing ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>
      <div className="p-4 sm:p-6 overflow-y-auto flex-1 space-y-5">
        <header className="ops-page-header">
          <p className="ops-kicker">Knowledge base</p>
          <h1 className="mt-2 text-xl font-medium tracking-tight text-gray-100">Durable worker memory</h1>
          <p className="mt-1 max-w-2xl text-xs leading-5 text-gray-500">Review stored context and run conservative maintenance. Archive is reversible; maintenance only archives records and never deletes history.</p>
        </header>
        {/* Stats + actions */}
        <div className="ops-panel flex flex-col gap-4 text-xs text-gray-400" aria-label="Memory statistics and actions">
          {stats && (
            <dl className="flex flex-wrap gap-x-5 gap-y-1" aria-live="polite">
              <div><dt className="inline text-gray-500">Active </dt><dd className="inline text-gray-300">{Object.values(stats.scopes).reduce((total, scope) => total + scope.active, 0)}</dd></div>
              <div><dt className="inline text-gray-500">Archived </dt><dd className="inline text-gray-300">{Object.values(stats.scopes).reduce((total, scope) => total + scope.archived, 0)}</dd></div>
              <div><dt className="inline text-gray-500">Entities </dt><dd className="inline text-gray-300">{stats.entity_graph.nodes}</dd></div>
            </dl>
          )}
          <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center sm:justify-end">
            <label className="sr-only" htmlFor="memory-fact">Memory to absorb</label>
            <input id="memory-fact" value={absorbContent} onChange={e => setAbsorbContent(e.target.value)} placeholder="Memory to absorb" className="ops-input min-w-0 sm:w-56" disabled={busyAction !== null} />
            <button type="button" aria-label="Absorb fact into worker memory" onClick={() => void doAction('Absorb', async () => {
              const content = absorbContent.trim()
              if (!content) throw new Error('Enter fact content before absorbing')
              const result = await api.memories.absorb(content)
              setAbsorbContent('')
              return result
            })} disabled={!absorbContent.trim() || busyAction !== null} className="ops-button-primary">
              {busyAction === 'Absorb' ? 'Absorbing…' : 'Absorb'}
            </button>
            <label className="sr-only" htmlFor="distill-session">Session for distillation</label>
            <select
              id="distill-session"
              value={selectedSession}
              onChange={e => setSelectedSession(e.target.value)}
              disabled={busyAction !== null || sessionsLoading || availableSessions.length === 0}
              className="ops-input min-w-0 sm:w-64"
              aria-describedby="distill-help"
            >
              <option value="">Select a session to distill</option>
              {availableSessions.map(session => (
                <option key={session.id} value={session.id}>
                  {session.query ? `${session.query.slice(0, 44)} · ` : ''}{session.id.slice(0, 8)} ({session.status})
                </option>
              ))}
            </select>
            <button type="button" aria-label="Distill worker memory for selected session" onClick={() => void doAction('Distill', () => {
              if (!selectedSession || !selectedSessionExists) throw new Error('Select an existing session before distilling')
              return api.memories.distill({ session: selectedSession })
            })} disabled={busyAction !== null || !distillReady} className="ops-button-secondary">
              {busyAction === 'Distill' ? 'Distilling…' : 'Distill'}
            </button>
            <button type="button" aria-label="Clean up worker memory" onClick={() => void doAction('GC', () => api.memories.gc())} disabled={busyAction !== null} className="ops-button-secondary">
              {busyAction === 'GC' ? 'Cleaning…' : 'GC'}
            </button>
          </div>
          <p id="distill-help" className="text-[11px] leading-4 text-gray-500">
            {sessionsLoading
              ? 'Loading sessions…'
              : sessionsError
                ? `Sessions unavailable: ${sessionsError}. Distill requires an existing session.`
                : availableSessions.length === 0
                  ? 'Distill requires an existing session. Start or complete a session from the sidebar, then return here.'
                  : 'Distill only promotes run-scoped facts from the selected session.'}
          </p>
        </div>

        {actionMsg && (
          <div className={`ops-${actionMsg.includes('failed') ? 'alert' : 'notice'} animate-fade-in`} role={actionMsg.includes('failed') ? 'alert' : 'status'}>{actionMsg}</div>
        )}

        {loading ? (
          <div className="flex items-center justify-center py-8" aria-label="Loading worker memory" role="status">
            <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
          </div>
        ) : error ? (
          <div role="alert" className="rounded-md border border-red-500/20 bg-red-500/5 p-4 text-xs text-red-300">
            <p>Worker memory could not be loaded: {error}</p>
            <button type="button" onClick={() => void load(true)} disabled={refreshing} className="mt-3 rounded border border-red-400/30 px-2 py-1 text-red-200 hover:bg-red-500/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-red-300">Retry</button>
          </div>
        ) : memories.length === 0 ? (
          <p className="text-gray-600 text-xs">No worker memories yet. Absorb context to give future runs durable memory.</p>
        ) : (
          <div className="space-y-2">
            {memories.map(m => (
              <div key={m.id} className="p-3 rounded-md border border-white/[0.06] bg-[#141414]">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] font-mono text-gray-500">{m.scope}:{m.scope_key}</span>
                  <span className="text-[10px] text-gray-600">{m.status}</span>
                  <button type="button" aria-label={`Archive memory from ${m.scope}`} onClick={() => doArchive(m.id)}
                    className="ml-auto text-[10px] text-gray-500 hover:text-red-400 px-1.5 py-0.5 rounded border border-white/[0.06] hover:border-red-500/30 focus-visible:outline focus-visible:outline-2 focus-visible:outline-red-300 disabled:opacity-40" disabled={busyAction !== null}>
                    Archive
                  </button>
                </div>
                <p className="text-xs text-gray-300 whitespace-pre-wrap line-clamp-3">{m.content}</p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}