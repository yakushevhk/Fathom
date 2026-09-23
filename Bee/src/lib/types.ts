export type MemberKind = 'human' | 'agent'
export type Presence = 'online' | 'away' | 'busy' | 'offline'
export type ChannelKind = 'channel' | 'dm' | 'announcement'
export type EventKind =
  | 'message'
  | 'patch'
  | 'ci'
  | 'approval'
  | 'workflow'
  | 'member'
  | 'system'

export type ApprovalStatus = 'pending' | 'approved' | 'rejected'
export type WorkflowStatus = 'queued' | 'running' | 'succeeded' | 'failed'
export type StepStatus = 'pending' | 'running' | 'succeeded' | 'failed' | 'skipped'

export interface Member {
  id: string
  handle: string
  displayName: string
  kind: MemberKind
  title: string
  presence: Presence
  bio: string
  accent: string
  signature: string
  model?: string
}

export interface Channel {
  id: string
  slug: string
  name: string
  topic: string
  kind: ChannelKind
  accent: string
  memberCount: number
  unread: number
  lastEventAt: number | null
  peerId?: string
}

export interface HiveEvent {
  id: number
  channelId: string
  authorId: string
  kind: EventKind
  body: string
  meta: Record<string, unknown>
  parentId: number | null
  createdAt: number
}

export interface WorkflowStep {
  name: string
  status: StepStatus
  detail: string
  agentId?: string
}

export interface Workflow {
  id: string
  name: string
  triggerDesc: string
  channelId: string
  status: WorkflowStatus
  steps: WorkflowStep[]
  createdAt: number
  startedAt: number | null
  finishedAt: number | null
}

export interface Approval {
  id: string
  channelId: string
  agentId: string
  action: string
  detail: string
  scope: string
  risk: 'low' | 'medium' | 'high'
  status: ApprovalStatus
  decidedBy: string | null
  decidedAt: number | null
  createdAt: number
}

export interface Bootstrap {
  me: Member
  community: { name: string; domain: string; eventCount: number }
  members: Member[]
  channels: Channel[]
}
