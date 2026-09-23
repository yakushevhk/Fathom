'use client'

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Hash, Megaphone, PanelRightClose, PanelRightOpen, Users } from 'lucide-react'
import { dayLabel } from '@/lib/format'
import { useHive } from '@/lib/store'
import type { Channel, HiveEvent, Member } from '@/lib/types'
import { Avatar } from './Avatar'
import { Composer } from './Composer'
import { MessageItem } from './MessageItem'

function RightPanel({ channel, members, onClose }: { channel: Channel; members: Member[]; onClose: () => void }) {
  const humans = members.filter((m) => m.kind === 'human')
  const agents = members.filter((m) => m.kind === 'agent')
  return (
    <aside className="w-[264px] shrink-0 overflow-y-auto border-l border-line bg-bg2 p-4">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-[11px] font-bold uppercase tracking-wider text-dim">Room</h3>
        <button onClick={onClose} className="text-dim hover:text-fg">
          <PanelRightClose size={15} />
        </button>
      </div>
      <div className="card mb-4 p-3">
        <div className="mb-1 flex items-center gap-2 font-semibold" style={{ color: channel.accent }}>
          <Hash size={14} /> {channel.slug}
        </div>
        <p className="text-[12px] leading-relaxed text-mut">{channel.topic || 'No topic set.'}</p>
      </div>
      <div className="mb-1 flex items-center gap-1.5 text-[11px] font-bold uppercase tracking-wider text-dim">
        <Users size={11} /> Members — {members.length}
      </div>
      {humans.length > 0 && (
        <div className="mb-2">
          <div className="px-1 py-1 text-[10px] font-semibold text-dim">Members</div>
          {humans.map((m) => (
            <div key={m.id} className="flex items-center gap-2 rounded-lg px-1 py-1.5 hover:bg-accsoft/50">
              <Avatar member={m} size={24} />
              <div className="min-w-0">
                <div className="truncate text-[12px] font-medium">{m.displayName}</div>
                <div className="truncate text-[10px] text-dim">{m.title}</div>
              </div>
              <span className="ml-auto text-[9.5px] capitalize text-dim">{m.presence}</span>
            </div>
          ))}
        </div>
      )}
      {agents.length > 0 && (
        <div className="mb-3">
          <div className="px-1 py-1 text-[10px] font-semibold text-dim">Agents</div>
          {agents.map((m) => (
            <div key={m.id} className="flex items-center gap-2 rounded-lg px-1 py-1.5 hover:bg-accsoft/50">
              <Avatar member={m} size={24} />
              <div className="min-w-0">
                <div className="truncate text-[12px] font-medium">{m.displayName}</div>
                <div className="truncate text-[10px] text-dim">{m.title}</div>
              </div>
              <span className="ml-auto text-[9.5px] capitalize text-dim">{m.presence}</span>
            </div>
          ))}
        </div>
      )}
      <div className="card p-3 text-[10.5px] leading-relaxed text-dim">
        <span className="font-semibold text-mut">Same keys, same log.</span> Every event in this room is signed by its
        author — human or agent — and lands in one index.
      </div>
    </aside>
  )
}

