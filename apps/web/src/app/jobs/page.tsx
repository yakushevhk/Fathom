'use client'

import { useEffect, useState } from 'react'
import { api, type Job } from '@/lib/api'

export default function JobsPage() {
  const [jobs, setJobs] = useState<Job[]>([])
  const [loading, setLoading] = useState(true)
  const [fetchError, setFetchError] = useState<string | null>(null)
  const [task, setTask] = useState('')
  const [creating, setCreating] = useState(false)
  const [createError, setCreateError] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [logId, setLogId] = useState<string | null>(null)
  const [logContent, setLogContent] = useState('')
  const [logLoading, setLogLoading] = useState(false)

  const load = async () => {
    setFetchError(null)
    try {
      const resp = await api.jobs.list()
      setJobs(resp.jobs ?? [])
    } catch (e) {
      setFetchError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { void (async () => { await load() })() }, [])

  const create = async () => {
    const nextTask = task.trim()
    if (!nextTask || creating) return
    setCreating(true)
    setCreateError(null)
    try {
      await api.jobs.create(nextTask)
      setTask('')
    } catch (e) {
      setCreateError(String(e))
      setCreating(false)
      return
    }
    setCreating(false)
    // Refresh separately: a successful dispatch should not be reported as a create failure
    // if the follow-up list request is temporarily unavailable.
    await load()
  }

  const viewLog = async (id: string) => {
    setLogLoading(true)
    setLogId(id)
    try {
      const resp = await api.jobs.log(id)
      setLogContent(resp.lines.join('\n'))
    } catch (e) {
      setLogContent(`Unable to load task log: ${String(e)}`)
    } finally {
      setLogLoading(false)
    }
  }

  const handleRerun = async (id: string) => {
    setActionError(null)
    try {
      await api.jobs.rerun(id)
      await load()
    } catch (e) {
      setActionError(`Could not rerun task: ${String(e)}`)
    }
  }

  const handleCancel = async (id: string) => {
    setActionError(null)
    try {
      await api.jobs.cancel(id)
      await load()
    } catch (e) {
      setActionError(`Could not cancel task: ${String(e)}`)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="h-9 flex items-center px-4 border-b border-white/[0.06] text-xs text-gray-400 shrink-0">
        Work / Tasks
      </div>
      <main className="p-4 sm:p-6 overflow-y-auto flex-1 space-y-6">
        <header>
          <p className="ops-kicker">Work queue</p>
          <h1 className="text-xl sm:text-2xl text-gray-100 font-medium tracking-tight mt-2">Submit and monitor tasks</h1>
          <p className="text-sm text-gray-500 mt-2 max-w-2xl">Send work to the autonomous worker fleet and follow each task from queue to completion.</p>
        </header>

        <section className="ops-panel" aria-labelledby="submit-task-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Dispatch</p>
              <h2 id="submit-task-heading">Submit a task</h2>
            </div>
          </div>
          <form onSubmit={e => { e.preventDefault(); create() }} className="flex flex-col sm:flex-row gap-2">
            <label htmlFor="new-task" className="sr-only">Task description</label>
            <input
              id="new-task"
              value={task}
              onChange={e => setTask(e.target.value)}
              placeholder="Describe work for an autonomous worker…"
              className="ops-input flex-1 min-w-0"
              disabled={creating}
            />
            <button type="submit" disabled={creating || !task.trim()} className="ops-button-primary w-full sm:w-auto justify-center">
              {creating ? 'Submitting…' : 'Submit task'}
            </button>
          </form>
          {createError && <div className="ops-alert mt-3" role="alert"><span>TASK NOT SUBMITTED</span>{createError}</div>}
        </section>

        {fetchError && (
          <div className="ops-alert flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3" role="alert">
            <div><span>WORK QUEUE UNAVAILABLE</span>{fetchError}</div>
            <button type="button" onClick={load} className="ops-button-secondary shrink-0">Retry</button>
          </div>
        )}
        {actionError && <div className="ops-alert" role="alert"><span>TASK ACTION FAILED</span>{actionError}</div>}

        <section aria-labelledby="task-queue-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Execution</p>
              <h2 id="task-queue-heading">Task queue</h2>
            </div>
            {!loading && !fetchError && <span className="text-[10px] text-gray-600">{jobs.length} {jobs.length === 1 ? 'task' : 'tasks'}</span>}
          </div>
          {loading ? (
            <div className="ops-panel text-xs text-gray-500" role="status">Loading task queue…</div>
          ) : fetchError ? (
            <div className="ops-panel text-xs text-gray-500">Task data is unavailable until the control plane reconnects.</div>
          ) : jobs.length === 0 ? (
            <div className="ops-panel text-xs text-gray-500">No tasks yet. Submit work above to dispatch your first task.</div>
          ) : (
            <div className="space-y-2">
              {jobs.map(j => (
                <article key={j.id} className="ops-panel">
                  <div className="flex flex-col sm:flex-row sm:items-start gap-2">
                    <div className="flex items-center gap-2 min-w-0">
                      <span className={`text-xs font-mono ${j.status === 'completed' ? 'text-green-500' : j.status === 'failed' ? 'text-red-500' : j.status === 'cancelled' ? 'text-gray-500' : 'text-blue-400'}`}>{j.status}</span>
                      <span className="text-[10px] text-gray-600 font-mono">{j.id.slice(0, 8)}</span>
                      <span className="text-[10px] text-gray-600">attempt {j.attempt}/{j.max_attempts}</span>
                    </div>
                    <div className="flex flex-wrap gap-1 sm:ml-auto" aria-label={`Actions for task ${j.id.slice(0, 8)}`}>
                      <button type="button" onClick={() => viewLog(j.id)} className="ops-button-secondary text-[10px] px-2 py-1">Log</button>
                      <button type="button" onClick={() => handleRerun(j.id)} className="ops-button-secondary text-[10px] px-2 py-1">Rerun</button>
                      {(j.status === 'running' || j.status === 'queued') && <button type="button" onClick={() => handleCancel(j.id)} className="text-[10px] text-red-400 hover:text-red-300 px-2 py-1 rounded border border-red-500/20 hover:border-red-500/40">Cancel</button>}
                    </div>
                  </div>
                  <p className="text-sm text-gray-300 mt-3 break-words">{j.task}</p>
                  {j.error && <p className="text-[10px] text-red-400 mt-2 font-mono break-words">{j.error}</p>}

                  {logId === j.id && (
                    <div className="mt-3 border-t border-white/[0.06] pt-3">
                      <div className="text-[10px] text-gray-500 mb-1">Task log</div>
                      <pre className="text-[10px] text-gray-400 font-mono bg-black/40 rounded p-2 max-h-48 overflow-y-auto whitespace-pre-wrap break-words">{logLoading ? 'Loading…' : logContent || '(empty)'}</pre>
                    </div>
                  )}
                </article>
              ))}
            </div>
          )}
        </section>
      </main>
    </div>
  )
}
