'use client'

import { useRouter } from 'next/navigation'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Activity, Bot, GitPullRequest, Hash, Inbox, MessageSquare, Search } from 'lucide-react'
import { useHive } from '@/lib/store'
import type { HiveEvent } from '@/lib/types'
import { relTime } from '@/lib/format'

interface Item {
  id: string
  label: string
  hint?: string
  icon: React.ReactNode
  action: () => void
}

// One palette for everything — rooms, people, events, actions. The log is
// the index; this is just a fast path into it.
export function Palette({ onClose }: { onClose: () => void }) {
  const { channels, members } = useHive()
  const router = useRouter()
  const [q, setQ] = useState('')
  const [idx, setIdx] = useState(0)
  const [fetched, setFetched] = useState<{ q: string; events: HiveEvent[] } | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => inputRef.current?.focus(), [])

  useEffect(() => {
    if (q.trim().length < 2) return
    const needle = q
    const t = setTimeout(() => {
      fetch(`/api/search?q=${encodeURIComponent(needle)}`)
        .then((r) => r.json())
        .then((d) => setFetched({ q: needle, events: (d.events as HiveEvent[]).slice(0, 6) }))
        .catch(() => {})
    }, 180)
    return () => clearTimeout(t)
  }, [q])
  const events = useMemo(
    () => (q.trim().length >= 2 && fetched?.q === q ? fetched.events : []),
    [q, fetched]
  )

  const go = useCallback(
    (path: string) => {
      onClose()
      router.push(path)
    },
    [onClose, router]
  )

  const items = useMemo<Item[]>(() => {
    const needle = q.toLowerCase()
    const list: Item[] = []
    for (const c of channels) {
      if (!needle || c.slug.includes(needle) || c.name.toLowerCase().includes(needle)) {
        list.push({
          id: `c-${c.id}`,
          label: `#${c.slug}`,
          hint: c.topic,
          icon: <Hash size={14} style={{ color: c.accent }} />,
          action: () => go(`/c/${c.slug}`),
        })
      }
    }
    for (const m of members) {
      if (m.kind === 'agent' && (!needle || m.displayName.toLowerCase().includes(needle) || m.handle.includes(needle))) {
        list.push({
          id: `m-${m.id}`,
          label: m.displayName,
          hint: m.title,
          icon: <Bot size={14} style={{ color: m.accent }} />,
          action: () => go(`/agents/${m.handle}`),
        })
      }
    }
    for (const e of events) {
      const ch = channels.find((c) => c.id === e.channelId)
      list.push({
        id: `e-${e.id}`,
        label: e.body.slice(0, 64),
        hint: `#${ch?.slug ?? '?'} · ${relTime(e.createdAt)}`,
        icon: <MessageSquare size={14} className="text-mut" />,
        action: () => go(`/c/${ch?.slug ?? 'general'}`),
      })
    }
    list.push(
      { id: 'nav-pulse', label: 'Go to Pulse', icon: <Activity size={14} />, action: () => go('/pulse') },
      { id: 'nav-wf', label: 'Go to Workflows', icon: <GitPullRequest size={14} />, action: () => go('/workflows') },
      { id: 'nav-rev', label: 'Go to Review queue', icon: <Inbox size={14} />, action: () => go('/review') }
    )
    return list.slice(0, 12)
  }, [q, channels, members, events, go])



  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose()
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setIdx((i) => Math.min(i + 1, items.length - 1))
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault()
      setIdx((i) => Math.max(i - 1, 0))
    }
    if (e.key === 'Enter' && items[idx]) items[idx].action()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[14vh]" onClick={onClose}>
      <div className="card w-full max-w-lg overflow-hidden" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center gap-2 border-b border-line px-3 py-2.5">
          <Search size={15} className="text-dim" />
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => {
              setQ(e.target.value)
              setIdx(0)
            }}
            onKeyDown={onKey}
            placeholder="Search rooms, people, events…"
            className="w-full bg-transparent text-[13.5px] outline-none placeholder:text-dim"
          />
          <span className="mono rounded border border-line px-1.5 text-[9px] text-dim">ESC</span>
        </div>
        <div className="max-h-[46vh] overflow-y-auto p-1.5">
          {items.map((it, i) => (
            <button
              key={it.id}
              onClick={it.action}
              onMouseEnter={() => setIdx(i)}
              className={`flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13px] ${
                i === idx ? 'bg-accsoft text-fg' : 'text-mut'
              }`}
            >
              {it.icon}
              <span className="min-w-0 flex-1 truncate">{it.label}</span>
              {it.hint && <span className="truncate text-[11px] text-dim">{it.hint}</span>}
            </button>
          ))}
          {items.length === 0 && <div className="px-3 py-6 text-center text-[12px] text-dim">Nothing in the log matches.</div>}
        </div>
      </div>
    </div>
  )
}
