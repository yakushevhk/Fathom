'use client'

import Link from 'next/link'
import { useState } from 'react'
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Clock,
  FileDiff,
  GitBranch,
  GitPullRequest,
  Hexagon,
  Loader2,
  ShieldCheck,
  SmilePlus,
  UserPlus,
  XCircle,
} from 'lucide-react'
import { cn } from '@/lib/cn'
import { clockTime, relTime, truncKey } from '@/lib/format'
import { useHive } from '@/lib/store'
import type { HiveEvent, Member } from '@/lib/types'
import { Avatar } from './Avatar'
import { Markdown } from './Markdown'

const EMOJIS = ['👍', '🌊', '✅', '🔥', '👀', '🐝']

function AgentTag({ member }: { member: Member }) {
  return (
    <span
      className="chip mono !border-transparent !px-1.5 !py-0 text-[9px]"
      style={{ background: `${member.accent}22`, color: member.accent }}
      title={member.model ?? 'Fathom agent'}
    >
      <Hexagon size={9} />
      AGENT
    </span>
  )
}

function Reactions({ event }: { event: HiveEvent }) {
  const reactions = (event.meta.reactions as Record<string, string[]> | undefined) ?? {}
  const [picker, setPicker] = useState(false)
  const react = (emoji: string) => {
    setPicker(false)
    fetch(`/api/events/${event.id}/react`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ emoji }),
    }).catch(() => {})
  }
  return (
    <span className="relative mt-1 flex flex-wrap items-center gap-1">
      {Object.entries(reactions)
        .filter(([, users]) => users.length > 0)
        .map(([emoji, users]) => (
          <button
            key={emoji}
            onClick={() => react(emoji)}
            className="chip hover:border-acc"
            style={{ background: 'var(--panel-2)' }}
          >
            {emoji} {users.length}
          </button>
        ))}
      <button
        onClick={() => setPicker((v) => !v)}
        className="chip border-transparent text-dim opacity-0 transition-opacity hover:border-line2 hover:text-mut group-hover:opacity-100"
        title="React"
      >
        <SmilePlus size={11} />
      </button>
      {picker && (
        <span className="card absolute left-0 top-6 z-20 flex gap-1 p-1.5 shadow-lg">
          {EMOJIS.map((e) => (
            <button key={e} onClick={() => react(e)} className="rounded px-1 text-[14px] hover:bg-accsoft">
              {e}
            </button>
          ))}
        </span>
      )}
    </span>
  )
}

function RiskChip({ risk }: { risk: string }) {
  const color = risk === 'high' ? 'var(--danger)' : risk === 'medium' ? 'var(--gold)' : 'var(--ok)'
  return (
    <span className="chip" style={{ color, borderColor: color }}>
      <AlertTriangle size={9} /> {risk} risk
    </span>
  )
}

