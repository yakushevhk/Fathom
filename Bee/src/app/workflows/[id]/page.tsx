'use client'

import Link from 'next/link'
import { useParams } from 'next/navigation'
import { useEffect, useState } from 'react'
import { CheckCircle2, ChevronRight, Circle, Loader2, XCircle } from 'lucide-react'
import { relTime } from '@/lib/format'
import type { Member, Workflow } from '@/lib/types'
import { Avatar } from '@/components/Avatar'
import { useHive } from '@/lib/store'

const STEP_ICON: Record<string, React.ReactNode> = {
  succeeded: <CheckCircle2 size={16} className="text-ok" />,
  failed: <XCircle size={16} className="text-danger" />,
  running: <Loader2 size={16} className="animate-spin text-acc2" />,
  pending: <Circle size={16} className="text-dim" />,
  skipped: <Circle size={16} className="text-dim" />,
}

export default function WorkflowDetailPage() {
  const { id } = useParams<{ id: string }>()
  const { liveVersion, members, channels } = useHive()
  const [wf, setWf] = useState<Workflow | null>(null)

  useEffect(() => {
    fetch(`/api/workflows/${id}`, { cache: 'no-store' })
      .then((r) => (r.ok ? r.json() : null))
      .then((d) => setWf(d?.workflow ?? null))
      .catch(() => {})
  }, [id, liveVersion])

  if (!wf) return <div className="flex flex-1 items-center justify-center text-[12px] text-dim">Loading…</div>
  const channel = channels.find((c) => c.id === wf.channelId)
  const memberById = new Map<string, Member>(members.map((m) => [m.id, m]))

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <header className="border-b border-line bg-panel/60 px-6 py-4">
        <div className="flex items-center gap-3">
          <h1 className="text-[17px] font-bold">{wf.name}</h1>
          <span
            className="chip"
            style={{
              color:
                wf.status === 'succeeded' ? 'var(--ok)' : wf.status === 'failed' ? 'var(--danger)' : 'var(--acc-2)',
            }}
          >
            {wf.status}
          </span>
          <span className="mono text-[10.5px] text-dim">{wf.id}</span>
        </div>
        <p className="mt-0.5 text-[12px] text-mut">
          trigger: {wf.triggerDesc} · room{' '}
          <Link href={`/c/${channel?.slug ?? 'general'}`} className="text-acc2">
            #{channel?.slug ?? '?'}
          </Link>{' '}
          · started {wf.startedAt ? relTime(wf.startedAt) : '—'}
          {wf.finishedAt ? ` · finished ${relTime(wf.finishedAt)}` : ''}
        </p>
      </header>
      <div className="mx-auto max-w-2xl p-6">
        {wf.steps.map((s, i) => {
          const agent = s.agentId ? memberById.get(s.agentId) : undefined
          const last = i === wf.steps.length - 1
          return (
            <div key={i} className="flex gap-3">
              <div className="flex flex-col items-center">
                {STEP_ICON[s.status]}
                {!last && <span className="w-px flex-1 bg-line2" />}
              </div>
              <div className="pb-6">
                <div className="flex items-center gap-2">
                  <span className="text-[13px] font-semibold">{s.name}</span>
                  {agent && (
                    <span className="flex items-center gap-1 text-[10.5px] text-mut">
                      <Avatar member={agent} size={14} /> {agent.displayName}
                    </span>
                  )}
                </div>
                {s.detail && <div className="mt-0.5 text-[11.5px] text-dim">{s.detail}</div>}
                <div className="mono mt-0.5 text-[9.5px] text-dim">step {i + 1} / {wf.steps.length}</div>
              </div>
            </div>
          )
        })}
        <div className="card mt-2 flex items-start gap-2 p-3 text-[11.5px] leading-relaxed text-dim">
          <ChevronRight size={12} className="mt-0.5 shrink-0" />
          Each step transition is a signed event in the room — the channel stays the record of why the work
          happened.
        </div>
      </div>
    </div>
  )
}
