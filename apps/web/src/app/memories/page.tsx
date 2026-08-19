'use client'

import { useEffect, useState } from 'react'
import { api, type Memory, type MemoryStats } from '@/lib/api'

export default function MemoriesPage() {
  const [memories, setMemories] = useState<Memory[]>([])
  const [stats, setStats] = useState<MemoryStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [actionMsg, setActionMsg] = useState('')

  const load = async () => {
    try {
      const [mResp, sResp] = await Promise.all([
        api.memories.list(),
        api.memories.stats(),
      ])
      setMemories(mResp.memories)
      setStats(sResp)
    } catch { /* ignore */ }
    setLoading(false)
  }

  useEffect(() => { load() }, [])

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
        Semantic Memory
      </div>
      <div className="p-6 overflow-y-auto flex-1">
        {/* Stats + actions */}
        <div className="flex items-center gap-4 mb-4 text-xs text-gray-400">
          {stats && (
            <>
              <span>{stats.total_facts} facts</span>
              <span>{stats.archived_facts} archived</span>
              <span>{stats.cache_entries} cache entries</span>
            </>
          )}
          <div className="ml-auto flex gap-1">
            <button onClick={() => doAction('Absorb', () => api.memories.absorb())}
              className="px-2 py-1 rounded border border-white/[0.06] text-[10px] text-gray-500 hover:text-gray-300 hover:border-white/20">
              Absorb
            </button>
            <button onClick={() => doAction('Distill', () => api.memories.distill())}
              className="px-2 py-1 rounded border border-white/[0.06] text-[10px] text-gray-500 hover:text-gray-300 hover:border-white/20">
              Distill
            </button>
            <button onClick={() => doAction('GC', () => api.memories.gc())}
              className="px-2 py-1 rounded border border-white/[0.06] text-[10px] text-gray-500 hover:text-gray-300 hover:border-white/20">
              GC
            </button>
          </div>
        </div>

        {actionMsg && (
          <div className="mb-4 text-xs text-blue-400 animate-fade-in">{actionMsg}</div>
        )}

        {loading ? (
          <div className="flex items-center justify-center py-8">
            <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
          </div>
        ) : memories.length === 0 ? (
          <p className="text-gray-600 text-xs">No memories yet</p>
        ) : (
          <div className="space-y-2">
            {memories.map(m => (
              <div key={m.id} className="p-3 rounded-md border border-white/[0.06] bg-[#141414]">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-[10px] font-mono text-gray-500">{m.type}</span>
                  {m.archived && <span className="text-[10px] text-gray-600">archived</span>}
                  <button onClick={() => doArchive(m.id)}
                    className="ml-auto text-[10px] text-gray-500 hover:text-red-400 px-1.5 py-0.5 rounded border border-white/[0.06] hover:border-red-500/30">
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