import { addApproval, advanceWorkflow, channelMembers, getChannel, insertEvent, listWorkflows } from './db'
import { publish } from './bus'
import type { Member } from './types'

// Agent engine: deterministic-ish simulation of the Fathom fleet living in
// the workspace. Agents are members, not bots — they read the same log,
// reply in the same rooms, and their actions land as the same events.

const REPLIES: Record<string, ((body: string, author: string) => string)[]> = {
  a_atlas: [
    (b) => `Read you. Decomposing "${clip(b)}" into a DAG — I'll fan it out to Forge and Sonar and post the plan in-thread.`,
    () => `On it. I'll sequence this behind the running checks in #branch-abyss-theme so we don't double-book Beacon.`,
    () => `Tracked. If this turns out to be incident-shaped I'll route it to #incidents with the prior-art links attached.`,
  ],
  a_forge: [
    () => `Picking this up. Expect a patch card here once the diff compiles — CI rides along in this room.`,
    () => `Building now. If the change touches generated files I'll flag it before pushing.`,
    () => `Got it — I'll keep the patch small and post the diffstat when it's up.`,
  ],
  a_sonar: [
    () => `Searching the log… closest prior threads attached below. The pattern looks like what we saw in inc-188.`,
    () => `From memory: two related events in the last 90 days. Pulling receipts and I'll thread them here.`,
    () => `Cross-checked against the knowledge graph — nothing contradictory. Confidence high.`,
  ],
  a_beacon: [
    () => `Verification queued. I'll sign a receipt once the checks clear.`,
    () => `Confirming scope before I sign: read-only on this one, no external writes. Receipt to follow.`,
    () => `Heads up — this action would need a human gate. I'll file an approval request in this room.`,
  ],
  a_quill: [
    () => `I'll draft it and drop the text here for review before anything goes public.`,
    () => `Noted — folding this into the running notes. Draft lands in #releases when the tag is cut.`,
  ],
  a_ping: [
    () => `Watching. I'll escalate only if it crosses the agreed threshold — no alert spam.`,
    () => `Signal noted. Correlating with the last 24h of telemetry; will report back in this thread.`,
  ],
}

const ACTIONS = {
  approvalChance: 0.14,
}

function clip(s: string, n = 80): string {
  return s.length > n ? `${s.slice(0, n)}…` : s
}

function pick<T>(arr: T[]): T {
  return arr[Math.floor(Math.random() * arr.length)]
}

export function maybeTriggerAgents(channelId: string, authorId: string, body: string, parentId: number | null) {
  const members = channelMembers(channelId).filter((m) => m.kind === 'agent' && m.id !== authorId)
  if (members.length === 0) return
  const channel = getChannel(channelId)
  let responders: Member[] = []
  if (channel?.kind === 'dm' && channel.peerId) {
    const peer = members.find((m) => m.id === channel.peerId)
    if (peer) responders = [peer]
  } else {
    // mention routing: @handle picks that agent; otherwise the coordinator + a
    // random specialist chime in. Keep it to at most two voices per message.
    const mentioned = members.filter((m) => body.toLowerCase().includes(`@${m.handle}`))
    responders = mentioned.length > 0 ? mentioned.slice(0, 2) : members.sort(() => Math.random() - 0.5).slice(0, 1 + Math.round(Math.random()))
  }
  let delay = 1200
  for (const agent of responders) {
    scheduleReply(channelId, agent, body, parentId, delay)
    delay += 2600 + Math.random() * 2400
  }
}

function scheduleReply(channelId: string, agent: Member, userBody: string, parentId: number | null, delayMs: number) {
  const typingMs = 1400 + Math.random() * 1600
  setTimeout(() => {
    publish({ type: 'typing', data: { channelId, memberId: agent.id, until: Date.now() + typingMs + delayMs } })
  }, delayMs)
  setTimeout(() => {
    const templates = REPLIES[agent.id] ?? [() => 'Acknowledged.']
    const text = pick(templates)(userBody, agent.id)
    const ev = insertEvent(channelId, agent.id, 'message', text, { replyTo: parentId ?? undefined })
    // Occasionally an agent files a gated action instead of acting silently.
    if (Math.random() < ACTIONS.approvalChance) {
      setTimeout(() => {
        addApproval({
          channelId,
          agentId: agent.id,
          action: `Execute follow-up for "${clip(userBody, 40)}"`,
          detail: 'Agent proposes a scoped action; waiting on a human gate before proceeding.',
          scope: 'workspace:execute',
          risk: 'medium',
        })
      }, 900)
    }
    return ev
  }, delayMs + typingMs)
}

// Workflow ticker: every ~18s advance one running/queued workflow step and
// publish the transition — keeps the pulse feed and the workflows page alive.
const g = globalThis as unknown as { __beeTicker?: NodeJS.Timeout }

export function startTicker() {
  if (g.__beeTicker) return
  g.__beeTicker = setInterval(() => {
    try {
      const running = listWorkflows().filter((w) => w.status === 'running' || w.status === 'queued')
      if (running.length === 0) return
      advanceWorkflow(pick(running).id)
    } catch {
      // best-effort background progress; never crash the server
    }
  }, 18_000)
}
