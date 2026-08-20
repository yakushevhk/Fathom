'use client'

import { useEffect, useMemo, useState } from 'react'
import {
  api,
  type Channel,
  type Coworker,
  type Schedule,
} from '@/lib/api'

type CoworkerDraft = Omit<Coworker, 'id' | 'created_at' | 'updated_at'>

const emptyDraft: CoworkerDraft = {
  name: '', title: '', role: '', prompt: '', visibility: 'private', active: true,
}

const emptySchedule = { coworker_id: '', cron_expression: '0 9 * * 1', timezone: 'UTC', query: '', enabled: true }

function messageFor(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback
}

function isOffline(error: unknown) {
  if (!error || typeof error !== 'object' || !('status' in error)) return false
  const status = error.status
  return status === null
}

function CoworkerEditor({
  draft,
  editing,
  saving,
  onChange,
  onSubmit,
  onCancel,
  readOnly,
}: {
  draft: CoworkerDraft
  editing: boolean
  saving: boolean
  onChange: (next: CoworkerDraft) => void
  onSubmit: (event: React.FormEvent) => void
  onCancel: () => void
  readOnly: boolean
}) {
  return (
    <form onSubmit={onSubmit} className="space-y-3">
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="ops-label">Name<input required disabled={readOnly} maxLength={100} className="ops-input mt-1" value={draft.name} onChange={e => onChange({ ...draft, name: e.target.value })} placeholder="Research operator" /></label>
        <label className="ops-label">Title<input disabled={readOnly} maxLength={200} className="ops-input mt-1" value={draft.title} onChange={e => onChange({ ...draft, title: e.target.value })} placeholder="Market intelligence" /></label>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="ops-label">Role<input disabled={readOnly} maxLength={100} className="ops-input mt-1" value={draft.role} onChange={e => onChange({ ...draft, role: e.target.value })} placeholder="Analyst" /></label>
        <label className="ops-label">Visibility<select disabled={readOnly} className="ops-input mt-1" value={draft.visibility} onChange={e => onChange({ ...draft, visibility: e.target.value })}><option value="private">Private</option><option value="team">Team</option><option value="public">Public</option></select></label>
      </div>
      <label className="ops-label">System prompt<textarea required disabled={readOnly} maxLength={20000} rows={5} className="ops-input mt-1 resize-y" value={draft.prompt} onChange={e => onChange({ ...draft, prompt: e.target.value })} placeholder="Describe how this coworker should work…" /></label>
      <label className="flex items-center gap-2 text-xs text-gray-400"><input type="checkbox" disabled={readOnly} checked={draft.active} onChange={e => onChange({ ...draft, active: e.target.checked })} className="accent-[#c0d4ce]" />Available for new work</label>
      <div className="flex justify-end gap-2 pt-1">
        {editing && <button type="button" onClick={onCancel} className="ops-button-secondary">Cancel</button>}
        {!readOnly && <button type="submit" disabled={saving || !draft.name.trim() || !draft.prompt.trim()} className="ops-button-primary">{saving ? 'Saving…' : editing ? 'Save changes' : 'Create coworker'}</button>}
      </div>
    </form>
  )
}

