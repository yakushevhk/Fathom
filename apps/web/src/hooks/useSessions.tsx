'use client'

import { createContext, useContext, useState, useEffect, type ReactNode } from 'react'
import { api, type Session } from '@/lib/api'

interface SessionsContextValue {
  sessions: Session[]
  loading: boolean
  error: string | null
  refresh: () => void
  createSession: (query: string) => Promise<string | null>
  cancelSession: (id: string) => Promise<void>
}

const SessionsContext = createContext<SessionsContextValue | null>(null)

export function SessionsProvider({ children }: { children: ReactNode }) {
  const [sessions, setSessions] = useState<Session[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const load = async () => {
    try {
      const resp = await api.sessions.list()
      setSessions(resp.sessions)
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { load() }, [])

  // Poll every 5s
  useEffect(() => {
    const interval = setInterval(load, 5000)
    return () => clearInterval(interval)
  }, [])

  const createSession = async (query: string): Promise<string | null> => {
    try {
      const s = await api.sessions.create(query)
      await load()
      return s.id
    } catch (e) {
      setError(String(e))
      return null
    }
  }

  const cancelSession = async (id: string) => {
    try {
      await api.sessions.cancel(id)
      await load()
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <SessionsContext.Provider value={{ sessions, loading, error, refresh: load, createSession, cancelSession }}>
      {children}
    </SessionsContext.Provider>
  )
}

export function useSessions() {
  const ctx = useContext(SessionsContext)
  if (!ctx) throw new Error('useSessions must be inside SessionsProvider')
  return ctx
}