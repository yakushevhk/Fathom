import { decideApproval, getApproval, ME_ID } from '@/lib/db'
import { NextRequest } from 'next/server'

export const dynamic = 'force-dynamic'

export async function GET(_req: Request, ctx: { params: Promise<{ id: string }> }): Promise<Response> {
  const { id } = await ctx.params
  const ap = getApproval(id)
  if (!ap) return Response.json({ error: 'not found' }, { status: 404 })
  return Response.json({ approval: ap })
}

export async function POST(req: NextRequest, ctx: { params: Promise<{ id: string }> }): Promise<Response> {
  const { id } = await ctx.params
  const { decision } = (await req.json()) as { decision?: string }
  if (decision !== 'approved' && decision !== 'rejected') {
    return Response.json({ error: 'decision must be approved|rejected' }, { status: 400 })
  }
  const ap = decideApproval(id, decision, ME_ID)
  if (!ap) return Response.json({ error: 'not found' }, { status: 404 })
  return Response.json({ approval: ap })
}
