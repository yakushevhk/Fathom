'use client'

import Link from 'next/link'
import { usePathname, useRouter } from 'next/navigation'
import { useEffect, useMemo, useState, useSyncExternalStore } from 'react'
import {
  Activity,
  Bot,
  Cog,
  GitPullRequest,
  Hash,
  Inbox,
  Megaphone,
  Moon,
  Plus,
  Search,
  Sun,
  Waves,
  X,
} from 'lucide-react'
import { cn } from '@/lib/cn'
import { useHive } from '@/lib/store'
import { Avatar } from './Avatar'
import { Palette } from './Palette'

function useTheme(): ['dark' | 'light', (t: 'dark' | 'light') => void] {
  const theme = useSyncExternalStore<'dark' | 'light'>(
    () => () => {},
    () => (document.documentElement.dataset.theme as 'dark' | 'light') || 'dark',
    () => 'dark'
  )
  const set = (t: 'dark' | 'light') => {
    document.documentElement.dataset.theme = t
    try {
      localStorage.setItem('bee-theme', t)
    } catch {}
  }
  return [theme, set]
}

function ThemeToggle() {
  const [theme, setTheme] = useTheme()
  const toggle = () => setTheme(theme === 'dark' ? 'light' : 'dark')
  return (
    <button className="navlink w-full" onClick={toggle} title="Toggle theme">
      {theme === 'dark' ? <Moon size={15} /> : <Sun size={15} />}
      {theme === 'dark' ? 'Abyss' : 'Reef'}
    </button>
  )
}

function NewChannelDialog({ onClose }: { onClose: () => void }) {
  const { members, refreshChannels } = useHive()
  const router = useRouter()
  const agents = members.filter((m) => m.kind === 'agent')
  const [name, setName] = useState('')
  const [topic, setTopic] = useState('')
  const [picked, setPicked] = useState<Set<string>>(new Set(['a_atlas']))
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  const submit = async () => {
    setBusy(true)
    setError('')
    const r = await fetch('/api/channels', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, topic, agentIds: [...picked] }),
    })
    const data = await r.json()
    setBusy(false)
    if (!r.ok) {
      setError(data.error ?? 'Failed')
      return
    }
    refreshChannels()
    onClose()
    router.push(`/c/${data.channel.slug}`)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4" onClick={onClose}>
      <div className="card w-full max-w-md p-5" onClick={(e) => e.stopPropagation()}>
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-[15px] font-semibold">Open a room</h2>
          <button onClick={onClose} className="text-mut hover:text-fg">
            <X size={16} />
          </button>
        </div>
        <label className="mb-1 block text-[11px] font-semibold text-mut">Name</label>
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. branch-feat-login"
          className="mb-3 w-full rounded-lg border border-line2 bg-bg2 px-3 py-2 text-[13px] outline-none focus:border-acc"
        />
        <label className="mb-1 block text-[11px] font-semibold text-mut">Topic</label>
        <input
          value={topic}
          onChange={(e) => setTopic(e.target.value)}
          placeholder="What happens in this room"
          className="mb-3 w-full rounded-lg border border-line2 bg-bg2 px-3 py-2 text-[13px] outline-none focus:border-acc"
        />
        <label className="mb-2 block text-[11px] font-semibold text-mut">Agents in the room</label>
        <div className="mb-4 flex flex-wrap gap-2">
          {agents.map((a) => (
            <button
              key={a.id}
              type="button"
              onClick={() =>
                setPicked((s) => (s.has(a.id) ? new Set([...s].filter((x) => x !== a.id)) : new Set([...s, a.id])))
              }
              className={cn('chip', picked.has(a.id) && 'border-acc text-acc')}
              style={picked.has(a.id) ? { background: 'var(--acc-soft)' } : undefined}
            >
              <span className="hex inline-block h-2.5 w-2.5" style={{ background: a.accent }} />
              {a.displayName}
            </button>
          ))}
        </div>
        {error && <p className="mb-3 text-[12px] text-danger">{error}</p>}
        <button className="btn btn-primary w-full" disabled={busy || !name.trim()} onClick={submit}>
          {busy ? 'Opening…' : 'Open room'}
        </button>
      </div>
    </div>
  )
}

