'use client'

import { useSyncExternalStore } from 'react'
import { Cog, KeyRound, Moon, Sun, Waves } from 'lucide-react'
import { truncKey } from '@/lib/format'
import { useHive } from '@/lib/store'
import { Avatar } from '@/components/Avatar'

export default function SettingsPage() {
  const { me, community, members } = useHive()
  const theme = useSyncExternalStore(
    () => () => {},
    () => (document.documentElement.dataset.theme as 'dark' | 'light') || 'dark',
    () => 'dark'
  )
  const set = (t: 'dark' | 'light') => {
    document.documentElement.dataset.theme = t
    try {
      localStorage.setItem('bee-theme', t)
    } catch {}
  }

  const agents = members.filter((m) => m.kind === 'agent')

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <header className="border-b border-line bg-panel/60 px-6 py-4">
        <h1 className="flex items-center gap-2 text-[17px] font-bold">
          <Cog size={18} className="text-acc" /> Settings
        </h1>
        <p className="mt-0.5 text-[12px] text-mut">Workspace profile, identity and appearance.</p>
      </header>
      <div className="mx-auto max-w-2xl space-y-4 p-6">
        {me && (
          <section className="card p-4">
            <h2 className="mb-3 text-[11px] font-bold uppercase tracking-wider text-dim">Your identity</h2>
            <div className="flex items-center gap-3">
              <Avatar member={me} size={44} />
              <div>
                <div className="text-[14px] font-bold">{me.displayName}</div>
                <div className="text-[11.5px] text-mut">@{me.handle} · {me.title}</div>
              </div>
              <div className="mono ml-auto flex items-center gap-1.5 text-[10.5px] text-dim">
                <KeyRound size={12} className="text-acc" /> {truncKey(me.signature)}
              </div>
            </div>
            <p className="mt-3 text-[11.5px] leading-relaxed text-dim">
              Your key signs every event you publish. Agents sign with their own keys — the log knows who did what,
              whether the author was a person or a process.
            </p>
          </section>
        )}

        <section className="card p-4">
          <h2 className="mb-3 text-[11px] font-bold uppercase tracking-wider text-dim">Appearance</h2>
          <div className="flex gap-2">
            <button className={`btn flex-1 ${theme === 'dark' ? 'btn-primary' : ''}`} onClick={() => set('dark')}>
              <Moon size={14} /> Abyss
            </button>
            <button className={`btn flex-1 ${theme === 'light' ? 'btn-primary' : ''}`} onClick={() => set('light')}>
              <Sun size={14} /> Reef
            </button>
          </div>
        </section>

        <section className="card p-4">
          <h2 className="mb-3 text-[11px] font-bold uppercase tracking-wider text-dim">Community</h2>
          <dl className="space-y-2 text-[12.5px]">
            <div className="flex justify-between">
              <dt className="text-dim">Workspace</dt>
              <dd className="font-semibold">{community?.name}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-dim">Relay</dt>
              <dd className="mono">{community?.domain}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-dim">Events in log</dt>
              <dd className="mono">{community?.eventCount.toLocaleString()}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-dim">Members</dt>
              <dd>
                {members.filter((m) => m.kind === 'human').length} humans · {agents.length} agents
              </dd>
            </div>
          </dl>
        </section>

        <section className="card p-4">
          <h2 className="mb-2 flex items-center gap-1.5 text-[11px] font-bold uppercase tracking-wider text-dim">
            <Waves size={12} /> About
          </h2>
          <p className="text-[12px] leading-relaxed text-mut">
            Fathom is a hive workspace: humans and agents share the same rooms, the same event log and the same
            audit trail. This dashboard runs on a self-hosted Next.js relay — an embedded SQLite event store, a
            server-sent event stream, and a simulated Fathom fleet answering in the rooms they’re members of.
          </p>
          <p className="mt-2 text-[11px] text-dim">
            Design language: <span className="font-semibold text-acc">abyss</span> — deep-water surfaces,
            bioluminescent accents, hexagonal agent marks.
          </p>
        </section>
      </div>
    </div>
  )
}
