import { getDb, listChannels, listMembers, ME_ID } from '@/lib/db'
import type { Bootstrap } from '@/lib/types'

export const dynamic = 'force-dynamic'

export function GET(): Response {
  const db = getDb()
  const members = listMembers()
  const me = members.find((m) => m.id === ME_ID)!
  const eventCount = (db.prepare('SELECT COUNT(*) AS n FROM events').get() as { n: number }).n
  const body: Bootstrap = {
    me,
    community: { name: 'Fathom', domain: 'fathom.local', eventCount },
    members,
    channels: listChannels(),
  }
  return Response.json(body)
}
