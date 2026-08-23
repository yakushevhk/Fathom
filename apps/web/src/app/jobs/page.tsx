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
  const [actionStatus, setActionStatus] = useState<string | null>(null)
  const [logId, setLogId] = useState<string | null>(null)
  const [logContent, setLogContent] = useState('')
  const [logLoading, setLogLoading] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const [busyAction, setBusyAction] = useState<string | null>(null)

  const load = async (isRefresh = false) => {
    if (isRefresh) setRefreshing(true)
    else setLoading(true)
    setFetchError(null)
    try {
      const resp = await api.jobs.list()
      setJobs(resp.jobs ?? [])
    } catch (e) {
      setFetchError(e instanceof Error ? e.message : 'Unable to load jobs')
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }

  useEffect(() => { void (async () => { await load() })() }, [])

  const create = async () => {
    const nextTask = task.trim()
    if (!nextTask || creating) return
    setCreating(true)
    setCreateError(null)
    setActionStatus(null)
    try {
      await api.jobs.create(nextTask)
      setTask('')
      setActionStatus('Job dispatched successfully.')
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
    if (busyAction) return
    setBusyAction(`rerun:${id}`)
    setActionError(null)
    setActionStatus(null)
    try {
      await api.jobs.rerun(id)
      setActionStatus(`Job ${id.slice(0, 8)} rerun successfully.`)
      await load(true)
    } catch (e) {
      setActionError(`Could not rerun task: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setBusyAction(null)
    }
  }

  const handleCancel = async (id: string) => {
    if (busyAction || !window.confirm('Cancel this queued job?')) return
    setBusyAction(`cancel:${id}`)
    setActionError(null)
    setActionStatus(null)
    try {
      await api.jobs.cancel(id)
      setActionStatus(`Job ${id.slice(0, 8)} cancelled successfully.`)
      await load(true)
    } catch (e) {
      setActionError(`Could not cancel task: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setBusyAction(null)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="ops-toolbar shrink-0">
        <span className="ops-toolbar-title">Work / Jobs</span>
        <button type="button" className="ops-button-secondary ml-auto" onClick={() => void load(true)} disabled={loading || refreshing}>
          {refreshing ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>
      <div className="ops-page p-4 sm:p-6 space-y-6">
        <header className="ops-page-header">
          <p className="ops-kicker">Work queue</p>
          <h1 className="text-xl sm:text-2xl text-gray-100 font-medium tracking-tight mt-2">Dispatch and monitor jobs</h1>
          <p className="text-sm text-gray-500 mt-2 max-w-2xl">Dispatch work to the remote worker runtime and follow each job from queue to completion. Active jobs cannot be rerun until they finish or are cancelled.</p>
        </header>

        <section className="ops-panel" aria-labelledby="submit-task-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Dispatch</p>
              <h2 id="submit-task-heading">Dispatch a job</h2>
            </div>
          </div>
          <form onSubmit={e => { e.preventDefault(); create() }} className="flex flex-col sm:flex-row gap-2">
            <label htmlFor="jobs-new-task" className="sr-only">Task description</label>
            <input
              id="jobs-new-task"
              value={task}
              onChange={e => setTask(e.target.value)}
              placeholder="Describe work for a remote worker…"
              className="ops-input flex-1 min-w-0"
              disabled={creating}
            />
            <button type="submit" disabled={creating || !task.trim()} className="ops-button-primary w-full sm:w-auto justify-center">
              {creating ? 'Submitting…' : 'Dispatch job'}
            </button>
          </form>
          {createError && <div className="ops-alert mt-3" role="alert"><span>TASK NOT SUBMITTED</span>{createError}</div>}
        </section>

        {fetchError && (
          <div className="ops-alert flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3" role="alert">
            <div><span>WORK QUEUE UNAVAILABLE</span>{fetchError}</div>
            <button type="button" onClick={() => void load(true)} className="ops-button-secondary shrink-0" disabled={refreshing}>Retry</button>
          </div>
        )}
        {actionError && <div className="ops-alert" role="alert"><span>TASK ACTION FAILED</span>{actionError}</div>}
        {actionStatus && <div className="sr-only" role="status" aria-live="polite">{actionStatus}</div>}

        <section aria-labelledby="task-queue-heading">
          <div className="ops-panel-head">
            <div>
              <p className="ops-kicker">Execution</p>
              <h2 id="task-queue-heading">Job queue</h2>
            </div>
            {!loading && !fetchError && <span className="text-[10px] text-gray-600">{jobs.length} {jobs.length === 1 ? 'task' : 'tasks'}</span>}
          </div>
          {loading ? (
            <div className="ops-panel text-xs text-gray-500" role="status">Loading task queue…</div>
          ) : fetchError ? (
            <div className="ops-panel text-xs text-gray-500">Task data is unavailable until the control plane reconnects.</div>
          ) : jobs.length === 0 ? (
            <div className="ops-panel text-xs text-gray-500">No jobs yet. Dispatch work above to create the first queued job.</div>
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
                      <button type="button" onClick={() => void viewLog(j.id)} className="ops-button-secondary text-[10px] px-2 py-1" disabled={logLoading && logId === j.id}>Log</button>
                      {j.status !== 'running' && j.status !== 'queued' && <button type="button" onClick={() => void handleRerun(j.id)} className="ops-button-secondary text-[10px] px-2 py-1" disabled={busyAction !== null}>{busyAction === `rerun:${j.id}` ? 'Rerunning…' : 'Rerun'}</button>}
                      {(j.status === 'running' || j.status === 'queued') && <button type="button" onClick={() => void handleCancel(j.id)} className="text-[10px] text-red-400 hover:text-red-300 px-2 py-1 rounded border border-red-500/20 hover:border-red-500/40 disabled:opacity-40" disabled={busyAction !== null}>{busyAction === `cancel:${j.id}` ? 'Cancelling…' : 'Cancel'}</button>}
                    </div>
                  </div>
                  <p className="text-sm text-gray-300 mt-3 break-words">{j.task}</p>
                  {j.error && <p className="text-[10px] text-red-400 mt-2 font-mono break-words">{j.error}</p>}

                  {logId === j.id && (
                    <div className="mt-3 border-t border-white/[0.06] pt-3">
                      <div className="text-[10px] text-gray-500 mb-1">Job log</div>
                      <pre className="text-[10px] text-gray-400 font-mono bg-black/40 rounded p-2 max-h-48 overflow-y-auto whitespace-pre-wrap break-words">{logLoading ? 'Loading…' : logContent || '(empty)'}</pre>
                    </div>
                  )}
                </article>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  )
}
