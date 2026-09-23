import { getWorkflow } from '@/lib/db'

export const dynamic = 'force-dynamic'

export async function GET(_req: Request, ctx: { params: Promise<{ id: string }> }): Promise<Response> {
  const { id } = await ctx.params
  const wf = getWorkflow(id)
  if (!wf) return Response.json({ error: 'not found' }, { status: 404 })
  return Response.json({ workflow: wf })
}