export default function CoworkerConsole() {
  const [coworkers, setCoworkers] = useState<Coworker[]>([])
  const [channels, setChannels] = useState<Channel[]>([])
  const [schedules, setSchedules] = useState<Schedule[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draft, setDraft] = useState<CoworkerDraft>(emptyDraft)
  const [editing, setEditing] = useState(false)
  const [channelTitle, setChannelTitle] = useState('')
  const [sessionId, setSessionId] = useState('')
  const [scheduleDraft, setScheduleDraft] = useState(emptySchedule)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [channelLoading, setChannelLoading] = useState(false)
  const [scheduleSaving, setScheduleSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [offline, setOffline] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)

  const selected = useMemo(() => coworkers.find(item => item.id === selectedId) ?? null, [coworkers, selectedId])

  const load = async () => {
    setLoading(true); setError(null); setOffline(false)
    const [coworkerResult, scheduleResult] = await Promise.allSettled([api.coworkers.list(), api.schedules.list()])
    if (coworkerResult.status === 'fulfilled') {
      const nextCoworkers = coworkerResult.value.coworkers ?? []
      setCoworkers(nextCoworkers)
      setSelectedId(current => current && nextCoworkers.some(item => item.id === current) ? current : nextCoworkers[0]?.id ?? null)
    } else {
      setError(messageFor(coworkerResult.reason, 'Could not reach the coworker service'))
      setOffline(isOffline(coworkerResult.reason))
    }
    if (scheduleResult.status === 'fulfilled') {
      setSchedules(scheduleResult.value.schedules ?? [])
    } else if (coworkerResult.status === 'fulfilled') {
      setError(messageFor(scheduleResult.reason, 'Could not load schedules'))
      setOffline(isOffline(scheduleResult.reason))
    }
    setLoading(false)
  }

  useEffect(() => { void load() }, [])

  useEffect(() => {
    if (!selected) { setChannels([]); return }
    setDraft({ name: selected.name, title: selected.title, role: selected.role, prompt: selected.prompt, visibility: selected.visibility, active: selected.active })
    setScheduleDraft(current => ({ ...current, coworker_id: selected.id }))
    setChannelLoading(true)
    api.channels.list(selected.id).then(response => setChannels(response.channels ?? [])).catch(e => {
      setError(messageFor(e, 'Could not load channels'))
      setOffline(isOffline(e))
    }).finally(() => setChannelLoading(false))
  }, [selected])

  const createMode = () => { setSelectedId(null); setEditing(false); setDraft(emptyDraft); setChannels([]) }
  const editMode = () => { if (selected) { setEditing(true); setDraft({ name: selected.name, title: selected.title, role: selected.role, prompt: selected.prompt, visibility: selected.visibility, active: selected.active }) } }

  const saveCoworker = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true); setError(null); setNotice(null)
    try {
      const response = editing && selected ? await api.coworkers.update(selected.id, draft) : await api.coworkers.create(draft)
      const saved = response.coworker
      setCoworkers(current => editing ? current.map(item => item.id === saved.id ? saved : item) : [saved, ...current])
      setSelectedId(saved.id); setEditing(false); setNotice(editing ? 'Coworker updated' : 'Coworker created')
    } catch (e) { setError(messageFor(e, 'Could not save coworker')); setOffline(isOffline(e)) }
    finally { setSaving(false) }
  }

  const archiveCoworker = async () => {
    if (!selected || !window.confirm(`Archive ${selected.name}?`)) return
    setError(null); setNotice(null)
    try {
      await api.coworkers.archive(selected.id)
      const remaining = coworkers.filter(item => item.id !== selected.id)
      setCoworkers(remaining); setSelectedId(remaining[0]?.id ?? null); setNotice('Coworker archived')
    } catch (e) { setError(messageFor(e, 'Could not archive coworker')); setOffline(isOffline(e)) }
  }

  const createChannel = async (event: React.FormEvent) => {
    event.preventDefault(); if (!selected || !channelTitle.trim()) return
    setChannelLoading(true); setError(null)
    try {
      const response = await api.channels.create({ coworker_id: selected.id, title: channelTitle, ...(sessionId.trim() ? { session_id: sessionId.trim() } : {}) })
      setChannels(current => [response.channel, ...current]); setChannelTitle(''); setSessionId(''); setNotice('Channel created')
    } catch (e) { setError(messageFor(e, 'Could not create channel')); setOffline(isOffline(e)) }
    finally { setChannelLoading(false) }
  }

  const deleteChannel = async (channel: Channel) => {
    setError(null)
    try { await api.channels.delete(channel.id); setChannels(current => current.filter(item => item.id !== channel.id)); setNotice('Channel deleted') }
    catch (e) { setError(messageFor(e, 'Could not delete channel')); setOffline(isOffline(e)) }
  }

  const createSchedule = async (event: React.FormEvent) => {
    event.preventDefault(); if (!scheduleDraft.coworker_id || !scheduleDraft.query.trim()) return
    setScheduleSaving(true); setError(null)
    try {
      const response = await api.schedules.create(scheduleDraft)
      setSchedules(current => [...current, response.schedule].sort((a, b) => a.next_run.localeCompare(b.next_run)))
      setScheduleDraft(current => ({ ...current, query: '' })); setNotice('Schedule created')
    } catch (e) { setError(messageFor(e, 'Could not create schedule')); setOffline(isOffline(e)) }
    finally { setScheduleSaving(false) }
  }

  return (
    <div className="flex-1 overflow-y-auto bg-[#080909]">
      <header className="border-b border-white/[0.08] px-5 py-5 md:px-8">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div><p className="ops-kicker">Control plane / coworker fleet</p><h1 className="mt-1 text-xl tracking-tight text-gray-100">Coworkers &amp; schedules</h1><p className="mt-1 max-w-2xl text-xs text-gray-500">Configure the people-shaped interfaces that run research, route conversations, and keep recurring work on time.</p></div>
          <button onClick={() => void load()} disabled={loading} className="ops-button-secondary">{loading ? 'Syncing…' : '↻ Refresh'}</button>
        </div>
      </header>
      <div className="grid gap-5 p-5 md:p-8 xl:grid-cols-[minmax(230px,.75fr)_minmax(420px,1.45fr)]">
        <aside className="space-y-5">
          {error && <div role="alert" className="ops-alert"><span>{offline ? 'SERVICE OFFLINE' : 'SERVICE NOTICE'}</span><p>{error}</p></div>}
          {notice && <div className="ops-notice">{notice}</div>}
          <section className="ops-panel">
            <div className="ops-panel-head"><div><p className="ops-kicker">Directory · {coworkers.length}</p><h2>Configured coworkers</h2></div><button onClick={createMode} className="ops-button-secondary">+ New</button></div>
            {loading ? <div className="ops-empty">Loading coworker directory…</div> : coworkers.length === 0 ? <div className="ops-empty">No coworkers configured. Create one to start routing work.</div> : <div className="space-y-1">{coworkers.map(item => <button key={item.id} onClick={() => { setSelectedId(item.id); setEditing(false) }} className={`w-full border-l-2 px-3 py-2.5 text-left transition-colors ${selectedId === item.id ? 'border-[#b9d0c8] bg-white/[0.07]' : 'border-transparent hover:bg-white/[0.03]'}`}><span className="flex items-center justify-between gap-2"><span className="truncate text-xs text-gray-200">{item.name}</span><span className={`h-1.5 w-1.5 shrink-0 rounded-full ${item.active ? 'bg-emerald-400' : 'bg-gray-600'}`} /></span><span className="mt-1 block truncate text-[10px] text-gray-600">{item.title || item.role || 'Unassigned role'} · {item.visibility}</span></button>)}</div>}
          </section>
          <section className="ops-panel">
            <div className="ops-panel-head"><div><p className="ops-kicker">Recurring runs · {schedules.length}</p><h2>Schedules</h2></div></div>
            {loading ? <div className="ops-empty">Loading schedules…</div> : schedules.length === 0 ? <div className="ops-empty">No recurring runs yet.</div> : <div className="max-h-64 space-y-3 overflow-y-auto">{schedules.map(schedule => <article key={schedule.id} className="border-l border-white/[0.12] pl-3"><div className="flex items-center justify-between gap-2"><code className="text-[11px] text-gray-300">{schedule.cron_expression}</code><span className={`text-[9px] uppercase tracking-wider ${schedule.enabled ? 'text-emerald-400' : 'text-gray-600'}`}>{schedule.enabled ? 'enabled' : 'paused'}</span></div><p className="mt-1 line-clamp-2 text-[11px] text-gray-500">{schedule.query}</p><p className="mt-1 font-mono text-[9px] text-gray-700">next {new Date(schedule.next_run).toLocaleString()} · {schedule.timezone}</p></article>)}</div>}
          </section>
        </aside>
        <div className="space-y-5">
          <section className="ops-panel">
            <div className="ops-panel-head"><div><p className="ops-kicker">{editing ? 'Edit profile' : selected ? 'Selected profile' : 'New profile'}</p><h2>{selected && !editing ? selected.name : 'Coworker profile'}</h2></div>{selected && !editing && <div className="flex gap-2"><button onClick={editMode} className="ops-button-secondary">Edit</button><button onClick={() => void archiveCoworker()} className="ops-button-secondary text-red-300 hover:border-red-300/40">Archive</button></div>}</div>
            <CoworkerEditor draft={draft} editing={editing} saving={saving} readOnly={Boolean(selected && !editing)} onChange={setDraft} onSubmit={saveCoworker} onCancel={() => setEditing(false)} />
          </section>
          <section className="ops-panel">
            <div className="ops-panel-head"><div><p className="ops-kicker">Conversation routing · {channels.length}</p><h2>{selected ? `${selected.name}'s channels` : 'Channels'}</h2></div></div>
            {!selected ? <div className="ops-empty">Select a coworker to manage channels.</div> : <><form onSubmit={createChannel} className="grid gap-2 sm:grid-cols-[1fr_1fr_auto]"><input required maxLength={200} className="ops-input" value={channelTitle} onChange={e => setChannelTitle(e.target.value)} placeholder="Channel title" /><input maxLength={128} className="ops-input font-mono" value={sessionId} onChange={e => setSessionId(e.target.value)} placeholder="Session ID (optional)" /><button className="ops-button-secondary" disabled={channelLoading || !channelTitle.trim()}>{channelLoading ? 'Adding…' : 'Add channel'}</button></form>{channels.length === 0 ? <div className="ops-empty">No channels yet. Add one above.</div> : <div className="mt-4 divide-y divide-white/[0.06]">{channels.map(channel => <div key={channel.id} className="flex items-center justify-between gap-3 py-2 text-xs"><div><span className="text-gray-300">{channel.title}</span>{channel.session_id && <span className="ml-2 font-mono text-[10px] text-gray-600">{channel.session_id.slice(0, 12)}</span>}</div><button onClick={() => void deleteChannel(channel)} className="text-[10px] text-gray-600 hover:text-red-300">Delete</button></div>)}</div>}</>}
          </section>
          <section className="ops-panel">
            <div className="ops-panel-head"><div><p className="ops-kicker">Automation</p><h2>Schedule a recurring run</h2></div></div>
            <form onSubmit={createSchedule} className="space-y-3"><div className="grid gap-3 sm:grid-cols-3"><label className="ops-label">Coworker<select required className="ops-input mt-1" value={scheduleDraft.coworker_id} onChange={e => setScheduleDraft(current => ({ ...current, coworker_id: e.target.value }))}><option value="">Select coworker</option>{coworkers.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</select></label><label className="ops-label">Cron expression<input required maxLength={256} className="ops-input mt-1 font-mono" value={scheduleDraft.cron_expression} onChange={e => setScheduleDraft(current => ({ ...current, cron_expression: e.target.value }))} placeholder="0 9 * * 1" /></label><label className="ops-label">Timezone<input required maxLength={128} className="ops-input mt-1" value={scheduleDraft.timezone} onChange={e => setScheduleDraft(current => ({ ...current, timezone: e.target.value }))} placeholder="UTC" /></label></div><label className="ops-label">Run query<textarea required maxLength={20000} rows={3} className="ops-input mt-1 resize-y" value={scheduleDraft.query} onChange={e => setScheduleDraft(current => ({ ...current, query: e.target.value }))} placeholder="What should this coworker check on each run?" /></label><div className="flex items-center justify-between gap-3"><label className="flex items-center gap-2 text-xs text-gray-400"><input type="checkbox" checked={scheduleDraft.enabled} onChange={e => setScheduleDraft(current => ({ ...current, enabled: e.target.checked }))} className="accent-[#c0d4ce]" />Start schedule enabled</label><button className="ops-button-primary" disabled={scheduleSaving || !scheduleDraft.coworker_id || !scheduleDraft.query.trim()}>{scheduleSaving ? 'Scheduling…' : 'Create schedule'}</button></div></form>
          </section>
        </div>
      </div>
    </div>
  )
}
