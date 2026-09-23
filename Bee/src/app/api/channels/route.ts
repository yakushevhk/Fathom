import { createChannel, getDb, insertEvent, listChannels, ME_ID } from '@/lib/db'
import { NextRequest } from 'next/server'

export const dynamic = 'force-dynamic'

export function GET(): Response {
  getDb()
  return Response.json({ channels: listChannels() })
}

export async function POST(req: NextRequest): Promise<Response> {
  const { name, topic = '', agentIds = [] } = (await req.json()) as { name?: string; topic?: string; agentIds?: string[] }
  if (!name || !/^[a-z0-9][a-z0-9-_ ]{1,40}$/i.test(name)) {
    return Response.json({ error: 'Channel name must be 2–40 chars of letters, digits, - _ or spaces.' }, { status: 400 })
  }
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
  if (listChannels().some((c) => c.slug === slug)) {
    return Response.json({ error: `A room named #${slug} already exists.` }, { status: 409 })
  }
  const channel = createChannel(slug, name, topic, 'channel', [ME_ID, 'm_mira', ...agentIds])
  insertEvent(channel.id, 'a_atlas', 'system', `Room **#${slug}** opened by Hermann. ${topic || 'No topic yet.'}`, {})
  return Response.json({ channel }, { status: 201 })
}
