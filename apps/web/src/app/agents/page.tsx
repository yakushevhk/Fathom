'use client'

import { useEffect, useState } from 'react'
import { api, type Agent } from '@/lib/api'

export default function AgentsPage() {
  const [agents, setAgents] = useState<Agent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [expandId, setExpandId] = useState<string | null>(null)

  const load = async () => {
    setLoading(true)
    setError(null)
    try {
      const resp = await api.agents.list()
      setAgents(resp.agents ?? [])
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unable to load workers')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void (async () => {
      await Promise.resolve()
      await load()
    })()
  }, [])

  const grouped = agents.reduce<Record<string, Agent[]>>((acc, a) => {
    (acc[a.session_id] ??= []).push(a)
    return acc
  }, {})

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        Worker runtime — {agents.length} total
      </div>
      <div className="p-6 overflow-y-auto flex-1">
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
          <p className="text-gray-600 text-xs">No active workers yet. Dispatch a session or job to start runtime work.</p>
        ) : (
          <div className="space-y-4">
            {Object.entries(grouped).map(([sessionId, group]) => (
              <div key={sessionId} className="rounded-md border border-white/[0.06] bg-[#141414]">
                <div className="px-3 py-2 border-b border-white/[0.06] text-xs text-gray-400 font-mono">
                  {sessionId.slice(0, 8)} — {group.length} workers
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
                        }`} />
                        <span className="text-gray-200 font-medium">{a.role}</span>
                        <span className="text-gray-500">{a.status}</span>
                        {a.depth > 0 && <span className="text-gray-600">depth {a.depth}</span>}
                        {a.tokens_used > 0 && <span className="text-gray-500">{a.tokens_used} tok</span>}
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