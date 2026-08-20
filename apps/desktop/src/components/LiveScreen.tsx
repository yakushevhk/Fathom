import { useCallback, useEffect, useRef, useState } from 'react'
import { api, type ComputerHealth, type ComputerSnapshot } from '../lib/api'

interface LiveScreenProps {
  /** Base URL for the computer service; relative by default for the desktop proxy. */
  baseUrl?: string
}

type SurfaceState = 'loading' | 'online' | 'offline'

export function LiveScreen({ baseUrl = '/api/v1/computers' }: LiveScreenProps) {
  const [surfaceState, setSurfaceState] = useState<SurfaceState>('loading')
  const [health, setHealth] = useState<ComputerHealth | null>(null)
  const [snapshot, setSnapshot] = useState<ComputerSnapshot | null>(null)
  const [imageUrl, setImageUrl] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState<'take' | 'release' | 'navigate' | 'secret' | null>(null)
  const [urlInput, setUrlInput] = useState('')
  const [secretRef, setSecretRef] = useState('')
  const [secretValue, setSecretValue] = useState('')
  const previousImage = useRef<string | null>(null)
  const imageUrlRef = useRef<string | null>(null)

  const refresh = useCallback(async () => {
    try {
      const [nextHealth, nextSnapshot] = await Promise.all([
        api.computer.health(baseUrl),
        api.computer.snapshot(baseUrl),
      ])
      setHealth(nextHealth)
      setSnapshot(nextSnapshot)
      setUrlInput(nextSnapshot.url ?? '')

      if (nextSnapshot.screenshot && nextSnapshot.screenshot !== previousImage.current) {
        setImageUrl(nextSnapshot.screenshot)
        previousImage.current = nextSnapshot.screenshot
      } else if (!nextSnapshot.screenshot) {
        const blob = await api.computer.screenshot(baseUrl)
        const nextImage = URL.createObjectURL(blob)
        setImageUrl(current => {
          if (current?.startsWith('blob:')) URL.revokeObjectURL(current)
          imageUrlRef.current = nextImage
          return nextImage
        })
      }
      setSurfaceState('online')
      setError(null)
    } catch (cause) {
      setSurfaceState('offline')
      setError(cause instanceof Error ? cause.message : 'Computer service unavailable')
    }
  }, [baseUrl])

  useEffect(() => {
    void refresh()
    const interval = window.setInterval(() => void refresh(), 3500)
    return () => {
      window.clearInterval(interval)
      if (imageUrlRef.current?.startsWith('blob:')) URL.revokeObjectURL(imageUrlRef.current)
    }
  }, [refresh])

  const runControl = async (kind: 'take' | 'release') => {
    setBusy(kind)
    try {
      if (kind === 'take') await api.computer.takeControl(baseUrl)
      else await api.computer.releaseControl(baseUrl)
      await refresh()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Control request failed')
    } finally {
      setBusy(null)
    }
  }

  const enterSecret = async () => {
    if (!secretRef.trim() || !secretValue) return
    setBusy('secret')
    try {
      await api.computer.action('secret', { ref: secretRef.trim(), secret: secretValue }, baseUrl)
      setSecretValue('')
      await refresh()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Secret entry failed')
    } finally {
      setBusy(null)
    }
  }

  const navigate = async () => {
    if (!urlInput.trim()) return
    setBusy('navigate')
    try {
      await api.computer.action('navigate', { url: urlInput.trim() }, baseUrl)
      await refresh()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Navigation failed')
    } finally {
      setBusy(null)
    }
  }

  const owner = snapshot?.control_owner ?? health?.control_owner
  const displayImage = imageUrl ?? snapshot?.screenshot

  return (
    <section className="control-surface live-screen" aria-label="Live computer screen">
      <div className="surface-heading">
        <div>
          <div className="eyebrow">COMPUTER / VISUAL FEED</div>
          <h2>Live screen</h2>
        </div>
        <span className={`surface-status ${surfaceState}`}>
          <span className="status-dot" />
          {surfaceState === 'loading' ? 'Connecting' : surfaceState === 'online' ? 'Online' : 'Offline'}
        </span>
      </div>

      <div className="screen-frame">
        {displayImage ? (
          <img src={displayImage} alt={snapshot?.title ? `Live screen: ${snapshot.title}` : 'Live computer screen'} />
        ) : (
          <div className="screen-empty">
            <span className="screen-grid" aria-hidden="true" />
            <strong>{surfaceState === 'offline' ? 'No signal' : 'Waiting for a frame'}</strong>
            <span>{surfaceState === 'offline' ? 'The computer service will reconnect automatically.' : 'The first screenshot is arriving.'}</span>
          </div>
        )}
        <div className="screen-corner screen-corner-tl" aria-hidden="true" />
        <div className="screen-corner screen-corner-br" aria-hidden="true" />
      </div>

      <div className="screen-meta">
        <span className="meta-label">URL</span>
        <span className="meta-value truncate" title={snapshot?.url}>{snapshot?.url || '—'}</span>
      </div>
      <div className="screen-meta">
        <span className="meta-label">CONTROL</span>
        <span className={`meta-value ${owner ? 'owner-active' : ''}`}>{owner || 'Unclaimed'}</span>
      </div>

      <div className="control-actions">
        <button className="surface-button primary" onClick={() => void runControl('take')} disabled={busy !== null || surfaceState !== 'online'}>
          {busy === 'take' ? 'Claiming…' : 'Take control'}
        </button>
        <button className="surface-button" onClick={() => void runControl('release')} disabled={busy !== null || surfaceState !== 'online' || !owner}>
          {busy === 'release' ? 'Releasing…' : 'Release'}
        </button>
        <button className="surface-button icon-button" onClick={() => void refresh()} disabled={surfaceState === 'loading'} title="Refresh live screen" aria-label="Refresh live screen">↻</button>
      </div>

      <div className="navigate-row">
        <input
          className="surface-input"
          value={urlInput}
          onChange={event => setUrlInput(event.target.value)}
          onKeyDown={event => { if (event.key === 'Enter') void navigate() }}
          placeholder="Navigate to URL…"
          aria-label="Navigate to URL"
        />
        <button className="surface-button" onClick={() => void navigate()} disabled={busy !== null || surfaceState !== 'online' || !urlInput.trim()}>
          {busy === 'navigate' ? '…' : 'Go'}
        </button>
      </div>

      <div className="navigate-row secret-entry-row">
        <input className="surface-input" value={secretRef} onChange={event => setSecretRef(event.target.value)} placeholder="Element ref (e_…)" aria-label="Secret element ref" />
        <input className="surface-input" type="password" value={secretValue} onChange={event => setSecretValue(event.target.value)} placeholder="Secret value (never logged)" aria-label="Secret value" />
        <button className="surface-button" onClick={() => void enterSecret()} disabled={busy !== null || surfaceState !== 'online' || !secretRef.trim() || !secretValue}>
          {busy === 'secret' ? 'Sending…' : 'Enter secret'}
        </button>
      </div>

      {error && (
        <div className="surface-error" role="status">
          <span>Connection issue</span>
          <button onClick={() => void refresh()}>Retry</button>
          <small>{error}</small>
        </div>
      )}
    </section>
  )
}
