'use client'

import { useEffect, useState } from 'react'
import { api, type Job } from '@/lib/api'

export default function JobsPage() {
  const [jobs, setJobs] = useState<Job[]>([])
  const [loading, setLoading] = useState(true)
  const [task, setTask] = useState('')
  const [creating, setCreating] = useState(false)
  const [logId, setLogId] = useState<string | null>(null)
  const [logContent, setLogContent] = useState('')
  const [logLoading, setLogLoading] = useState(false)

  const load = async () => {
    try {
      const resp = await api.jobs.list()
      setJobs(resp.jobs ?? [])
    } catch { /* engine may be off */ }
    setLoading(false)
  }

  useEffect(() => { load() }, [])

  const create = async () => {
    if (!task.trim()) return
    setCreating(true)
    try {
      await api.jobs.create(task)
      setTask('')
      await load()
    } catch { /* ignore */ }
    setCreating(false)
  }

  const viewLog = async (id: string) => {
    setLogLoading(true)
    setLogId(id)
    try {
      const resp = await api.jobs.log(id)
      setLogContent(resp.lines.join('\n'))
    } catch { setLogContent('(no log available)') }
    setLogLoading(false)
  }

  const handleRerun = async (id: string) => {
    try {
      await api.jobs.rerun(id)
      await load()
    } catch { /* ignore */ }
  }

  const handleCancel = async (id: string) => {
    try {
      await api.jobs.cancel(id)
      await load()
    } catch { /* ignore */ }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        Background Jobs
      </div>
      <div className="p-6 overflow-y-auto flex-1">
        <div className="flex gap-2 mb-6">
          <input value={task} onChange={e => setTask(e.target.value)}
            placeholder="New research task..."
            className="flex-1 p-2 rounded-md bg-[#141414] border border-white/[0.06] text-sm text-gray-200 outline-none focus:border-gray-500"
          />
          <button onClick={create} disabled={creating || !task.trim()}
            className="px-4 py-2 rounded-md bg-gray-600 text-black text-xs font-semibold hover:bg-gray-400 disabled:opacity-30">
            Submit
          </button>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-8">
            <div className="w-4 h-4 rounded-full border border-white/10 border-t-white/60 animate-spin" />
          </div>
        ) : jobs.length === 0 ? (
          <p className="text-gray-600 text-xs">No jobs</p>
        ) : (
          <div className="space-y-2">
            {jobs.map(j => (
              <div key={j.id} className="p-3 rounded-md border border-white/[0.06] bg-[#141414]">
                <div className="flex items-center gap-2 mb-1">
                  <span className={`text-xs font-mono ${
                    j.status === 'completed' ? 'text-green-500' :
                    j.status === 'failed' ? 'text-red-500' :
                    j.status === 'cancelled' ? 'text-gray-500' :
                    'text-blue-400'
                  }`}>
                    {j.status}
                  </span>
                  <span className="text-[10px] text-gray-600 font-mono">{j.id.slice(0, 8)}</span>
                  <span className="text-[10px] text-gray-600">attempt {j.attempt}/{j.max_attempts}</span>
                  <div className="ml-auto flex gap-1">
                    <button onClick={() => viewLog(j.id)}
                      className="text-[10px] text-gray-500 hover:text-gray-300 px-1.5 py-0.5 rounded border border-white/[0.06] hover:border-white/20">
                      Log
                    </button>
                    <button onClick={() => handleRerun(j.id)}
                      className="text-[10px] text-gray-500 hover:text-gray-300 px-1.5 py-0.5 rounded border border-white/[0.06] hover:border-white/20">
                      Rerun
                    </button>
                    {(j.status === 'running' || j.status === 'queued') && (
                      <button onClick={() => handleCancel(j.id)}
                        className="text-[10px] text-red-400 hover:text-red-300 px-1.5 py-0.5 rounded border border-red-500/20 hover:border-red-500/40">
                        Cancel
                      </button>
                    )}
                  </div>
                </div>
                <p className="text-xs text-gray-300">{j.task}</p>
                {j.error && <p className="text-[10px] text-red-400 mt-1 font-mono">{j.error}</p>}

                {logId === j.id && (
                  <div className="mt-2 border-t border-white/[0.06] pt-2">
                    <div className="text-[10px] text-gray-500 mb-1">Log</div>
                    <pre className="text-[10px] text-gray-400 font-mono bg-black/40 rounded p-2 max-h-48 overflow-y-auto whitespace-pre-wrap">
                      {logLoading ? 'Loading...' : logContent || '(empty)'}
                    </pre>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}