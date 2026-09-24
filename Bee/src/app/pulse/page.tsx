'use client'

import Link from 'next/link'
import { useEffect, useMemo, useState } from 'react'
import { Activity } from 'lucide-react'
import { cn } from '@/lib/cn'
import { relTime } from '@/lib/format'
import { useHive } from '@/lib/store'
import type { HiveEvent } from '@/lib/types'
import { MessageItem } from '@/components/MessageItem'

const KINDS = ['all', 'message', 'patch', 'ci', 'approval', 'workflow', 'member', 'system'] as const

// Pulse: the whole community as one stream — every event kind, one index.
export default function PulsePage() {
  const { liveEvents, members, channels } = useHive()
  const [kind, setKind] = useState<(typeof KINDS)[number]>('all')
  const [events, setEvents] = useState<HiveEvent[]>([])

  useEffect(() => {
    fetch(`/api/pulse${kind === 'all' ? '' : `?kind=${kind}`}`, { cache: 'no-store' })
      .then((r) => r.json())
      .then((d) => setEvents(d.events))
      .catch(() => {})
  }, [kind])

  // Merge the store's full SSE queue by id — a single lastEvent slot can drop
  // events that coalesce into one React render.
  const shown = useMemo(() => {
    const byId = new Map<number, HiveEvent>()
    for (const e of events) byId.set(e.id, e)
    for (const e of liveEvents) byId.set(e.id, e)
    return [...byId.values()].filter((e) => kind === 'all' || e.kind === kind).sort((a, b) => b.id - a.id)
  }, [events, liveEvents, kind])

  const memberById = new Map(members.map((m) => [m.id, m]))
  const chById = new Map(channels.map((c) => [c.id, c]))

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex items-center gap-3 border-b border-line bg-panel/60 px-6 py-3">
        <h1 className="flex items-center gap-2 text-[15px] font-bold">
          <Activity size={16} className="text-acc" /> Pulse
        </h1>
        <div className="flex flex-wrap gap-1">
          {KINDS.map((k) => (
            <button
              key={k}
              onClick={() => setKind(k)}
              className={cn('chip capitalize', kind === k && 'border-acc text-acc')}
              style={kind === k ? { background: 'var(--acc-soft)' } : undefined}
            >
              {k}
            </button>
          ))}
        </div>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto py-2">
        {shown.map((e) => {
          const c = chById.get(e.channelId)
          return (
            <div key={e.id} className="msg-in flex gap-3 px-4 py-1.5">
              <Link
                href={`/c/${c?.slug ?? 'general'}`}
                className="chip mt-1 h-fit w-[132px] shrink-0 justify-center truncate"
                style={{ color: c?.accent ?? 'var(--mut)' }}
                title={c?.name}
              >
                #{c?.slug ?? '?'}
              </Link>
              <div className="min-w-0 flex-1 border-b border-line pb-2">
                {e.kind === 'message' || e.kind === 'member' || e.kind === 'system' ? (
                  <div className="flex items-baseline gap-2">
                    <span className="text-[12px] font-bold" style={{ color: memberById.get(e.authorId)?.accent }}>
                      {memberById.get(e.authorId)?.displayName ?? e.authorId}
                    </span>
                    <span className="text-[10px] text-dim">{relTime(e.createdAt)}</span>
                    <span className="truncate text-[12px] text-mut">{e.body.replace(/\*\*/g, '').slice(0, 160)}</span>
                  </div>
                ) : (
                  <div className="-mx-4">
                    <MessageItem event={e} member={memberById.get(e.authorId)} />
                  </div>
                )}
              </div>
            </div>
          )
        })}
        {shown.length === 0 && <div className="py-10 text-center text-[12px] text-dim">The log is quiet.</div>}
      </div>
    </div>
  )
}
