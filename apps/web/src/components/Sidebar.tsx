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
  const { sessions, loading, error, refresh, createSession, cancelSession } = useSessions()
  const pathname = usePathname()
  const router = useRouter()
  const [query, setQuery] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const [isMobileViewport, setIsMobileViewport] = useState(false)
  const [engineOk, setEngineOk] = useState<boolean | null>(null)
  const sidebarOpenRef = useRef(false)
  const searchRef = useRef<HTMLInputElement>(null)
  const asideRef = useRef<HTMLElement>(null)
  const toggleRef = useRef<HTMLButtonElement>(null)
  const previousBodyOverflow = useRef('')
  const previousPathname = useRef(pathname)
  type SidebarCloseReason = 'escape' | 'overlay' | 'navigation'
  const closeSidebar = (reason: SidebarCloseReason) => {
    setSidebarOpen(false)
    // Escape and the overlay return keyboard users to the control that opened the drawer.
    // Route navigation must not move focus back into the now-hidden mobile context.
    if (reason !== 'navigation') toggleRef.current?.focus()
  }

  // Keep mobile-only drawer semantics separate from the persistent desktop sidebar.
  useEffect(() => {
    const mediaQuery = window.matchMedia('(max-width: 767px)')
    const updateViewport = () => setIsMobileViewport(mediaQuery.matches)
    updateViewport()
    mediaQuery.addEventListener('change', updateViewport)
    return () => mediaQuery.removeEventListener('change', updateViewport)
  }, [])

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

  // Close the mobile navigation with Escape and keep the drawer from trapping the page.
  useEffect(() => {
    if (!sidebarOpen) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        closeSidebar('escape')
        return
      }
      if (e.key !== 'Tab') return
      const focusable = asideRef.current?.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])',
      )
      if (!focusable?.length) return
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault()
        last.focus()
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault()
        first.focus()
      }
    }
    window.addEventListener('keydown', handler)
    previousBodyOverflow.current = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    return () => {
      window.removeEventListener('keydown', handler)
      document.body.style.overflow = previousBodyOverflow.current
    }
  }, [sidebarOpen])

  // Route transitions should always dismiss the mobile drawer.
  useEffect(() => {
    sidebarOpenRef.current = sidebarOpen
  }, [sidebarOpen])

  useEffect(() => {
    if (previousPathname.current === pathname) return
    previousPathname.current = pathname
    if (!sidebarOpenRef.current) return
    const closeTimer = window.setTimeout(() => closeSidebar('navigation'), 0)
    return () => window.clearTimeout(closeTimer)
  }, [pathname])

  // Put focus into the drawer when it opens so keyboard users can navigate immediately.
  useEffect(() => {
    if (sidebarOpen) searchRef.current?.focus()
  }, [sidebarOpen])

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

  return (
    <>
      {/* Mobile toggle button */}
      <button
        ref={toggleRef}
        type="button"
        onClick={() => setSidebarOpen(o => !o)}
        className="fixed left-3 top-3 z-50 flex h-9 w-9 items-center justify-center rounded-md border border-white/[0.1] bg-[#141414] text-gray-400 shadow-lg shadow-black/20 transition-colors hover:border-white/20 hover:text-gray-100 md:hidden"
        aria-label={sidebarOpen ? 'Close primary navigation' : 'Open primary navigation'}
        aria-expanded={sidebarOpen}
        aria-controls="primary-navigation"
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
        <button
          type="button"
          className="fixed inset-0 z-30 cursor-default bg-black/60 backdrop-blur-[1px] md:hidden"
          onClick={() => closeSidebar('overlay')}
          aria-label="Close primary navigation"
        />
      )}

      <aside
        ref={asideRef}
        id="primary-navigation"
        aria-label="Primary navigation"
        aria-hidden={isMobileViewport && !sidebarOpen ? true : undefined}
        inert={isMobileViewport && !sidebarOpen ? true : undefined}
        className={`
          h-full w-[min(18rem,calc(100vw-1.25rem))] min-w-0 flex flex-col border-r border-white/[0.08] bg-[#060606]
          fixed z-40 shadow-2xl shadow-black/40 transition-transform duration-200 md:relative md:w-64 md:min-w-64 md:shadow-none
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
            role="status"
            aria-label={
              engineOk === null
                ? 'Checking worker service connection'
                : engineOk
                  ? 'Worker service online'
                  : 'Worker service offline'
            }
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
            type="button"
            onClick={refresh}
            className="rounded p-1 text-gray-500 transition-colors hover:text-gray-300"
            title="Refresh worker runtime"
            aria-label="Refresh worker runtime"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <path d="M21 12a9 9 0 1 1-3-6.7M21 4v5h-5" />
            </svg>
          </button>
        </div>

        {/* Dispatch a worker session */}
        <form onSubmit={handleCreate} className="p-3 border-b border-white/[0.06]">
          <label htmlFor="new-task" className="sr-only">Submit work to a worker</label>
          <input
            id="new-task"
            value={query}
            onChange={e => setQuery(e.target.value)}
            placeholder="Dispatch a worker session…"
            className="ops-input"
          />
        </form>

        {/* Search filter */}
        <div className="px-3 py-2 border-b border-white/[0.06]">
          <label htmlFor="session-search" className="sr-only">Search worker sessions</label>
          <input
            id="session-search"
            ref={searchRef}
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            placeholder="Search worker sessions…"
            className="ops-input"
          />
        </div>

        {/* Session list */}
        <nav aria-label="Remote worker sessions" className="flex-1 overflow-y-auto py-1">
          {error && (
            <div className="mx-3 my-2 ops-alert" role="alert">
              <span>WORKER CONNECTION ERROR</span>
              Unable to load submitted work. The worker service may be offline.
              <button type="button" onClick={refresh} className="mt-2 underline underline-offset-2 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/70">
                Retry
              </button>
            </div>
          )}
          {loading && sessions.length === 0 && !error && (
            <div className="flex items-center justify-center py-8">
              <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
            </div>
          )}
          {filtered.map(s => {
            const active = pathname === `/chat/${s.id}`
            return (
              <div key={s.id} className={`group flex items-center gap-2.5 px-3 py-2.5 transition-colors ${
                active
                  ? 'bg-white/[0.06] border-l-2 border-gray-400'
                  : 'hover:bg-white/[0.03] border-l-2 border-transparent'
              }`}>
                <Link
                  href={`/chat/${s.id}`}
                  onClick={() => closeSidebar('navigation')}
                  aria-current={active ? 'page' : undefined}
                  className="flex min-w-0 flex-1 items-center gap-2.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-white/70"
                >
                  <span
                    aria-hidden="true"
                    className={`w-2 h-2 rounded-full flex-shrink-0 ${statusColor(s.status)} ${s.active ? 'animate-pulse' : ''}`}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block text-xs text-gray-200 truncate">{s.query || 'Untitled worker session'}</span>
                    <span className="block text-[10px] text-gray-600 mt-0.5">
                      {s.id.slice(0, 8)} · {s.status}
                      {s.total_agents > 0 && ` · ${s.total_agents} workers`}
                    </span>
                  </span>
                </Link>
                {s.active && (
                  <button
                    type="button"
                    onClick={() => cancelSession(s.id)}
                    className="text-red-400 hover:text-red-300 text-xs transition-opacity focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-300"
                    aria-label={`Cancel worker session ${s.query || s.id.slice(0, 8)}`}
                    title="Cancel worker session"
                  >
                    <span aria-hidden="true">✕</span>
                  </button>
                )}
              </div>
            )
          })}
          {!loading && !error && filtered.length === 0 && (
            <div className="text-center text-gray-600 text-xs py-8">
              {searchQuery.trim() ? 'No matching worker sessions' : 'No worker sessions yet'}
            </div>
          )}
        </nav>

        {/* Footer links */}
        <nav aria-label="Control plane" className="p-3 border-t border-white/[0.06] flex flex-wrap justify-center gap-x-3 gap-y-2 text-xs text-gray-500">
          {[
            ['/', 'Overview'],
            ['/agents', 'Workers'],
            ['/jobs', 'Jobs'],
            ['/memories', 'Memory'],
            ['/events', 'Events'],
            ['/observability', 'Observability'],
            ['/governance', 'Control'],
            ['/computers', 'Computers'],
            ['/coworkers', 'Coworkers'],
          ].map(([href, label]) => {
            const active = pathname === href
            return (
              <Link key={href} href={href} aria-current={active ? 'page' : undefined} className={active ? 'text-gray-200' : 'hover:text-gray-300'} onClick={() => closeSidebar('navigation')}>
                {label}
              </Link>
            )
          })}
        </nav>
      </aside>
    </>
  )
}