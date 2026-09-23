'use client'

import Link from 'next/link'
import { useEffect, useState } from 'react'
import { CheckCircle2, ChevronRight, GitPullRequest, Loader2, XCircle } from 'lucide-react'
import { relTime } from '@/lib/format'
import type { Workflow } from '@/lib/types'
import { useHive } from '@/lib/store'

const STATUS_STYLE: Record<string, { color: string; icon: React.ReactNode }> = {
  running: { color: 'var(--acc-2)', icon: <Loader2 size={11} className="animate-spin" /> },
  queued: { color: 'var(--gold)', icon: null },
  succeeded: { color: 'var(--ok)', icon: <CheckCircle2 size={11} /> },
  failed: { color: 'var(--danger)', icon: <XCircle size={11} /> },
}

export default function WorkflowsPage() {
  const { liveVersion, channels } = useHive()
  const [workflows, setWorkflows] = useState<Workflow[]>([])

  useEffect(() => {
    fetch('/api/workflows', { cache: 'no-store' })
      .then((r) => r.json())
      .then((d) => setWorkflows(d.workflows))
      .catch(() => {})
  }, [liveVersion])

  const chById = new Map(channels.map((c) => [c.id, c]))

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <header className="border-b border-line bg-panel/60 px-6 py-4">
        <h1 className="flex items-center gap-2 text-[17px] font-bold">
          <GitPullRequest size={18} className="text-acc" /> Workflows
        </h1>
        <p className="mt-0.5 max-w-xl text-[12px] text-mut">
          DAG runs fired by tags, patches and schedules. Every transition lands in the log as a signed event.
        </p>
      </header>
      <div className="space-y-2 p-6">
        {workflows.map((w) => {
          const done = w.steps.filter((s) => s.status === 'succeeded').length
          const c = chById.get(w.channelId)
          const st = STATUS_STYLE[w.status]
          return (
            <Link key={w.id} href={`/workflows/${w.id}`} className="card card-hover block p-4">
              <div className="flex items-center gap-3">
                <ChevronRight size={14} className="text-dim" />
                <span className="text-[13.5px] font-bold">{w.name}</span>
                <span className="chip" style={{ color: st.color }}>
                  {st.icon} {w.status}
                </span>
                <span className="mono ml-auto text-[10.5px] text-dim">{w.id}</span>
              </div>
              <div className="mt-2 flex items-center gap-3">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-panel2">
                  <div
                    className="h-full rounded-full bg-gradient-to-r from-acc to-acc2 transition-all"
                    style={{ width: `${(done / Math.max(w.steps.length, 1)) * 100}%` }}
                  />
                </div>
                <span className="text-[10.5px] text-mut">
                  {done}/{w.steps.length} steps
                </span>
              </div>
              <div className="mt-2 flex items-center gap-3 text-[10.5px] text-dim">
                <span>trigger: {w.triggerDesc}</span>
                {c && (
                  <span className="chip" style={{ color: c.accent }}>
                    #{c.slug}
                  </span>
                )}
                <span className="ml-auto">{relTime(w.createdAt)}</span>
              </div>
            </Link>
          )
        })}
        {workflows.length === 0 && (
          <div className="card p-10 text-center text-[12.5px] text-dim">No workflow runs yet.</div>
        )}
      </div>
    </div>
  )
}
