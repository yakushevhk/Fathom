'use client'

import Link from 'next/link'
import { useEffect, useState } from 'react'
import { AlertTriangle, Check, Inbox, ShieldCheck, XCircle } from 'lucide-react'
import { cn } from '@/lib/cn'
import { relTime } from '@/lib/format'
import { useHive } from '@/lib/store'
import type { Approval, Member } from '@/lib/types'
import { Avatar } from '@/components/Avatar'

const RISK_COLOR: Record<string, string> = {
  low: 'var(--ok)',
  medium: 'var(--gold)',
  high: 'var(--danger)',
}

export default function ReviewPage() {
  const { members, channels, liveVersion } = useHive()
  const [approvals, setApprovals] = useState<Approval[]>([])
  const [filter, setFilter] = useState<'pending' | 'all'>('pending')
  const [busyId, setBusyId] = useState<string | null>(null)

  const load = () => {
    fetch('/api/approvals', { cache: 'no-store' })
      .then((r) => r.json())
      .then((d) => setApprovals(d.approvals))
      .catch(() => {})
  }
  useEffect(load, [liveVersion])

  const decide = async (id: string, decision: 'approved' | 'rejected') => {
    setBusyId(id)
    await fetch(`/api/approvals/${id}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ decision }),
    }).catch(() => {})
    setBusyId(null)
    load()
  }

  const memberById = new Map<string, Member>(members.map((m) => [m.id, m]))
  const chById = new Map(channels.map((c) => [c.id, c]))
  const shown = approvals.filter((a) => filter === 'all' || a.status === 'pending')
  const pending = approvals.filter((a) => a.status === 'pending').length

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <header className="border-b border-line bg-panel/60 px-6 py-4">
        <h1 className="flex items-center gap-2 text-[17px] font-bold">
          <Inbox size={18} className="text-acc" /> Review queue
        </h1>
        <p className="mt-0.5 max-w-xl text-[12px] text-mut">
          Agents act inside their scope; anything outside it waits on a human gate. Approvals and rejections land
          in the log like everything else.
        </p>
        <div className="mt-2 flex gap-1">
          {(['pending', 'all'] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={cn('chip capitalize', filter === f && 'border-acc text-acc')}
              style={filter === f ? { background: 'var(--acc-soft)' } : undefined}
            >
              {f} {f === 'pending' && pending > 0 ? `(${pending})` : ''}
            </button>
          ))}
        </div>
      </header>
      <div className="space-y-3 p-6">
        {shown.map((a) => {
          const agent = memberById.get(a.agentId)
          const c = chById.get(a.channelId)
          return (
            <div key={a.id} className="card card-hover p-4">
              <div className="flex items-start gap-3">
                {agent && <Avatar member={agent} size={34} />}
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[13px] font-bold">{agent?.displayName ?? a.agentId}</span>
                    <span className="chip" style={{ color: RISK_COLOR[a.risk], borderColor: RISK_COLOR[a.risk] }}>
                      <AlertTriangle size={9} /> {a.risk} risk
                    </span>
                    <span className="mono chip text-dim">{a.scope}</span>
                  </div>
                  <div className="mt-1 flex items-center gap-2 text-[13px]">
                    <ShieldCheck size={14} className="shrink-0 text-gold" />
                    <span className="font-medium">{a.action}</span>
                  </div>
                  {a.detail && <p className="mt-1 text-[12px] text-mut">{a.detail}</p>}
                  <div className="mt-1.5 flex items-center gap-3 text-[10.5px] text-dim">
                    {c && (
                      <Link href={`/c/${c.slug}`} className="chip" style={{ color: c.accent }}>
                        #{c.slug}
                      </Link>
                    )}
                    <span>{relTime(a.createdAt)}</span>
                    <span className="mono">{a.id}</span>
                  </div>
                </div>
                <div className="shrink-0">
                  {a.status === 'pending' ? (
                    <div className="flex gap-2">
                      <button
                        className="btn btn-primary !py-1 !text-[11px]"
                        disabled={busyId === a.id}
                        onClick={() => decide(a.id, 'approved')}
                      >
                        <Check size={12} /> Approve
                      </button>
                      <button
                        className="btn btn-danger !py-1 !text-[11px]"
                        disabled={busyId === a.id}
                        onClick={() => decide(a.id, 'rejected')}
                      >
                        <XCircle size={12} /> Reject
                      </button>
                    </div>
                  ) : (
                    <span
                      className={cn('chip', a.status === 'approved' ? 'text-ok' : 'text-danger')}
                      style={{ borderColor: a.status === 'approved' ? 'var(--ok)' : 'var(--danger)' }}
                    >
                      {a.status} · {relTime(a.decidedAt ?? a.createdAt)}
                    </span>
                  )}
                </div>
              </div>
            </div>
          )
        })}
        {shown.length === 0 && (
          <div className="card p-10 text-center text-[12.5px] text-dim">
            Queue is clear — no agent is waiting on a human.
          </div>
        )}
      </div>
    </div>
  )
}
