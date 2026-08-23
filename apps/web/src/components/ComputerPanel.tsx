'use client'

import { useCallback, useEffect, useState } from 'react'
import { api, type ComputerSession, type ComputerSnapshot } from '@/lib/api'

function imageSource(value: unknown): string | null {
  if (typeof value !== 'string' || !value) return null
  return value.startsWith('data:') || value.startsWith('http') ? value : `data:image/png;base64,${value}`
}

export default function ComputerPanel() {
  const [session, setSession] = useState<ComputerSession | null>(null)
  const [snapshot, setSnapshot] = useState<ComputerSnapshot | null>(null)
  const [screenshot, setScreenshot] = useState<string | null>(null)
  const [url, setUrl] = useState('https://')
  const [loading, setLoading] = useState(true)
  const [working, setWorking] = useState(false)
  const [controlled, setControlled] = useState(false)
  const [controlKnown, setControlKnown] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [offline, setOffline] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)

  const refresh = useCallback(async (sessionHint: ComputerSession | null = session) => {
    setRefreshing(true)
    try {
      // Snapshot/screenshot are optional until a browser session exists. A missing
      // session is an expected idle state, not proof that the service is offline.
      const nextSnapshot = await api.computers.snapshot().catch(error => {
        const status = error && typeof error === 'object' && 'status' in error ? error.status : null
        if (status === 400 || status === 404 || status === 409) return null
        throw error
      })
      setSnapshot(nextSnapshot)
      if (nextSnapshot?.control?.owner) {
        setControlled(nextSnapshot.control.owner === 'human')
        setControlKnown(true)
      }
      const snapshotSessionId = nextSnapshot?.tab_id ?? nextSnapshot?.session_id
      setSession(current => {
        const id = snapshotSessionId ?? sessionHint?.id ?? current?.id
        if (!id) return current
        return current?.id === id ? current : { ...(current ?? {}), id, status: current?.status ?? 'active' }
      })
      if (!snapshotSessionId && !sessionHint) {
        setScreenshot(previous => { if (previous?.startsWith('blob:')) URL.revokeObjectURL(previous); return null })
        setOffline(false); setError(null)
        return
      }
      const nextScreenshot = await api.computers.screenshot().catch(error => {
        const status = error && typeof error === 'object' && 'status' in error ? error.status : null
        if (status === 400 || status === 404 || status === 409) return null
        throw error
      })
      setScreenshot(previous => {
        if (previous?.startsWith('blob:')) URL.revokeObjectURL(previous)
        return nextScreenshot
      })
      setOffline(false); setError(null)
    } catch (e) {
      setOffline(true)
      setError(e instanceof Error ? e.message : 'Computer service unavailable')
    } finally { setRefreshing(false) }
  }, [session])

  useEffect(() => {
    void (async () => {
      await refresh()
      setLoading(false)
    })()
    return () => setScreenshot(previous => { if (previous?.startsWith('blob:')) URL.revokeObjectURL(previous); return null })
  }, [refresh])

  const sessionActive = Boolean(session?.id || snapshot?.tab_id || snapshot?.session_id)
  const sessionReady = Boolean(sessionActive && !offline)
  const botActionsReady = Boolean(sessionReady && controlKnown && !controlled)

  const createSession = async () => {
    setWorking(true); setError(null)
    try {
      const next = await api.computers.createSession({ url: url === 'https://' ? undefined : url })
      const sessionId = next.snapshot?.tab_id ?? next.tab_id ?? next.snapshot?.session_id ?? next.session_id ?? next.id
      const sessionState = sessionId ? { ...next, id: sessionId } : next
      setSession(sessionState)
      if (next.snapshot) setSnapshot(next.snapshot)
      if (next.control?.owner) {
        setControlled(next.control.owner === 'human')
        setControlKnown(true)
      } else {
        setControlKnown(false)
      }
      setNotice('Browser session ready. Read-only snapshots are available; bot actions unlock after the lease owner is confirmed.')
      await refresh(sessionState)
    } catch (e) { setOffline(false); setError(e instanceof Error ? e.message : 'Could not start browser session') }
    finally { setWorking(false) }
  }

  const run = async (operation: () => Promise<unknown>) => {
    if (!sessionReady) return
    if (controlled) {
      setNotice('Bot actions are paused while the human lease is active. Release control to resume HTTP actions.')
      return
    }
    setWorking(true); setError(null); setNotice(null)
    try { await operation(); setNotice('Bot action completed.'); await refresh() }
    catch (e) { setError(e instanceof Error ? e.message : 'Computer action failed') }
    finally { setWorking(false) }
  }

  const takeControl = async () => {
    if (!sessionReady) return
    setWorking(true); setError(null); setNotice(null)
    try {
      const result = await api.computers.takeControl()
      const ownsLease = result.control?.owner === 'human' || result.owner === 'human'
      setControlled(ownsLease)
      setControlKnown(Boolean(result.control?.owner || result.owner))
      setNotice(ownsLease ? 'Human lease active. Bot HTTP actions are paused until you release control.' : 'Control request completed without a human lease.')
      await refresh()
    }
    catch (e) { setError(e instanceof Error ? e.message : 'Could not take control') }
    finally { setWorking(false) }
  }
  const releaseControl = async () => {
    if (!sessionReady || !controlled) return
    setWorking(true); setError(null); setNotice(null)
    try {
      await api.computers.releaseControl()
      setControlled(false)
      setControlKnown(true)
      setNotice('Human lease released. Bot HTTP actions are available again.')
      await refresh()
    }
    catch (e) { setError(e instanceof Error ? e.message : 'Could not release control') }
    finally { setWorking(false) }
  }

  const refs = snapshot?.elements ?? (snapshot?.refs ? Object.entries(snapshot.refs).map(([ref, element]) => ({ ref, ...element })) : [])
  const image = screenshot || imageSource(snapshot?.screenshot)
  const statusLabel = offline ? 'OFFLINE' : loading ? 'CONNECTING' : sessionReady ? 'SESSION READY' : 'IDLE'

  return (
    <div className="flex-1 overflow-y-auto bg-[#080909]">
      <header className="border-b border-white/[0.08] px-5 py-5 md:px-8"><p className="ops-kicker">Computer / remote workspace</p><div className="mt-1 flex flex-wrap items-center justify-between gap-3"><h1 className="text-xl tracking-tight text-gray-100">Remote computer for worker operations</h1><span className={`ops-status shrink-0 ${offline ? 'ops-status-deny' : sessionActive ? 'ops-status-allow' : ''}`} aria-live="polite">{statusLabel}</span></div><p className="mt-1 max-w-xl text-xs text-gray-500">An optional governed computer surface for remote workers. Snapshots remain available whenever a session exists. Bot HTTP actions pause while the audited human lease is active.</p></header>
      <div className="grid gap-5 p-5 md:p-8 xl:grid-cols-[minmax(0,1fr)_300px]">
        <section className="ops-panel min-h-[500px]">
          <div className="ops-panel-head"><div><p className="ops-kicker">{session ? 'Active browser session' : 'No active session'}</p><h2>Viewport</h2></div><button type="button" onClick={() => void refresh()} disabled={working || refreshing} aria-label="Refresh browser snapshot" className="ops-button-secondary">{refreshing ? 'Refreshing…' : '↻ Snapshot'}</button></div>
          <form className="mb-4 flex flex-col gap-2 sm:flex-row" onSubmit={event => { event.preventDefault(); if (!botActionsReady || working) return; void run(() => api.computers.navigate(url)) }}><label className="sr-only" htmlFor="computer-url">Destination URL</label><input id="computer-url" type="text" inputMode="url" className="ops-input min-w-0 flex-1 font-mono" value={url} onChange={e => setUrl(e.target.value)} placeholder="https://example.com" /><button type="submit" disabled={!botActionsReady || working} className="ops-button-secondary">Navigate</button></form>
          <div className="flex min-h-[360px] items-center justify-center overflow-hidden border border-white/[0.08] bg-[#050606]">
            {image ? <>
              {/* Dynamic authenticated/blob screenshots cannot use Next image optimization. */}
              {/* eslint-disable-next-line @next/next/no-img-element */}
              <img src={image} alt={snapshot?.title ? `Browser screenshot: ${snapshot.title}` : 'Browser screenshot'} className="max-h-[600px] w-full object-contain" decoding="async" />
            </> : <div className="px-6 text-center text-xs text-gray-600">{offline ? 'Computer service unavailable. Start the service or check the configured server URL.' : session ? 'No screenshot available yet.' : 'Start a session to open the live browser.'}</div>}
          </div>
          {snapshot?.url && <p className="mt-2 truncate font-mono text-[10px] text-gray-600">{snapshot.url}{snapshot.title ? ` · ${snapshot.title}` : ''}</p>}
        </section>
        <aside className="space-y-5">
          <section className="ops-panel"><div className="ops-panel-head"><div><p className="ops-kicker">Operator gate</p><h2>Control lease</h2></div><span className={`h-2 w-2 rounded-full ${controlled ? 'bg-amber-300 animate-pulse' : 'bg-gray-600'}`} role="img" aria-label={controlled ? 'Control lease active' : 'Read-only control lease'} /></div><p className="mb-4 text-xs text-gray-500">{controlled ? 'Human lease active. Read-only snapshot stays available, but bot HTTP actions are paused. Release control to resume them.' : controlKnown ? 'Bot HTTP actions are enabled for this session. Take the human lease when an operator needs direct control; doing so pauses bot actions.' : 'Lease owner is not confirmed yet. Read-only snapshot remains available; bot actions stay disabled until the gate is known.'}</p>{!sessionActive && <button type="button" onClick={createSession} disabled={working} className="ops-button-primary w-full">{working ? 'Starting…' : 'Start browser session'}</button>}{sessionActive && !controlled && <button type="button" onClick={takeControl} disabled={working} className="ops-button-primary w-full">{working ? 'Requesting…' : 'Take human control'}</button>}{controlled && <button type="button" onClick={releaseControl} disabled={working} className="ops-button-secondary w-full">Release human control</button>}</section>
          {error && <div role="alert" className="ops-alert"><span>SERVICE NOTICE</span><p>{error}</p></div>}
          {notice && <p role="status" aria-live="polite" className="text-xs text-emerald-300/80">{notice}</p>}
          <section className="ops-panel"><div className="ops-panel-head"><div><p className="ops-kicker">Snapshot refs</p><h2>Interactable elements</h2></div><span className="font-mono text-[10px] text-gray-600">{refs.length}</span></div>{refs.length === 0 ? <div className="ops-empty">No interactable worker-control refs in this snapshot.</div> : <div className="max-h-72 space-y-1 overflow-y-auto">{refs.map((element, index) => <div key={`${element.ref}-${index}`} className="flex items-center gap-2 border-b border-white/[0.04] py-2 text-xs"><code className="text-amber-300/80">{element.ref}</code><span className="min-w-0 flex-1 truncate text-gray-500">{element.name || element.text || element.role || 'element'}</span><button type="button" onClick={() => void run(() => api.computers.click(element.ref))} aria-label={`Click ${element.name || element.text || element.role || element.ref}`} disabled={!botActionsReady || working} className="shrink-0 text-[10px] text-gray-600 hover:text-gray-200 disabled:opacity-30">BOT CLICK</button></div>)}</div>}</section>
          <div className="grid grid-cols-2 gap-2"><button type="button" onClick={() => void run(() => api.computers.key('ENTER'))} disabled={!botActionsReady || working} className="ops-button-secondary">Bot Enter</button><button type="button" onClick={() => void run(() => api.computers.key('ESC'))} disabled={!botActionsReady || working} className="ops-button-secondary">Bot Escape</button></div>
        </aside>
      </div>
    </div>
  )
}
