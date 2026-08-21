'use client'

import { useEffect, useState } from 'react'
import { api, type Memory, type MemoryStats } from '@/lib/api'

export default function MemoriesPage() {
  const [memories, setMemories] = useState<Memory[]>([])
  const [stats, setStats] = useState<MemoryStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [actionMsg, setActionMsg] = useState('')
  const [absorbContent, setAbsorbContent] = useState('')

  const load = async () => {
    setLoading(true)
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
    }
  }

  useEffect(() => {
    void (async () => {
      await Promise.resolve()
      await load()
    })()
  }, [])

  const doAction = async (name: string, fn: () => Promise<unknown>) => {
    setActionMsg(`${name}...`)
    try {
      const result = await fn()
      setActionMsg(`${name} done: ${JSON.stringify(result)}`)
      await load()
    } catch (e) {
      setActionMsg(`${name} failed: ${e}`)
    }
    setTimeout(() => setActionMsg(''), 5000)
  }

  const doArchive = async (id: string) => {
    setActionMsg('Archiving...')
    try {
      await api.memories.archive(id)
      setActionMsg('Archived')
      await load()
    } catch (e) { setActionMsg(`Archive failed: ${e}`) }
    setTimeout(() => setActionMsg(''), 3000)
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        Worker Memory
      </div>
      <div className="p-6 overflow-y-auto flex-1">
        {/* Stats + actions */}
        <div className="flex items-center gap-4 mb-4 text-xs text-gray-400">
          {stats && (
            <>
              <span>{Object.values(stats.scopes).reduce((total, scope) => total + scope.active, 0)} active</span>
              <span>{Object.values(stats.scopes).reduce((total, scope) => total + scope.archived, 0)} archived</span>
              <span>{stats.entity_graph.nodes} entities</span>
            </>
          )}
          <div className="ml-auto flex gap-1">
            <label className="sr-only" htmlFor="memory-fact">Fact to absorb</label>
            <input id="memory-fact" value={absorbContent} onChange={e => setAbsorbContent(e.target.value)} placeholder="Fact to absorb" className="px-2 py-1 rounded border border-white/[0.06] bg-transparent text-[10px] text-gray-300 focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-300" />
            <button type="button" aria-label="Absorb fact into worker memory" onClick={() => doAction('Absorb', async () => {
              const content = absorbContent.trim()
              if (!content) throw new Error('Enter fact content before absorbing')
              const result = await api.memories.absorb(content)
              setAbsorbContent('')
              return result
            })} disabled={!absorbContent.trim()}
              className="px-2 py-1 rounded border border-white/[0.06] text-[10px] text-gray-500 hover:text-gray-300 hover:border-white/20 focus-visible:outline focus-visible:outline-2 focus-visible:outline-gray-300 disabled:opacity-30">
              Absorb
            </button>
            <button type="button" aria-label="Distill worker memory" onClick={() => doAction('Distill', () => api.memories.distill())}
              className="px-2 py-1 rounded border border-white/[0.06] text-[10px] text-gray-500 hover:text-gray-300 hover:border-white/20">
              Distill
            </button>
            <button type="button" aria-label="Clean up worker memory" onClick={() => doAction('GC', () => api.memories.gc())}
              className="px-2 py-1 rounded border border-white/[0.06] text-[10px] text-gray-500 hover:text-gray-300 hover:border-white/20">
              GC
            </button>
          </div>
        </div>

        {actionMsg && (
          <div className="mb-4 text-xs text-blue-400 animate-fade-in">{actionMsg}</div>
        )}

        {loading ? (
          <div className="flex items-center justify-center py-8" aria-label="Loading worker memory" role="status">
            <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
          </div>
        ) : error ? (
          <div role="alert" className="rounded-md border border-red-500/20 bg-red-500/5 p-4 text-xs text-red-300">
            <p>Worker memory could not be loaded: {error}</p>
            <button type="button" onClick={load} className="mt-3 rounded border border-red-400/30 px-2 py-1 text-red-200 hover:bg-red-500/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-red-300">Retry</button>
          </div>
        ) : memories.length === 0 ? (
          <p className="text-gray-600 text-xs">No worker memory yet</p>
        ) : (
          <div className="space-y-2">
            {memories.map(m => (
              <div key={m.id} className="p-3 rounded-md border border-white/[0.06] bg-[#141414]">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] font-mono text-gray-500">{m.scope}:{m.scope_key}</span>
                  <span className="text-[10px] text-gray-600">{m.status}</span>
                  <button type="button" aria-label={`Archive memory from ${m.scope}`} onClick={() => doArchive(m.id)}
                    className="ml-auto text-[10px] text-gray-500 hover:text-red-400 px-1.5 py-0.5 rounded border border-white/[0.06] hover:border-red-500/30 focus-visible:outline focus-visible:outline-2 focus-visible:outline-red-300">
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