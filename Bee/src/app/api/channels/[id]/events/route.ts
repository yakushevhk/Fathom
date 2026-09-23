import { channelMembers, getChannel, listEvents } from '@/lib/db'

export const dynamic = 'force-dynamic'

export async function GET(_req: Request, ctx: { params: Promise<{ id: string }> }): Promise<Response> {
  const { id } = await ctx.params
  const channel = getChannel(id)
  if (!channel) return Response.json({ error: 'not found' }, { status: 404 })
  return Response.json({ channel, members: channelMembers(channel.id), events: listEvents(channel.id) })
}