export function ChannelRoom({ slug }: { slug: string }) {
  const { members: allMembers, typing, lastEvent, markRead } = useHive()
  const [channel, setChannel] = useState<Channel | null>(null)
  const [members, setMembers] = useState<Member[]>([])
  const [events, setEvents] = useState<HiveEvent[]>([])
  const [panel, setPanel] = useState(true)
  const [missing, setMissing] = useState(false)
  const bottomRef = useRef<HTMLDivElement>(null)

  const load = useCallback(() => {
    fetch(`/api/channels/${slug}/events`, { cache: 'no-store' })
      .then((r) => (r.ok ? r.json() : null))
      .then((d) => {
        if (!d) {
          setMissing(true)
          return
        }
        setChannel(d.channel)
        setMembers(d.members)
        setEvents(d.events)
        const last = d.events[d.events.length - 1]
        if (last) markRead(d.channel.id, last.id)
      })
      .catch(() => setMissing(true))
  }, [slug, markRead])

  // Render-phase reset when navigating between rooms.
  const [prevSlug, setPrevSlug] = useState(slug)
  if (prevSlug !== slug) {
    setPrevSlug(slug)
    setChannel(null)
    setMissing(false)
    setEvents([])
  }

  useEffect(() => {
    load()
  }, [load])

  // Mark the room read when a new event lands while we're viewing it.
  useEffect(() => {
    if (channel && lastEvent && lastEvent.channelId === channel.id) {
      markRead(channel.id, lastEvent.id)
    }
  }, [lastEvent, channel, markRead])

  // Live events merge into the feed at render time.
  const shownEvents = (() => {
    if (!channel || !lastEvent || lastEvent.channelId !== channel.id) return events
    const i = events.findIndex((e) => e.id === lastEvent.id)
    if (i === -1) {
      return lastEvent.id > (events[events.length - 1]?.id ?? 0) ? [...events, lastEvent] : events
    }
    const copy = [...events]
    copy[i] = lastEvent
    return copy
  })()

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
  }, [shownEvents.length])

  const memberById = useMemo(() => {
    const m = new Map<string, Member>(members.map((x) => [x.id, x]))
    for (const x of allMembers) if (!m.has(x.id)) m.set(x.id, x)
    return m
  }, [members, allMembers])

  const typers = (typing[channel?.id ?? ''] ?? [])
    .map((t) => memberById.get(t.memberId))
    .filter(Boolean) as Member[]

  if (missing) {
    return (
      <div className="flex flex-1 items-center justify-center text-[13px] text-dim">
        Room <span className="mono mx-1">#{slug}</span> doesn’t exist in this community.
      </div>
    )
  }
  if (!channel) {
    return <div className="flex flex-1 items-center justify-center text-[12px] text-dim">Tuning into #{slug}…</div>
  }

  const peer = channel.peerId ? memberById.get(channel.peerId) : undefined
  const rows: { day: string | null; event: HiveEvent; dense: boolean }[] = []
  let lastDay = ''
  let lastAuthor = ''
  for (const e of shownEvents) {
    const day = dayLabel(e.createdAt)
    const showDay = day !== lastDay
    lastDay = day
    const dense = !showDay && e.authorId === lastAuthor && e.kind === 'message'
    lastAuthor = e.kind === 'message' ? e.authorId : ''
    rows.push({ day: showDay ? day : null, event: e, dense })
  }

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center gap-2.5 border-b border-line bg-panel/60 px-4 py-2.5">
          {channel.kind === 'announcement' ? (
            <Megaphone size={16} className="text-gold" />
          ) : channel.kind === 'dm' ? (
            peer ? (
              <Avatar member={peer} size={22} />
            ) : (
              <Hash size={16} />
            )
          ) : (
            <Hash size={16} style={{ color: channel.accent }} />
          )}
          <div className="min-w-0">
            <div className="truncate text-[13.5px] font-bold">
              {channel.kind === 'dm' ? channel.name : `#${channel.slug}`}
            </div>
            {channel.topic && <div className="truncate text-[10.5px] text-mut">{channel.topic}</div>}
          </div>
          <div className="ml-auto flex items-center gap-1">
            <span className="chip text-dim">
              <Users size={10} /> {channel.memberCount}
            </span>
            <button onClick={() => setPanel((v) => !v)} className="btn !p-1.5" title="Toggle panel">
              {panel ? <PanelRightClose size={14} /> : <PanelRightOpen size={14} />}
            </button>
          </div>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto py-2">
          {rows.map(({ day, event, dense }) => (
            <div key={event.id} className="msg-in">
              {day && (
                <div className="my-2 flex items-center gap-3 px-4">
                  <span className="h-px flex-1 bg-line" />
                  <span className="text-[10px] font-bold uppercase tracking-wider text-dim">{day}</span>
                  <span className="h-px flex-1 bg-line" />
                </div>
              )}
              <MessageItem event={event} member={memberById.get(event.authorId)} dense={dense} />
            </div>
          ))}
          {typers.length > 0 && (
            <div className="flex items-center gap-2 px-4 py-1.5 text-[11px] text-mut">
              <span className="typing-dots">
                <span /> <span /> <span />
              </span>
              {typers.map((t) => t.displayName).join(', ')} {typers.length === 1 ? 'is' : 'are'} thinking…
            </div>
          )}
          <div ref={bottomRef} />
        </div>

        <Composer channel={channel} onSent={() => {}} />
      </div>
      {panel && <RightPanel channel={channel} members={members} onClose={() => setPanel(false)} />}
    </div>
  )
}
