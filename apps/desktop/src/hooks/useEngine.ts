import { useEffect, useState } from 'react'
import { api, listenToDaemonStatus, type DaemonStatus } from '../lib/api'

const STOPPED: DaemonStatus = {
  running: false,
  url: null,
  port: null,
  binary: null,
  version: null,
  phase: 'stopped',
  error: null,
}

/**
 * Engine lifecycle hook: reads status, can start/stop the fathom engine,
 * and subscribes to backend-pushed status changes.
 */
export function useEngine() {
  const [status, setStatus] = useState<DaemonStatus>(STOPPED)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let alive = true
    api.daemon.status().then(s => {
      if (alive) setStatus(s)
    }).catch(e => {
      if (alive) setError(String(e))
    })

    const stop = listenToDaemonStatus(s => {
      if (alive) setStatus(s)
    })
    return () => {
      alive = false
      stop()
    }
  }, [])

  const start = async (port?: number, force = false) => {
    setLoading(true)
    setError(null)
    try {
      const s = await api.daemon.start(port, force)
      setStatus(s)
      return s
    } catch (e) {
      setError(String(e))
      return null
    } finally {
      setLoading(false)
    }
  }

  const stop = async () => {
    setLoading(true)
    setError(null)
    try {
      await api.daemon.stop()
      setStatus(STOPPED)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  return { status, loading, error, start, stop }
}