import { subscribe, type BusMessage } from '@/lib/bus'
import { getDb } from '@/lib/db'
import { startTicker } from '@/lib/agents'

export const dynamic = 'force-dynamic'

// SSE stream: every mutation on the single log is pushed to every client.
export function GET(): Response {
  getDb()
  startTicker()
  let unsubscribe: () => void = () => {}
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      const enc = new TextEncoder()
      const send = (msg: BusMessage) => {
        try {
          controller.enqueue(enc.encode(`data: ${JSON.stringify(msg)}\n\n`))
        } catch {
          unsubscribe()
        }
      }
      controller.enqueue(enc.encode(': connected\n\n'))
      unsubscribe = subscribe(send)
      const hb = setInterval(() => {
        try {
          controller.enqueue(enc.encode(': ping\n\n'))
        } catch {
          clearInterval(hb)
          unsubscribe()
        }
      }, 25_000)
    },
    cancel() {
      unsubscribe()
    },
  })
  return new Response(stream, {
    headers: {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache, no-transform',
      Connection: 'keep-alive',
    },
  })
}
