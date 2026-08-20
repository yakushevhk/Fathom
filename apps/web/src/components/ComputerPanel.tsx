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
  const [error, setError] = useState<string | null>(null)
  const [offline, setOffline] = useState(false)

  const refresh = useCallback(async () => {
    try {
      const [nextSnapshot, nextScreenshot] = await Promise.all([api.computers.snapshot(), api.computers.screenshot()])
      setSnapshot(nextSnapshot)
      setScreenshot(nextScreenshot)
      setOffline(false); setError(null)
    } catch (e) {
      setOffline(true)
      setError(e instanceof Error ? e.message : 'Computer service unavailable')
    }
  }, [])

  useEffect(() => { refresh().finally(() => setLoading(false)) }, [refresh])

  const createSession = async () => {
    setWorking(true); setError(null)
    try {
      const next = await api.computers.createSession({ url: url === 'https://' ? undefined : url })
      setSession(next)
      if (next.snapshot) setSnapshot(next.snapshot)
      if (next.control?.owner === 'human') setControlled(true)
      await refresh()
    } catch (e) { setOffline(true); setError(e instanceof Error ? e.message : 'Could not start browser session') }
    finally { setWorking(false) }
  }

  const run = async (operation: () => Promise<unknown>) => {
    if (!controlled) return
    setWorking(true); setError(null)
    try { await operation(); await refresh() }
    catch (e) { setError(e instanceof Error ? e.message : 'Computer action failed') }
    finally { setWorking(false) }
  }

  const takeControl = async () => {
    setWorking(true); setError(null)
    try { const result = await api.computers.takeControl(); setControlled(result.control?.owner === 'human' || result.owner === 'human' || result.ok === true); await refresh() }
    catch (e) { setError(e instanceof Error ? e.message : 'Could not take control') }
    finally { setWorking(false) }
  }
  const releaseControl = async () => {
    setWorking(true)
    try { await api.computers.releaseControl(); setControlled(false) }
    catch (e) { setError(e instanceof Error ? e.message : 'Could not release control') }
    finally { setWorking(false) }
  }

  const refs = snapshot?.elements ?? (snapshot?.refs ? Object.entries(snapshot.refs).map(([ref, element]) => ({ ref, ...element })) : [])
  const image = screenshot || imageSource(snapshot?.screenshot)

  return (
    <div className="flex-1 overflow-y-auto bg-[#080909]">
      <header className="border-b border-white/[0.08] px-5 py-5 md:px-8"><p className="ops-kicker">Computer / live browser</p><div className="mt-1 flex items-center justify-between gap-3"><h1 className="text-xl tracking-tight text-gray-100">Remote workspace</h1><span className={`ops-status ${offline ? 'ops-status-deny' : 'ops-status-allow'}`}>{offline ? 'OFFLINE' : loading ? 'CONNECTING' : 'READY'}</span></div><p className="mt-1 max-w-xl text-xs text-gray-500">A governed browser surface. Take control before issuing actions; screenshots are read-only until then.</p></header>
      <div className="grid gap-5 p-5 md:p-8 xl:grid-cols-[minmax(0,1fr)_300px]">
        <section className="ops-panel min-h-[500px]">
          <div className="ops-panel-head"><div><p className="ops-kicker">{session ? 'Active browser session' : 'No active session'}</p><h2>Viewport</h2></div><button onClick={() => refresh()} disabled={working} className="ops-button-secondary">↻ Snapshot</button></div>
          <div className="mb-4 flex gap-2"><input className="ops-input flex-1 font-mono" value={url} onChange={e => setUrl(e.target.value)} placeholder="https://example.com" /><button onClick={() => run(() => api.computers.navigate(url))} disabled={!controlled || working || !session} className="ops-button-secondary">Navigate</button></div>
          <div className="flex min-h-[360px] items-center justify-center overflow-hidden border border-white/[0.08] bg-[#050606]">
            {image ? <img src={image} alt={snapshot?.title ? `Browser screenshot: ${snapshot.title}` : 'Browser screenshot'} className="max-h-[600px] w-full object-contain" /> : <div className="px-6 text-center text-xs text-gray-600">{offline ? 'Computer service unavailable. Start the service or check the configured server URL.' : session ? 'No screenshot available yet.' : 'Start a session to open the live browser.'}</div>}
          </div>
          {snapshot?.url && <p className="mt-2 truncate font-mono text-[10px] text-gray-600">{snapshot.url}{snapshot.title ? ` · ${snapshot.title}` : ''}</p>}
        </section>
        <aside className="space-y-5">
          <section className="ops-panel"><div className="ops-panel-head"><div><p className="ops-kicker">Operator gate</p><h2>Control lease</h2></div><span className={`h-2 w-2 rounded-full ${controlled ? 'bg-amber-300 animate-pulse' : 'bg-gray-600'}`} /></div><p className="mb-4 text-xs text-gray-500">{controlled ? 'You have the lease. Actions are enabled.' : 'Read-only mode. Taking control is audited.'}</p>{!session && <button onClick={createSession} disabled={working} className="ops-button-primary w-full">{working ? 'Starting…' : 'Start browser session'}</button>}{session && !controlled && <button onClick={takeControl} disabled={working} className="ops-button-primary w-full">{working ? 'Requesting…' : 'Take control'}</button>}{controlled && <button onClick={releaseControl} disabled={working} className="ops-button-secondary w-full">Release control</button>}</section>
          {error && <div role="alert" className="ops-alert"><span>SERVICE NOTICE</span><p>{error}</p></div>}
          <section className="ops-panel"><div className="ops-panel-head"><div><p className="ops-kicker">Snapshot refs</p><h2>Interactable elements</h2></div><span className="font-mono text-[10px] text-gray-600">{refs.length}</span></div>{refs.length === 0 ? <div className="ops-empty">No interactable refs in this snapshot.</div> : <div className="max-h-72 space-y-1 overflow-y-auto">{refs.map((element, index) => <div key={`${element.ref}-${index}`} className="flex items-center gap-2 border-b border-white/[0.04] py-2 text-xs"><code className="text-amber-300/80">{element.ref}</code><span className="min-w-0 flex-1 truncate text-gray-500">{element.name || element.text || element.role || 'element'}</span><button onClick={() => run(() => api.computers.click(element.ref))} disabled={!controlled || working} className="text-[10px] text-gray-600 hover:text-gray-200 disabled:opacity-30">CLICK</button></div>)}</div>}</section>
          <div className="grid grid-cols-2 gap-2"><button onClick={() => run(() => api.computers.key('ENTER'))} disabled={!controlled || working} className="ops-button-secondary">Enter</button><button onClick={() => run(() => api.computers.key('ESC'))} disabled={!controlled || working} className="ops-button-secondary">Escape</button></div>
        </aside>
      </div>
    </div>
  )
}
