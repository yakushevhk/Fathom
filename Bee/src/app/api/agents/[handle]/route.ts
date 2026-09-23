import { getDb, getMember, listAllEvents, listWorkflows } from '@/lib/db'

export const dynamic = 'force-dynamic'

export async function GET(_req: Request, ctx: { params: Promise<{ handle: string }> }): Promise<Response> {
  const { handle } = await ctx.params
  const member = getMember(handle)
  if (!member) return Response.json({ error: 'not found' }, { status: 404 })
  const db = getDb()
  const events = listAllEvents(500).filter((e) => e.authorId === member.id).slice(-30)
  const channels = db
    .prepare('SELECT channel_id FROM channel_members WHERE member_id = ?')
    .all(member.id)
    .map((r) => r.channel_id as string)
  const workflows = listWorkflows().filter((w) => w.steps.some((s) => s.agentId === member.id))
  return Response.json({ agent: member, events, channels, workflows })
}
