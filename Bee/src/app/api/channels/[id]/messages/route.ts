import { getChannel, insertEvent, ME_ID } from '@/lib/db'
import { maybeTriggerAgents } from '@/lib/agents'
import { NextRequest } from 'next/server'

export const dynamic = 'force-dynamic'

export async function POST(req: NextRequest, ctx: { params: Promise<{ id: string }> }): Promise<Response> {
  const { id } = await ctx.params
  const channel = getChannel(id)
  if (!channel) return Response.json({ error: 'not found' }, { status: 404 })
  const { body, parentId } = (await req.json()) as { body?: string; parentId?: number | null }
  const text = (body ?? '').trim()
  if (!text) return Response.json({ error: 'empty message' }, { status: 400 })
  if (text.length > 8000) return Response.json({ error: 'message too long' }, { status: 413 })
  if (channel.kind === 'announcement') {
    return Response.json({ error: 'Announcements are coordinator-only.' }, { status: 403 })
  }
  const event = insertEvent(channel.id, ME_ID, 'message', text, {}, parentId ?? null)
  maybeTriggerAgents(channel.id, ME_ID, text, event.id)
  return Response.json({ event }, { status: 201 })
}
