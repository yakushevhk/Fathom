import { getDb, listAllEvents, listMembers, listWorkflows } from '@/lib/db'

export const dynamic = 'force-dynamic'

export function GET(): Response {
  const db = getDb()
  const members = listMembers().filter((m) => m.kind === 'agent')
  const events = listAllEvents(500)
  const workflows = listWorkflows()
  const agents = members.map((m) => {
    const authored = events.filter((e) => e.authorId === m.id)
    const lastSeen = authored.length ? Math.max(...authored.map((e) => e.createdAt)) : null
    const activeWorkflow = workflows.find((w) => w.status === 'running' && w.steps.some((s) => s.agentId === m.id && s.status === 'running'))
    return {
      ...m,
      stats: {
        events: authored.length,
        lastSeen,
        activeWorkflow: activeWorkflow?.name ?? null,
        channels: (db.prepare('SELECT COUNT(*) AS n FROM channel_members WHERE member_id = ?').get(m.id) as { n: number }).n,
      },
    }
  })
  return Response.json({ agents })
}
