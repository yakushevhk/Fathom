import { listenToDaemonStatus, connectSSE, type DaemonStatus, type AgentEvent } from '../lib/api'

/**
 * SSE stream of events from a specific session.
 * Returns a cleanup function.
 */
export function useSessionSSE(
  baseUrl: string | null | undefined,
  sessionId: string | null,
  onEvent: (event: AgentEvent) => void,
  onError?: (err: string) => void,
): () => void {
  let cleanup: (() => void) | undefined

  if (baseUrl && sessionId) {
    const ctrl = connectSSE(
      baseUrl,
      `/api/v1/sessions/${sessionId}/events`,
      data => onEvent(data as AgentEvent),
      onError,
    )
    cleanup = () => ctrl.abort()
  }

  return () => cleanup?.()
}

/**
 * SSE stream of all global events.
 * Returns a cleanup function.
 */
export function useGlobalSSE(
  baseUrl: string | null | undefined,
  onEvent: (event: AgentEvent) => void,
  onError?: (err: string) => void,
): () => void {
  let cleanup: (() => void) | undefined

  if (baseUrl) {
    const ctrl = connectSSE(
      baseUrl,
      '/api/v1/events',
      data => onEvent(data as AgentEvent),
      onError,
    )
    cleanup = () => ctrl.abort()
  }

  return () => cleanup?.()
}

export { listenToDaemonStatus, connectSSE }
export type { DaemonStatus, AgentEvent }