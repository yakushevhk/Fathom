// In-process pub/sub for SSE fanout. One relay, one log — every mutation
// publishes onto the bus and every connected client hears it.

export type BusMessage =
  | { type: 'event'; data: unknown }
  | { type: 'typing'; data: { channelId: string; memberId: string; until: number } }
  | { type: 'workflow'; data: unknown }
  | { type: 'approval'; data: unknown }
  | { type: 'presence'; data: { memberId: string; presence: string } }

type Subscriber = (msg: BusMessage) => void

const g = globalThis as unknown as { __beeBus?: Set<Subscriber> }

function subscribers(): Set<Subscriber> {
  if (!g.__beeBus) g.__beeBus = new Set()
  return g.__beeBus
}

export function publish(msg: BusMessage) {
  for (const sub of subscribers()) {
    try {
      sub(msg)
    } catch {
      subscribers().delete(sub)
    }
  }
}

export function subscribe(sub: Subscriber): () => void {
  subscribers().add(sub)
  return () => subscribers().delete(sub)
}
