import { getDb, toggleReaction, ME_ID } from '@/lib/db'
import { NextRequest } from 'next/server'

export const dynamic = 'force-dynamic'

export async function POST(req: NextRequest, ctx: { params: Promise<{ id: string }> }): Promise<Response> {
  const { id } = await ctx.params
  const { emoji } = (await req.json()) as { emoji?: string }
  if (!emoji || emoji.length > 8) return Response.json({ error: 'invalid emoji' }, { status: 400 })
  getDb()
  const event = toggleReaction(Number(id), ME_ID, emoji)
  if (!event) return Response.json({ error: 'not found' }, { status: 404 })
  return Response.json({ event })
}
