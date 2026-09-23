'use client'

import Link from 'next/link'
import { useEffect, useRef, useState } from 'react'
import { Hash, Search as SearchIcon } from 'lucide-react'
import { relTime } from '@/lib/format'
import { useHive } from '@/lib/store'
import type { Channel, HiveEvent, Member } from '@/lib/types'
import { Avatar } from '@/components/Avatar'
import { Markdown } from '@/components/Markdown'

interface Results {
  events: HiveEvent[]
  channels: Channel[]
  members: Member[]
}

export default function SearchPage() {
  const { members: allMembers, channels: allChannels } = useHive()
  const [q, setQ] = useState('')
  const [fetched, setFetched] = useState<{ q: string; data: Results } | null>(null)
  const results = q.trim().length >= 2 && fetched?.q === q ? fetched.data : null
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => inputRef.current?.focus(), [])

  useEffect(() => {
    if (q.trim().length < 2) return
    const needle = q
    const t = setTimeout(() => {
      fetch(`/api/search?q=${encodeURIComponent(needle)}`, { cache: 'no-store' })
        .then((r) => r.json())
        .then((data: Results) => setFetched({ q: needle, data }))
        .catch(() => {})
    }, 200)
    return () => clearTimeout(t)
  }, [q])

  const memberById = new Map(allMembers.map((m) => [m.id, m]))
  const chById = new Map(allChannels.map((c) => [c.id, c]))

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <header className="border-b border-line bg-panel/60 px-6 py-4">
        <h1 className="flex items-center gap-2 text-[17px] font-bold">
          <SearchIcon size={18} className="text-acc" /> Search
        </h1>
        <p className="mt-0.5 text-[12px] text-mut">
          Messages, patches, workflows and approvals — one index, because they’re all the same kind of event.
        </p>
        <input
          ref={inputRef}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search the whole log…"
          className="mt-3 w-full max-w-xl rounded-lg border border-line2 bg-bg2 px-3 py-2 text-[13.5px] outline-none focus:border-acc"
        />
      </header>
      <div className="p-6">
        {!results && (
          <div className="card mx-auto max-w-xl p-8 text-center text-[12.5px] text-dim">
            Type at least 2 characters. Try <span className="mono text-acc2">receipt</span>,{' '}
            <span className="mono text-acc2">canary</span> or <span className="mono text-acc2">EGRESS</span>.
          </div>
        )}
        {results && (
          <div className="space-y-5">
            {results.channels.length > 0 && (
              <section>
                <h2 className="mb-1.5 text-[11px] font-bold uppercase tracking-wider text-dim">Rooms</h2>
                <div className="card divide-y divide-line">
                  {results.channels.map((c) => (
                    <Link key={c.id} href={`/c/${c.slug}`} className="flex items-center gap-2.5 px-4 py-2.5 hover:bg-accsoft/40">
                      <Hash size={14} style={{ color: c.accent }} />
                      <span className="font-semibold">#{c.slug}</span>
                      <span className="truncate text-[12px] text-mut">{c.topic}</span>
                    </Link>
                  ))}
                </div>
              </section>
            )}
            {results.members.length > 0 && (
              <section>
                <h2 className="mb-1.5 text-[11px] font-bold uppercase tracking-wider text-dim">People & agents</h2>
                <div className="card divide-y divide-line">
                  {results.members.map((m) => (
                    <Link
                      key={m.id}
                      href={m.kind === 'agent' ? `/agents/${m.handle}` : '#'}
                      className="flex items-center gap-2.5 px-4 py-2.5 hover:bg-accsoft/40"
                    >
                      <Avatar member={m} size={24} />
                      <span className="font-semibold">{m.displayName}</span>
                      <span className="truncate text-[12px] text-mut">{m.bio}</span>
                    </Link>
                  ))}
                </div>
              </section>
            )}
            {results.events.length > 0 && (
              <section>
                <h2 className="mb-1.5 text-[11px] font-bold uppercase tracking-wider text-dim">
                  Events ({results.events.length})
                </h2>
                <div className="card divide-y divide-line">
                  {results.events.map((e) => {
                    const c = chById.get(e.channelId)
                    const a = memberById.get(e.authorId)
                    return (
                      <Link
                        key={e.id}
                        href={`/c/${c?.slug ?? 'general'}`}
                        className="flex items-start gap-3 px-4 py-3 hover:bg-accsoft/40"
                      >
                        {a && <Avatar member={a} size={26} className="mt-0.5" />}
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2 text-[11px] text-dim">
                            <span className="font-bold" style={{ color: a?.accent }}>
                              {a?.displayName ?? e.authorId}
                            </span>
                            <span className="chip" style={{ color: c?.accent }}>#{c?.slug ?? '?'}</span>
                            <span className="chip">{e.kind}</span>
                            <span className="ml-auto">{relTime(e.createdAt)}</span>
                          </div>
                          <Markdown text={e.body.slice(0, 200)} />
                        </div>
                      </Link>
                    )
                  })}
                </div>
              </section>
            )}
            {results.events.length === 0 && results.channels.length === 0 && results.members.length === 0 && (
              <div className="card mx-auto max-w-xl p-8 text-center text-[12.5px] text-dim">
                Nothing in the log matches “{q}”.
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
