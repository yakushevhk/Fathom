'use client'

import { useEffect, useState } from 'react'
import { api, type Agent } from '@/lib/api'

export default function AgentsPage() {
  const [agents, setAgents] = useState<Agent[]>([])
  const [loading, setLoading] = useState(true)
  const [expandId, setExpandId] = useState<string | null>(null)

  useEffect(() => {
    const load = async () => {
      try {
        const resp = await api.agents.list()
        setAgents(resp.agents ?? [])
      } catch { /* ignore */ }
      setLoading(false)
    }
    load()
  }, [])

  const grouped = agents.reduce<Record<string, Agent[]>>((acc, a) => {
    (acc[a.session_id] ??= []).push(a)
    return acc
  }, {})

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        Agents — {agents.length} total
      </div>
      <div className="p-6 overflow-y-auto flex-1">
        {loading ? (
          <div className="flex items-center justify-center py-8">
            <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
          </div>
        ) : Object.entries(grouped).length === 0 ? (
          <p className="text-gray-600 text-xs">No agents</p>
        ) : (
          <div className="space-y-4">
            {Object.entries(grouped).map(([sessionId, group]) => (
              <div key={sessionId} className="rounded-md border border-white/[0.06] bg-[#141414]">
                <div className="px-3 py-2 border-b border-white/[0.06] text-xs text-gray-400 font-mono">
                  {sessionId.slice(0, 8)} — {group.length} agents
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
                          onClick={() => setExpandId(expandId === a.id ? null : a.id)}
                          className="ml-auto text-gray-500 hover:text-gray-300 text-[10px]"
                        >
                          {expandId === a.id ? '▾' : '▸'}
                        </button>
                      </div>
                      {expandId === a.id && (
                        <div className="mt-2 ml-4 text-xs text-gray-500 space-y-1">
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