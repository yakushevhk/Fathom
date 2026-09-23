'use client'

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'
import type { Bootstrap, Channel, HiveEvent, Member } from './types'

interface HiveState {
  ready: boolean
  me: Member | null
  community: Bootstrap['community'] | null
  members: Member[]
  channels: Channel[]
  lastEvent: HiveEvent | null
  typing: Record<string, { memberId: string; until: number }[]>
  liveVersion: number
  refreshChannels: () => void
  markRead: (channelId: string, eventId: number) => void
}

const Ctx = createContext<HiveState | null>(null)

export function useHive(): HiveState {
  const v = useContext(Ctx)
  if (!v) throw new Error('useHive outside provider')
  return v
}

export function HiveProvider({ children }: { children: React.ReactNode }) {
  const [boot, setBoot] = useState<Bootstrap | null>(null)
  const [lastEvent, setLastEvent] = useState<HiveEvent | null>(null)
  const [typing, setTyping] = useState<HiveState['typing']>({})
  const [liveVersion, setLiveVersion] = useState(0)
  const openChannel = useRef<string | null>(null)

  const refreshChannels = useCallback(() => {
    fetch('/api/bootstrap', { cache: 'no-store' })
      .then((r) => r.json())
      .then((b: Bootstrap) => setBoot(b))
      .catch(() => {})
  }, [])

  const markRead = useCallback((channelId: string, eventId: number) => {
    setBoot((prev) =>
      prev ? { ...prev, channels: prev.channels.map((c) => (c.id === channelId ? { ...c, unread: 0 } : c)) } : prev
    )
    fetch(`/api/channels/${channelId}/read`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ eventId }),
    }).catch(() => {})
  }, [])

  useEffect(() => {
    refreshChannels()
  }, [refreshChannels])

  useEffect(() => {
    const es = new EventSource('/api/events/stream')
    es.onmessage = (m) => {
      const msg = JSON.parse(m.data)
      if (msg.type === 'event') {
        const ev = msg.data as HiveEvent
        setLastEvent(ev)
        setBoot((prev) =>
          prev
            ? {
                ...prev,
                channels: prev.channels.map((c) =>
                  c.id === ev.channelId
                    ? {
                        ...c,
                        lastEventAt: ev.createdAt,
                        unread: ev.authorId === prev.me.id || openChannel.current === c.id ? c.unread : c.unread + 1,
                      }
                    : c
                ),
              }
            : prev
        )
      } else if (msg.type === 'typing') {
        const { channelId, memberId, until } = msg.data as { channelId: string; memberId: string; until: number }
        setTyping((prev) => {
          const list = (prev[channelId] ?? []).filter((t) => t.memberId !== memberId)
          return { ...prev, [channelId]: [...list, { memberId, until }] }
        })
        setTimeout(() => {
          setTyping((prev) => ({
            ...prev,
            [channelId]: (prev[channelId] ?? []).filter((t) => t.memberId !== memberId),
          }))
        }, until - Date.now() + 500)
      } else if (msg.type === 'workflow' || msg.type === 'approval') {
        setLiveVersion((v) => v + 1)
      }
    }
    return () => es.close()
  }, [])

  // Track which room is open so its events don't count as unread.
  useEffect(() => {
    const detect = () => {
      const m = window.location.pathname.match(/^\/c\/([^/]+)/)
      if (!m) {
        openChannel.current = null
        return
      }
      openChannel.current = boot?.channels.find((c) => c.slug === m[1])?.id ?? null
    }
    detect()
    window.addEventListener('popstate', detect)
    const id = setInterval(detect, 1000)
    return () => {
      window.removeEventListener('popstate', detect)
      clearInterval(id)
    }
  }, [boot?.channels])

  const value = useMemo<HiveState>(
    () => ({
      ready: boot !== null,
      me: boot?.me ?? null,
      community: boot?.community ?? null,
      members: boot?.members ?? [],
      channels: boot?.channels ?? [],
      lastEvent,
      typing,
      liveVersion,
      refreshChannels,
      markRead,
    }),
    [boot, lastEvent, typing, liveVersion, refreshChannels, markRead]
  )

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}
