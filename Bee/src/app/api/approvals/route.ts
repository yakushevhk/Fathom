import { getDb, listApprovals } from '@/lib/db'

export const dynamic = 'force-dynamic'

export function GET(): Response {
  getDb()
  return Response.json({ approvals: listApprovals() })
}
