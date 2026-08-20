'use client'

import { useState, useEffect, useRef } from 'react'
import Link from 'next/link'
import { usePathname, useRouter } from 'next/navigation'
import { useSessions } from '@/hooks/useSessions'
import { api } from '@/lib/api'

function statusColor(status: string): string {
  switch (status) {
    case 'running': return 'bg-blue-400'
    case 'completed': return 'bg-green-500'
    case 'failed': return 'bg-red-500'
    default: return 'bg-gray-600'
  }
}

export function Sidebar() {
  const { sessions, loading, refresh, createSession, cancelSession } = useSessions()
  const pathname = usePathname()
  const router = useRouter()
  const [query, setQuery] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const [engineOk, setEngineOk] = useState<boolean | null>(null)
  const searchRef = useRef<HTMLInputElement>(null)

  // Filter sessions by search query
  const filtered = searchQuery.trim()
    ? sessions.filter(s =>
        (s.query || '').toLowerCase().includes(searchQuery.toLowerCase())
      )
    : sessions

  // Hotkey '/' focuses the search input
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === '/' && !['INPUT', 'TEXTAREA'].includes((e.target as HTMLElement).tagName)) {
        e.preventDefault()
        searchRef.current?.focus()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  // Poll engine health every 5s
  useEffect(() => {
    let cancelled = false
    const poll = async () => {
      try {
        const h = await api.health()
        if (!cancelled) setEngineOk(h.status === 'ok')
      } catch {
        if (!cancelled) setEngineOk(false)
      }
    }
    poll()
    const id = setInterval(poll, 5000)
    return () => { cancelled = true; clearInterval(id) }
  }, [])

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault()
    const q = query.trim()
    if (!q || submitting) return
    setSubmitting(true)
    const id = await createSession(q)
    setSubmitting(false)
    if (id) {
      setQuery('')
      router.push(`/chat/${id}`)
    }
  }

  const closeSidebar = () => setSidebarOpen(false)

  return (
    <>
      {/* Mobile toggle button */}
      <button
        onClick={() => setSidebarOpen(o => !o)}
        className="fixed top-3 left-3 z-50 md:hidden p-2 rounded-md bg-[#141414] border border-white/[0.06] text-gray-400 hover:text-gray-200 transition-colors"
        aria-label="Toggle sidebar"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
          {sidebarOpen ? (
            <>
              <path d="M18 6 6 18" />
              <path d="M6 6 18 18" />
            </>
          ) : (
            <>
              <path d="M3 12h18" />
              <path d="M3 6h18" />
              <path d="M3 18h18" />
            </>
          )}
        </svg>
      </button>

      {/* Overlay on mobile */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 z-30 bg-black/50 md:hidden"
          onClick={closeSidebar}
        />
      )}

      <aside
        className={`
          w-64 min-w-64 h-full flex flex-col bg-[#060606] border-r border-white/[0.06]
          fixed md:relative z-40 transition-transform duration-200
          ${sidebarOpen ? 'translate-x-0' : '-translate-x-full md:translate-x-0'}
        `}
      >
        {/* Header */}
        <div className="p-3 border-b border-white/[0.06] flex items-center gap-2">
          <div className="w-4 h-4 rounded-full border border-white/60 relative">
            <div className="absolute inset-1.5 border border-white/40 rounded-full" />
            <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-1 h-3 bg-white/80" />
          </div>
          <span className="font-semibold text-sm">Fathom</span>

          {/* Engine health indicator */}
          <div
            className="ml-auto flex items-center gap-1.5"
            title={
              engineOk === null
                ? 'Checking engine…'
                : engineOk
                  ? 'Engine online'
                  : 'Engine offline'
            }
          >
            <span
              className={`w-2 h-2 rounded-full ${
                engineOk === null
                  ? 'bg-gray-600'
                  : engineOk
                    ? 'bg-green-500'
                    : 'bg-red-500'
              }`}
            />
            <span className="text-[10px] text-gray-500 hidden sm:inline">
              {engineOk === null ? '…' : engineOk ? 'OK' : 'ERR'}
            </span>
          </div>

          <button
            onClick={refresh}
            className="text-gray-500 hover:text-gray-300 transition-colors"
            title="Refresh"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <path d="M21 12a9 9 0 1 1-3-6.7M21 4v5h-5" />
            </svg>
          </button>
        </div>

        {/* New session */}
        <form onSubmit={handleCreate} className="p-3 border-b border-white/[0.06]">
          <input
            value={query}
            onChange={e => setQuery(e.target.value)}
            placeholder="New research task…"
            className="w-full p-2 rounded-md bg-[#141414] border border-white/[0.06] text-xs text-gray-200 placeholder-gray-600 outline-none focus:border-gray-500 transition-colors"
          />
        </form>

        {/* Search filter */}
        <div className="px-3 py-2 border-b border-white/[0.06]">
          <input
            ref={searchRef}
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            placeholder="Search sessions…"
            className="w-full p-1.5 rounded-md bg-[#141414] border border-white/[0.06] text-xs text-gray-200 placeholder-gray-600 outline-none focus:border-gray-500 transition-colors"
          />
        </div>

        {/* Session list */}
        <nav className="flex-1 overflow-y-auto py-1">
          {loading && sessions.length === 0 && (
            <div className="flex items-center justify-center py-8">
              <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
            </div>
          )}
          {filtered.map(s => {
            const active = pathname === `/chat/${s.id}`
            return (
              <div
                key={s.id}
                className={`group flex items-center gap-2.5 px-3 py-2.5 cursor-pointer transition-colors ${
                  active
                    ? 'bg-white/[0.06] border-l-2 border-gray-400'
                    : 'hover:bg-white/[0.03] border-l-2 border-transparent'
                }`}
                onClick={() => { router.push(`/chat/${s.id}`); closeSidebar() }}
              >
                <span
                  className={`w-2 h-2 rounded-full flex-shrink-0 ${statusColor(s.status)} ${s.active ? 'animate-pulse' : ''}`}
                />
                <div className="min-w-0 flex-1">
                  <div className="text-xs text-gray-200 truncate">{s.query || 'Untitled'}</div>
                  <div className="text-[10px] text-gray-600 mt-0.5">
                    {s.id.slice(0, 8)} · {s.status}
                    {s.total_agents > 0 && ` · ${s.total_agents} agents`}
                  </div>
                </div>
                {s.active && (
                  <button
                    onClick={e => { e.stopPropagation(); cancelSession(s.id) }}
                    className="opacity-0 group-hover:opacity-100 text-red-400 hover:text-red-300 text-xs transition-opacity"
                    title="Cancel"
                  >
                    ✕
                  </button>
                )}
              </div>
            )
          })}
          {!loading && filtered.length === 0 && (
            <div className="text-center text-gray-600 text-xs py-8">
              {searchQuery.trim() ? 'No matching sessions' : 'No sessions yet'}
            </div>
          )}
        </nav>

        {/* Footer links */}
        <div className="p-3 border-t border-white/[0.06] flex flex-wrap justify-center gap-x-3 gap-y-2 text-xs text-gray-500">
          <Link href="/" className={pathname === '/' ? 'text-gray-200' : 'hover:text-gray-300'} onClick={closeSidebar}>
            Chats
          </Link>
          <Link href="/agents" className={pathname === '/agents' ? 'text-gray-200' : 'hover:text-gray-300'} onClick={closeSidebar}>
            Agents
          </Link>
          <Link href="/jobs" className={pathname === '/jobs' ? 'text-gray-200' : 'hover:text-gray-300'} onClick={closeSidebar}>
            Jobs
          </Link>
          <Link href="/memories" className={pathname === '/memories' ? 'text-gray-200' : 'hover:text-gray-300'} onClick={closeSidebar}>
            Memory
          </Link>
          <Link href="/events" className={pathname === '/events' ? 'text-gray-200' : 'hover:text-gray-300'} onClick={closeSidebar}>
            Events
          </Link>
          <Link href="/governance" className={pathname === '/governance' ? 'text-gray-200' : 'hover:text-gray-300'} onClick={closeSidebar}>
            Govern
          </Link>
          <Link href="/computers" className={pathname === '/computers' ? 'text-gray-200' : 'hover:text-gray-300'} onClick={closeSidebar}>
            Browser
          </Link>
          <Link href="/coworkers" className={pathname === '/coworkers' ? 'text-gray-200' : 'hover:text-gray-300'} onClick={closeSidebar}>
            Coworkers
          </Link>
        </div>
      </aside>
    </>
  )
}