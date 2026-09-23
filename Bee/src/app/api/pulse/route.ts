import { getDb, listAllEvents } from '@/lib/db'
import { NextRequest } from 'next/server'

export const dynamic = 'force-dynamic'

export function GET(req: NextRequest): Response {
  getDb()
  const kind = new URL(req.url).searchParams.get('kind') ?? undefined
  return Response.json({ events: listAllEvents(300, kind ?? undefined) })
}