export function AppShell({ children }: { children: React.ReactNode }) {
  const { me, community, members, channels, ready } = useHive()
  const pathname = usePathname()
  const [paletteOpen, setPaletteOpen] = useState(false)
  const [newChannel, setNewChannel] = useState(false)
  const [pendingCount, setPendingCount] = useState(0)

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setPaletteOpen((v) => !v)
      }
    }
    window.addEventListener('keydown', h)
    return () => window.removeEventListener('keydown', h)
  }, [])

  useEffect(() => {
    fetch('/api/approvals')
      .then((r) => r.json())
      .then((d) => setPendingCount(d.approvals.filter((a: { status: string }) => a.status === 'pending').length))
      .catch(() => {})
  }, [])

  const rooms = channels.filter((c) => c.kind === 'channel' || c.kind === 'announcement')
  const dms = channels.filter((c) => c.kind === 'dm')
  const online = useMemo(() => members.filter((m) => m.presence === 'online').length, [members])
  const memberById = useMemo(() => new Map(members.map((m) => [m.id, m])), [members])

  const nav = [
    { href: '/pulse', label: 'Pulse', icon: Activity },
    { href: '/agents', label: 'Agents', icon: Bot },
    { href: '/workflows', label: 'Workflows', icon: GitPullRequest },
    { href: '/review', label: 'Review', icon: Inbox, badge: pendingCount },
    { href: '/search', label: 'Search', icon: Search },
  ]

  return (
    <div className="flex h-screen overflow-hidden bg-bg">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <aside className="flex w-[248px] shrink-0 flex-col border-r border-line bg-bg2">
        <div className="flex items-center gap-2.5 border-b border-line px-3 py-3">
          <span className="hex flex h-8 w-8 items-center justify-center bg-gradient-to-br from-acc to-acc2 text-[13px] font-black text-[#04231f]">
            F
          </span>
          <div className="min-w-0">
            <div className="truncate text-[13.5px] font-bold leading-tight">Fathom</div>
            <div className="mono truncate text-[10px] text-dim">{community?.domain ?? 'loading…'}</div>
          </div>
          <span className="ml-auto flex items-center gap-1 text-[10.5px] text-mut">
            <span className="pulse-dot h-1.5 w-1.5 rounded-full bg-ok" />
            {online} online
          </span>
        </div>

        <button
          onClick={() => setPaletteOpen(true)}
          className="mx-2 mt-2 flex items-center gap-2 rounded-lg border border-line bg-panel px-2.5 py-1.5 text-[12px] text-dim hover:border-line2 hover:text-mut"
        >
          <Search size={13} />
          Search the log
          <span className="mono ml-auto rounded border border-line px-1 text-[9px]">⌘K</span>
        </button>

        <div className="mt-2 px-2">
          {nav.map((n) => (
            <Link key={n.href} href={n.href} className={cn('navlink', pathname.startsWith(n.href) && 'active')}>
              <n.icon size={15} />
              {n.label}
              {n.badge ? (
                <span className="ml-auto rounded-full bg-danger px-1.5 text-[10px] font-bold text-white">{n.badge}</span>
              ) : null}
            </Link>
          ))}
        </div>

        <div className="mt-3 flex-1 overflow-y-auto px-2 pb-2">
          <div className="mb-1 flex items-center justify-between px-2 text-[10px] font-bold uppercase tracking-wider text-dim">
            Rooms
            <button className="text-dim hover:text-acc" onClick={() => setNewChannel(true)} title="Open a room">
              <Plus size={13} />
            </button>
          </div>
          {rooms.map((c) => (
            <Link
              key={c.id}
              href={`/c/${c.slug}`}
              className={cn('chan', pathname === `/c/${c.slug}` && 'active', c.unread > 0 && 'unread')}
            >
              {c.kind === 'announcement' ? (
                <Megaphone size={13} className="text-gold" />
              ) : (
                <Hash size={13} style={{ color: c.accent }} />
              )}
              <span className="min-w-0 flex-1 truncate">{c.slug}</span>
              {c.unread > 0 && (
                <span className="rounded-full bg-acc px-1.5 text-[9.5px] font-bold text-[#04231f]">{c.unread}</span>
              )}
            </Link>
          ))}

          <div className="mb-1 mt-4 px-2 text-[10px] font-bold uppercase tracking-wider text-dim">Direct</div>
          {dms.map((c) => {
            const peer = c.peerId ? memberById.get(c.peerId) : undefined
            return (
              <Link
                key={c.id}
                href={`/c/${c.slug}`}
                className={cn('chan', pathname === `/c/${c.slug}` && 'active', c.unread > 0 && 'unread')}
              >
                {peer ? <Avatar member={peer} size={18} /> : null}
                <span className="min-w-0 flex-1 truncate">{c.name}</span>
                {c.unread > 0 && (
                  <span className="rounded-full bg-acc px-1.5 text-[9.5px] font-bold text-[#04231f]">{c.unread}</span>
                )}
              </Link>
            )
          })}
          {!ready && <div className="px-2 py-3 text-[11px] text-dim">Connecting to relay…</div>}
        </div>

        <div className="border-t border-line px-2 py-2">
          <ThemeToggle />
          <Link href="/settings" className={cn('navlink', pathname === '/settings' && 'active')}>
            <Cog size={15} />
            Settings
          </Link>
          {me && (
            <div className="mt-1 flex items-center gap-2 rounded-lg px-2 py-1.5">
              <Avatar member={me} size={26} />
              <div className="min-w-0">
                <div className="truncate text-[12px] font-semibold leading-tight">{me.displayName}</div>
                <div className="mono truncate text-[9.5px] text-dim">{me.signature.slice(0, 14)}…</div>
              </div>
              <Waves size={13} className="ml-auto text-acc2" />
            </div>
          )}
        </div>
      </aside>

      <main id="main-content" tabIndex={-1} className="dotgrid flex min-w-0 flex-1 flex-col overflow-hidden bg-bg">
        {children}
      </main>

      {paletteOpen && <Palette onClose={() => setPaletteOpen(false)} />}
      {newChannel && <NewChannelDialog onClose={() => setNewChannel(false)} />}
    </div>
  )
}
