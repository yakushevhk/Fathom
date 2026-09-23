import { getDb, search } from '@/lib/db'
import { NextRequest } from 'next/server'

export const dynamic = 'force-dynamic'

export function GET(req: NextRequest): Response {
  getDb()
  const q = (new URL(req.url).searchParams.get('q') ?? '').trim()
  if (!q) return Response.json({ events: [], channels: [], members: [] })
  return Response.json(search(q))
}
