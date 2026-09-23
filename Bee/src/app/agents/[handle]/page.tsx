'use client'

import Link from 'next/link'
import { useParams } from 'next/navigation'
import { useEffect, useState } from 'react'
import { Bot, ChevronRight, GitPullRequest, Hash, Hexagon, KeyRound } from 'lucide-react'
import { relTime, truncKey } from '@/lib/format'
import type { Channel, HiveEvent, Member, Workflow } from '@/lib/types'
import { Avatar } from '@/components/Avatar'
import { Markdown } from '@/components/Markdown'
import { useHive } from '@/lib/store'

interface Detail {
  agent: Member
  events: HiveEvent[]
  channels: string[]
  workflows: Workflow[]
}

export default function AgentDetailPage() {
  const { handle } = useParams<{ handle: string }>()
  const { channels, liveVersion } = useHive()
  const [data, setData] = useState<Detail | null>(null)

  useEffect(() => {
    fetch(`/api/agents/${handle}`, { cache: 'no-store' })
      .then((r) => (r.ok ? r.json() : null))
      .then(setData)
      .catch(() => {})
  }, [handle, liveVersion])

  if (!data) return <div className="flex flex-1 items-center justify-center text-[12px] text-dim">Loading…</div>
  const { agent, events, channels: chIds, workflows } = data
  const chById = new Map(channels.map((c: Channel) => [c.id, c]))

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <header className="border-b border-line bg-panel/60 px-6 py-4">
        <div className="flex items-center gap-4">
          <Avatar member={agent} size={52} />
          <div>
            <h1 className="flex items-center gap-2 text-[17px] font-bold">
              {agent.displayName}
              <span className="chip" style={{ color: agent.accent, borderColor: `${agent.accent}55` }}>
                <Hexagon size={9} /> {agent.title}
              </span>
            </h1>
            <p className="text-[12px] text-mut">
              @{agent.handle} · {agent.model ?? 'fathom-rt'} · {agent.presence}
            </p>
          </div>
          <div className="card ml-auto flex items-center gap-2 px-3 py-2">
            <KeyRound size={13} className="text-acc" />
            <span className="mono text-[10.5px] text-mut">{truncKey(agent.signature)}</span>
          </div>
        </div>
        <p className="mt-3 max-w-xl text-[12.5px] leading-relaxed text-mut">{agent.bio}</p>
      </header>

      <div className="grid grid-cols-1 gap-4 p-6 lg:grid-cols-3">
        <section className="card p-4">
          <h2 className="mb-2 flex items-center gap-1.5 text-[12px] font-bold uppercase tracking-wider text-dim">
            <Hash size={12} /> Rooms
          </h2>
          {chIds.map((id) => {
            const c = chById.get(id)
            return (
              <Link key={id} href={`/c/${c?.slug ?? 'general'}`} className="navlink">
                <Hash size={13} style={{ color: c?.accent }} /> {c?.slug ?? id}
              </Link>
            )}
          )}
        </section>

        <section className="card p-4">
          <h2 className="mb-2 flex items-center gap-1.5 text-[12px] font-bold uppercase tracking-wider text-dim">
            <GitPullRequest size={12} /> Workflows
          </h2>
          {workflows.length === 0 && <p className="text-[12px] text-dim">No workflow runs yet.</p>}
          {workflows.map((w) => (
            <Link key={w.id} href={`/workflows/${w.id}`} className="mb-1.5 flex items-center gap-2 rounded-lg px-2 py-1.5 text-[12.5px] hover:bg-accsoft/60">
              <ChevronRight size={12} className="text-dim" />
              <span className="min-w-0 flex-1 truncate">{w.name}</span>
              <span
                className="chip"
                style={{
                  color:
                    w.status === 'succeeded' ? 'var(--ok)' : w.status === 'failed' ? 'var(--danger)' : 'var(--acc-2)',
                }}
              >
                {w.status}
              </span>
            </Link>
          ))}
        </section>

        <section className="card p-4">
          <h2 className="mb-2 flex items-center gap-1.5 text-[12px] font-bold uppercase tracking-wider text-dim">
            <Bot size={12} /> About
          </h2>
          <dl className="space-y-1.5 text-[12px]">
            <div className="flex justify-between">
              <dt className="text-dim">Kind</dt>
              <dd>agent — own keys, own audit trail</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-dim">Runtime</dt>
              <dd className="mono">{agent.model ?? 'fathom-rt 1.4'}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-dim">Presence</dt>
              <dd className="capitalize">{agent.presence}</dd>
            </div>
          </dl>
        </section>
      </div>

      <section className="px-6 pb-6">
        <h2 className="mb-2 text-[12px] font-bold uppercase tracking-wider text-dim">Recent events</h2>
        <div className="card divide-y divide-line">
          {events
            .slice()
            .reverse()
            .slice(0, 12)
            .map((e) => {
              const c = chById.get(e.channelId)
              return (
                <Link
                  key={e.id}
                  href={`/c/${c?.slug ?? 'general'}`}
                  className="flex items-start gap-3 px-4 py-2.5 hover:bg-accsoft/40"
                >
                  <span className="chip mt-0.5 shrink-0" style={{ color: c?.accent ?? 'var(--mut)' }}>
                    #{c?.slug ?? '?'}
                  </span>
                  <div className="min-w-0 flex-1 text-[12.5px]">
                    <Markdown text={e.body.slice(0, 140)} />
                  </div>
                  <span className="shrink-0 text-[10px] text-dim">{relTime(e.createdAt)}</span>
                </Link>
              )
            })}
          {events.length === 0 && <div className="px-4 py-6 text-center text-[12px] text-dim">No events yet.</div>}
        </div>
      </section>
    </div>
  )
}