export function ApprovalCard({ event, compact }: { event: HiveEvent; compact?: boolean }) {
  const { members } = useHive()
  const [busy, setBusy] = useState(false)
  const apId = event.meta.approvalId as string | undefined
  const [status, setStatus] = useState<string | null>(null)
  const decided = event.meta.approvalStatus as string | undefined
  const agent = members.find((m) => m.id === event.authorId)
  const decide = async (d: 'approved' | 'rejected') => {
    if (!apId) return
    setBusy(true)
    const r = await fetch(`/api/approvals/${apId}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ decision: d }),
    })
    setBusy(false)
    if (r.ok) setStatus(d)
  }
  const final = status ?? decided ?? (compact ? 'pending' : 'pending')
  return (
    <div className={cn('card card-hover p-3', compact && 'p-2.5')}>
      <div className="mb-1 flex items-center gap-2 text-[11px] font-semibold text-gold">
        <ShieldCheck size={13} /> HUMAN GATE
        <RiskChip risk={(event.meta.risk as string) ?? 'low'} />
        <span className="mono ml-auto text-dim">{(event.meta.scope as string) ?? ''}</span>
      </div>
      <Markdown text={event.body} />
      {final === 'pending' ? (
        <div className="mt-2 flex gap-2">
          <button className="btn btn-primary !py-1 !text-[11px]" disabled={busy} onClick={() => decide('approved')}>
            <Check size={12} /> Approve
          </button>
          <button className="btn btn-danger !py-1 !text-[11px]" disabled={busy} onClick={() => decide('rejected')}>
            <XCircle size={12} /> Reject
          </button>
          {agent && <span className="ml-auto text-[10.5px] text-dim">requested by {agent.displayName}</span>}
        </div>
      ) : (
        <div className={cn('mt-2 text-[11px] font-semibold', final === 'approved' ? 'text-ok' : 'text-danger')}>
          {final === 'approved' ? '✓ Approved' : '✕ Rejected'}
        </div>
      )}
    </div>
  )
}

function PatchCard({ event }: { event: HiveEvent }) {
  const m = event.meta
  return (
    <div className="card card-hover p-3">
      <div className="mb-1 flex items-center gap-2 text-[11px] font-semibold text-acc2">
        <FileDiff size={13} /> PATCH
        <span className="chip" style={{ color: 'var(--acc)' }}>
          {String(m.status ?? 'open')}
        </span>
        <span className="mono ml-auto text-dim">
          {String(m.repo ?? '')}:{String(m.branch ?? '')}
        </span>
      </div>
      <Markdown text={event.body} />
      <div className="mono mt-1.5 flex gap-3 text-[10.5px] text-mut">
        <span>{String(m.commits ?? 1)} commit{Number(m.commits) > 1 ? 's' : ''}</span>
        <span className="text-ok">+{String(m.additions ?? 0)}</span>
        <span className="text-danger">−{String(m.deletions ?? 0)}</span>
        <span>{String(m.files ?? 0)} files</span>
        <span className="ml-auto flex items-center gap-1">
          <GitBranch size={10} /> {String(m.branch ?? '')}
        </span>
      </div>
    </div>
  )
}

function CiCard({ event }: { event: HiveEvent }) {
  const ok = event.meta.conclusion === 'success'
  return (
    <div className="card card-hover flex items-center gap-2.5 px-3 py-2">
      {ok ? <CheckCircle2 size={15} className="text-ok" /> : <XCircle size={15} className="text-danger" />}
      <div className="min-w-0 flex-1">
        <Markdown text={event.body} />
      </div>
      <span className="mono shrink-0 text-[10px] text-dim">
        {String(event.meta.commit ?? '')} · {String(event.meta.duration ?? '')}
      </span>
    </div>
  )
}

export function WorkflowCard({ event }: { event: HiveEvent }) {
  const status = String(event.meta.workflowStatus ?? 'running')
  const wfId = String(event.meta.workflowId ?? '')
  return (
    <Link href={`/workflows/${wfId}`} className="card card-hover block p-3">
      <div className="mb-1 flex items-center gap-2 text-[11px] font-semibold text-acc">
        <GitPullRequest size={13} /> WORKFLOW
        <span
          className="chip"
          style={{
            color: status === 'succeeded' ? 'var(--ok)' : status === 'failed' ? 'var(--danger)' : 'var(--acc-2)',
          }}
        >
          {status === 'running' && <Loader2 size={9} className="animate-spin" />}
          {status}
        </span>
        <span className="mono ml-auto text-dim">{wfId}</span>
      </div>
      <Markdown text={event.body} />
    </Link>
  )
}

function SystemLine({ event }: { event: HiveEvent }) {
  const join = event.kind === 'member'
  return (
    <div className="my-1 flex justify-center">
      <span className="syscard flex items-center gap-1.5">
        {join ? <UserPlus size={11} /> : <Clock size={11} />}
        <Markdown text={event.body} />
        <span className="text-[10px] text-dim">{relTime(event.createdAt)}</span>
      </span>
    </div>
  )
}

export function MessageItem({ event, member, dense }: { event: HiveEvent; member: Member | undefined; dense?: boolean }) {
  const { me } = useHive()
  if (event.kind === 'member' || event.kind === 'system') return <SystemLine event={event} />
  const name = member?.displayName ?? 'unknown'
  const reactionsBlock = <Reactions event={event} />
  const content =
    event.kind === 'patch' ? (
      <PatchCard event={event} />
    ) : event.kind === 'ci' ? (
      <CiCard event={event} />
    ) : event.kind === 'approval' ? (
      <ApprovalCard event={event} />
    ) : event.kind === 'workflow' ? (
      <WorkflowCard event={event} />
    ) : (
      <Markdown text={event.body} />
    )

  if (dense) {
    return (
      <div className="group flex gap-3 px-4 py-0.5 hover:bg-accsoft/40">
        <span className="w-[34px] shrink-0 pt-1 text-right text-[9.5px] text-dim opacity-0 group-hover:opacity-100">
          {clockTime(event.createdAt)}
        </span>
        <div className="min-w-0 flex-1">
          {content}
          {reactionsBlock}
        </div>
      </div>
    )
  }
  return (
    <div className="group flex gap-3 px-4 pb-1 pt-2 hover:bg-accsoft/40">
      {member ? (
        <Avatar member={member} size={32} className="mt-0.5" />
      ) : (
        <span className="h-8 w-8 shrink-0 rounded-full bg-panel2" />
      )}
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="text-[13px] font-bold" style={{ color: member?.accent ?? 'var(--fg)' }}>
            {name}
          </span>
          {member?.kind === 'agent' && <AgentTag member={member} />}
          <span className="text-[10px] text-dim">{clockTime(event.createdAt)}</span>
          {member && <span className="mono hidden text-[9px] text-dim group-hover:inline">{truncKey(member.signature)}</span>}
          {event.authorId === me?.id && <span className="text-[9px] text-dim">· you</span>}
        </div>
        {content}
        {reactionsBlock}
      </div>
    </div>
  )
}
