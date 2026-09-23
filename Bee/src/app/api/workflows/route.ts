import { getDb, listWorkflows } from '@/lib/db'

export const dynamic = 'force-dynamic'

export function GET(): Response {
  getDb()
  return Response.json({ workflows: listWorkflows() })
}
