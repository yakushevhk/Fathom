'use client'

import Link from 'next/link'
import { useEffect, useState } from 'react'
import { Bot, Hash, Hexagon, Loader2, Workflow } from 'lucide-react'
import { relTime, truncKey } from '@/lib/format'
import type { Member } from '@/lib/types'
import { Avatar } from '@/components/Avatar'
import { useHive } from '@/lib/store'

interface AgentRow extends Member {
  stats: { events: number; lastSeen: number | null; activeWorkflow: string | null; channels: number }
}

export default function AgentsPage() {
  const { liveVersion } = useHive()
  const [agents, setAgents] = useState<AgentRow[]>([])

  useEffect(() => {
    fetch('/api/agents', { cache: 'no-store' })
      .then((r) => r.json())
      .then((d) => setAgents(d.agents))
      .catch(() => {})
  }, [liveVersion])

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <header className="border-b border-line bg-panel/60 px-6 py-4">
        <h1 className="flex items-center gap-2 text-[17px] font-bold">
          <Bot size={18} className="text-acc" /> Agents
        </h1>
        <p className="mt-0.5 max-w-xl text-[12px] text-mut">
          Members, not bots. Each agent holds its own keys, its own room memberships and its own audit trail —
          scoped by identity, the same way you scope a teammate.
        </p>
      </header>
      <div className="grid grid-cols-1 gap-3 p-6 md:grid-cols-2 xl:grid-cols-3">
        {agents.map((a) => (
          <Link key={a.id} href={`/agents/${a.handle}`} className="card card-hover block p-4">
            <div className="mb-2 flex items-start gap-3">
              <Avatar member={a} size={40} />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-[14px] font-bold">{a.displayName}</span>
                  <span className="chip" style={{ color: a.accent, borderColor: `${a.accent}55` }}>
                    {a.title}
                  </span>
                </div>
                <div className="mono mt-0.5 text-[9.5px] text-dim">{truncKey(a.signature)}</div>
              </div>
              <span className="text-[10px] capitalize text-mut">{a.presence}</span>
            </div>
            <p className="mb-3 min-h-[32px] text-[12px] leading-relaxed text-mut">{a.bio}</p>
            <div className="flex items-center gap-3 text-[10.5px] text-dim">
              <span className="flex items-center gap-1">
                <Hash size={10} /> {a.stats.channels} rooms
              </span>
              <span className="flex items-center gap-1">
                <Hexagon size={10} /> {a.stats.events} events
              </span>
              <span className="ml-auto">{a.stats.lastSeen ? `active ${relTime(a.stats.lastSeen)}` : 'idle'}</span>
            </div>
            {a.stats.activeWorkflow && (
              <div className="mt-2 flex items-center gap-1.5 rounded-lg bg-accsoft px-2 py-1 text-[10.5px] font-medium text-acc">
                <Loader2 size={10} className="animate-spin" />
                <Workflow size={10} /> {a.stats.activeWorkflow}
              </div>
            )}
          </Link>
        ))}
      </div>
    </div>
  )
}
