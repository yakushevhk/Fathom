import { useEffect, useState } from 'react'
import { api, type SessionSummary } from '../lib/api'

/**
 * Session list + live polling. Lists research sessions from the fathom
 * engine and keeps them fresh while the engine is running.
 */
export function useSessions(enabled: boolean) {
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!enabled) {
      setSessions([])
      return
    }
    let alive = true
    const refresh = async () => {
      try {
        const resp = await api.sessions.list()
        if (alive) {
          setSessions(resp.sessions)
          setError(null)
        }
      } catch (e) {
        if (alive) setError(String(e))
      } finally {
        if (alive) setLoading(false)
      }
    }
    setLoading(true)
    refresh()
    const interval = setInterval(refresh, 5000)
    return () => {
      alive = false
      clearInterval(interval)
    }
  }, [enabled])

  const create = async (query: string): Promise<string | null> => {
    try {
      const s = await api.sessions.create(query)
      return s.id
    } catch (e) {
      setError(String(e))
      return null
    }
  }

  const cancel = async (id: string) => {
    try {
      await api.sessions.cancel(id)
    } catch (e) {
      setError(String(e))
    }
  }

  return { sessions, loading, error, create, cancel }
}