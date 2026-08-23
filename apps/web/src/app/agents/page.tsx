'use client'

import { useEffect, useMemo, useState } from 'react'
import { api, type Agent } from '@/lib/api'

export default function AgentsPage() {
  const [agents, setAgents] = useState<Agent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [expandId, setExpandId] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null)

  const load = async () => {
    setLoading(true)
    setError(null)
    try {
      const resp = await api.agents.list()
      setAgents(resp.agents ?? [])
      setLastUpdated(new Date())
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unable to load workers')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    const initial = window.setTimeout(() => void load(), 0)
    const timer = window.setInterval(() => void load(), 15000)
    return () => {
      window.clearTimeout(initial)
      window.clearInterval(timer)
    }
  }, [])

  const filteredAgents = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return agents
    return agents.filter(a => [a.role, a.task, a.status, a.id, a.session_id].some(value => value.toLowerCase().includes(needle)))
  }, [agents, query])

  const grouped = filteredAgents.reduce<Record<string, Agent[]>>((acc, a) => {
    (acc[a.session_id] ??= []).push(a)
    return acc
  }, {})
  const activeCount = agents.filter(a => a.status === 'running').length
  const completedCount = agents.filter(a => a.status === 'completed').length
  const failedCount = agents.filter(a => a.status === 'failed').length
  const totalTokens = agents.reduce((sum, a) => sum + (a.tokens_used || 0), 0)
  const formatTokens = (value: number) => value >= 1000000 ? `${(value / 1000000).toFixed(1)}M` : value >= 1000 ? `${(value / 1000).toFixed(1)}k` : String(value)
  const formatDate = (value: string | null) => value ? new Date(value).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' }) : '—'

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="min-h-9 flex flex-wrap items-center gap-2 px-4 py-2 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        <h1 className="font-medium text-gray-300">Worker runtime</h1>
        <span className="text-gray-600">{agents.length} total</span>
        <span className="ml-auto text-[10px] text-gray-600">{lastUpdated ? `Updated ${formatDate(lastUpdated.toISOString())}` : 'Not yet synced'}</span>
        <button type="button" onClick={() => void load()} disabled={loading} className="rounded border border-white/10 px-2 py-1 text-[10px] text-gray-400 hover:text-gray-200 disabled:opacity-40 focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-300">{loading ? 'Refreshing…' : 'Refresh'}</button>
      </div>
      <div className="p-4 sm:p-6 overflow-y-auto flex-1">
        {!loading && !error && (
          <>
            <div className="grid grid-cols-2 lg:grid-cols-4 gap-2 mb-4" aria-label="Worker runtime summary">
              {[
                ['Running', activeCount, 'text-blue-300'],
                ['Completed', completedCount, 'text-green-300'],
                ['Failed', failedCount, 'text-red-300'],
                ['Tokens', formatTokens(totalTokens), 'text-gray-200'],
              ].map(([label, value, color]) => <div key={label} className="rounded-md border border-white/[0.06] bg-[#141414] px-3 py-2"><div className="text-[10px] uppercase tracking-wider text-gray-600">{label}</div><div className={`mt-1 text-lg font-semibold ${color}`}>{value}</div></div>)}
            </div>
            <label className="mb-4 block"><span className="sr-only">Search workers</span><input value={query} onChange={e => setQuery(e.target.value)} placeholder="Search role, task, status, or ID…" className="w-full rounded-md border border-white/[0.08] bg-[#141414] px-3 py-2 text-sm text-gray-200 placeholder-gray-600 outline-none focus:border-gray-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-300" /></label>
          </>
        )}
        {loading ? (
          <div role="status" aria-label="Loading workers" className="flex items-center justify-center py-8">
            <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
          </div>
        ) : error ? (
          <div role="alert" className="rounded-md border border-red-500/20 bg-red-500/5 p-4 text-xs text-red-300">
            <p>Workers could not be loaded: {error}</p>
            <button type="button" onClick={load} className="mt-3 rounded border border-red-400/30 px-2 py-1 text-red-200 hover:bg-red-500/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-red-300">
              Retry
            </button>
          </div>
        ) : Object.entries(grouped).length === 0 ? (
          <div className="rounded-md border border-dashed border-white/[0.1] p-6 text-center text-xs text-gray-500">{query ? 'No workers match this search.' : 'No active workers yet. Dispatch a session or job to start runtime work.'}</div>
        ) : (
          <div className="space-y-4">
            {Object.entries(grouped).map(([sessionId, group]) => (
              <div key={sessionId} className="rounded-md border border-white/[0.06] bg-[#141414]">
                <div className="flex flex-wrap items-center gap-2 px-3 py-2 border-b border-white/[0.06] text-xs text-gray-400 font-mono">
                  <span>{sessionId.slice(0, 8)} — {group.length} workers</span>
                  <a href={`/chat/${encodeURIComponent(sessionId)}`} className="ml-auto text-blue-300 hover:text-blue-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-blue-300">Open session ↗</a>
                </div>
                <div className="divide-y divide-white/[0.03]">
                  {group.map(a => (
                    <div key={a.id} className="px-3 py-2">
                      <div className="flex items-center gap-2 text-xs">
                        <span className="font-mono" style={{ marginLeft: `${a.depth * 12}px` }}>
                          {a.depth > 0 && '└ '}
                        </span>
                        <span className={`w-1.5 h-1.5 rounded-full ${
                          a.status === 'completed' ? 'bg-green-500' :
                          a.status === 'failed' ? 'bg-red-500' :
                          a.status === 'running' ? 'bg-blue-400 animate-pulse' : 'bg-gray-600'
                        }`} aria-hidden="true" />
                        <span className="text-gray-200 font-medium">{a.role}</span>
                        <span className={`rounded-full border px-1.5 py-0.5 text-[10px] ${a.status === 'completed' ? 'border-green-500/20 text-green-300' : a.status === 'failed' ? 'border-red-500/20 text-red-300' : a.status === 'running' ? 'border-blue-500/20 text-blue-300' : 'border-white/10 text-gray-500'}`}>{a.status}</span>
                        {a.depth > 0 && <span className="text-gray-600">depth {a.depth}</span>}
                        {a.tokens_used > 0 && <span className="text-gray-500">{formatTokens(a.tokens_used)} tok</span>}
                        <span className="text-gray-600">{formatDate(a.completed_at ?? a.created_at)}</span>
                        <button
                          type="button"
                          aria-expanded={expandId === a.id}
                          aria-controls={`worker-details-${a.id}`}
                          aria-label={`${expandId === a.id ? 'Hide' : 'Show'} details for ${a.role}`}
                          onClick={() => setExpandId(expandId === a.id ? null : a.id)}
                          className="ml-auto rounded px-1 text-gray-500 hover:text-gray-300 focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-300 text-[10px]"
                        >
                          {expandId === a.id ? '▾' : '▸'}
                        </button>
                      </div>
                      {expandId === a.id && (
                        <div id={`worker-details-${a.id}`} className="mt-2 ml-4 text-xs text-gray-500 space-y-1">
                          <div className="text-gray-400 font-mono">{a.task}</div>
                          <div className="text-gray-600">ID: {a.id}</div>
                          {a.completed_at && <div className="text-gray-600">Completed: {new Date(a.completed_at).toLocaleString()}</div>}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}