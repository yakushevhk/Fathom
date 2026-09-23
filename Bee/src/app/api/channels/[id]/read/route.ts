import { getChannel, markRead, ME_ID } from '@/lib/db'
import { NextRequest } from 'next/server'

export const dynamic = 'force-dynamic'

export async function POST(req: NextRequest, ctx: { params: Promise<{ id: string }> }): Promise<Response> {
  const { id } = await ctx.params
  const channel = getChannel(id)
  if (!channel) return Response.json({ error: 'not found' }, { status: 404 })
  const { eventId } = (await req.json().catch(() => ({}))) as { eventId?: number }
  markRead(channel.id, ME_ID, eventId ?? Number.MAX_SAFE_INTEGER)
  return Response.json({ ok: true })
}
