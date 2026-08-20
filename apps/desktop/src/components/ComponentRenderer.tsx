/**
 * The small component vocabulary accepted from the AG-UI bridge. Keeping the
 * discriminant and payloads explicit prevents server data from selecting an
 * arbitrary React component or executable code path.
 */
export type ComponentResponse =
  | { name: 'status'; props: StatusProps }
  | { name: 'audit_summary'; props: AuditSummaryProps }
  | { name: 'markdown'; props: MarkdownProps }
  | { name: 'ApprovalCard'; props: ApprovalCardProps }
  | { name: 'BrowserPreview'; props: BrowserPreviewProps }
  | { name: 'PolicyDecision'; props: PolicyDecisionProps }
  | { name: 'JobStatus'; props: JobStatusProps }

export interface StatusProps {
  title?: string
  state: string
  detail?: string
}

export interface AuditSummaryProps {
  title?: string
  total: number
  allowed: number
  denied: number
}

export interface MarkdownProps {
  content: string
}

export interface ApprovalCardProps {
  title?: string
  requestId?: string
  description?: string
  risk?: string
}

export interface BrowserPreviewProps {
  title?: string
  url: string
  description?: string
}

export interface PolicyDecisionProps {
  title?: string
  decision: string
  reason?: string
}

export interface JobStatusProps {
  title?: string
  jobId: string
  status: string
  detail?: string
  progress?: number
}

interface ComponentRendererProps {
  response: unknown
}

const MAX_PAYLOAD_BYTES = 32_768
const MAX_TEXT_LENGTH = 4_096

function boundedString(value: unknown): value is string {
  return typeof value === 'string' && value.length <= MAX_TEXT_LENGTH
}

function optionalString(value: unknown): string | undefined {
  return value === undefined ? undefined : boundedString(value) ? value : undefined
}

function validOptionalString(value: unknown): boolean {
  return value === undefined || boundedString(value)
}

function hasOnlyKeys(props: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(props).every((key) => keys.includes(key))
}

function hasOversizedString(props: Record<string, unknown>): boolean {
  return Object.values(props).some((value) => typeof value === 'string' && value.length > MAX_TEXT_LENGTH)
}

/** Parse untrusted JSON into the closed component vocabulary. */
export function parseComponentResponse(value: unknown): ComponentResponse | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null
  try {
    if (JSON.stringify(value).length > MAX_PAYLOAD_BYTES) return null
  } catch {
    return null
  }
  const envelope = value as { name?: unknown; props?: unknown }
  if (typeof envelope.name !== 'string' || typeof envelope.props !== 'object' || envelope.props === null || Array.isArray(envelope.props)) {
    return null
  }
  const props = envelope.props as Record<string, unknown>
  if (hasOversizedString(props)) return null

  switch (envelope.name) {
    case 'status': {
      if (!hasOnlyKeys(props, ['title', 'state', 'detail']) || !boundedString(props.state)
        || !validOptionalString(props.title) || !validOptionalString(props.detail)) return null
      return {
        name: 'status',
        props: {
          title: optionalString(props.title),
          state: props.state,
          detail: optionalString(props.detail),
        },
      }
    }
    case 'audit_summary': {
      if (!hasOnlyKeys(props, ['title', 'total', 'allowed', 'denied']) || !validOptionalString(props.title) || typeof props.total !== 'number' || !Number.isFinite(props.total)
        || typeof props.allowed !== 'number' || !Number.isFinite(props.allowed)
        || typeof props.denied !== 'number' || !Number.isFinite(props.denied)) {
        return null
      }
      return {
        name: 'audit_summary',
        props: {
          title: optionalString(props.title),
          total: props.total,
          allowed: props.allowed,
          denied: props.denied,
        },
      }
    }
    case 'markdown': {
      if (!hasOnlyKeys(props, ['content']) || !boundedString(props.content)) return null
      return { name: 'markdown', props: { content: props.content } }
    }
    case 'ApprovalCard': {
      if (!hasOnlyKeys(props, ['title', 'requestId', 'description', 'risk']) || !validOptionalString(props.title) || !validOptionalString(props.requestId) || !validOptionalString(props.description) || !validOptionalString(props.risk)) return null
      return { name: 'ApprovalCard', props: { title: optionalString(props.title), requestId: optionalString(props.requestId), description: optionalString(props.description), risk: optionalString(props.risk) } }
    }
    case 'BrowserPreview': {
      if (!hasOnlyKeys(props, ['title', 'url', 'description']) || !boundedString(props.url) || !validOptionalString(props.title) || !validOptionalString(props.description)) return null
      return { name: 'BrowserPreview', props: { title: optionalString(props.title), url: props.url, description: optionalString(props.description) } }
    }
    case 'PolicyDecision': {
      if (!hasOnlyKeys(props, ['title', 'decision', 'reason']) || !boundedString(props.decision) || !validOptionalString(props.title) || !validOptionalString(props.reason)) return null
      return { name: 'PolicyDecision', props: { title: optionalString(props.title), decision: props.decision, reason: optionalString(props.reason) } }
    }
    case 'JobStatus': {
      if (!hasOnlyKeys(props, ['title', 'jobId', 'status', 'detail', 'progress']) || !boundedString(props.jobId) || !boundedString(props.status) || !validOptionalString(props.title) || !validOptionalString(props.detail)) return null
      if (props.progress !== undefined && (typeof props.progress !== 'number' || !Number.isFinite(props.progress) || props.progress < 0 || props.progress > 100)) return null
      return { name: 'JobStatus', props: { title: optionalString(props.title), jobId: props.jobId, status: props.status, detail: optionalString(props.detail), progress: props.progress as number | undefined } }
    }
    default:
      return null
  }
}

/**
 * Render only validated, known response forms. All untrusted values are
 * passed as React text children; no HTML strings, eval, or dynamic component
 * lookup is used.
 */
export function ComponentRenderer({ response }: ComponentRendererProps) {
  const component = parseComponentResponse(response)
  if (!component) {
    return <div role="status" className="component-error">Unsupported component response</div>
  }

  switch (component.name) {
    case 'status':
      return <StatusComponent {...component.props} />
    case 'audit_summary':
      return <AuditSummaryComponent {...component.props} />
    case 'markdown':
      return <MarkdownComponent {...component.props} />
    case 'ApprovalCard':
      return <ApprovalCardComponent {...component.props} />
    case 'BrowserPreview':
      return <BrowserPreviewComponent {...component.props} />
    case 'PolicyDecision':
      return <PolicyDecisionComponent {...component.props} />
    case 'JobStatus':
      return <JobStatusComponent {...component.props} />
  }
}

function StatusComponent({ title = 'Status', state, detail }: StatusProps) {
  return (
    <section className="component-card component-status" aria-label={title}>
      <div className="component-card-title">{title}</div>
      <div className="component-status-state">{state}</div>
      {detail && <div className="text-secondary">{detail}</div>}
    </section>
  )
}

function AuditSummaryComponent({ title = 'Audit summary', total, allowed, denied }: AuditSummaryProps) {
  return (
    <section className="component-card component-audit" aria-label={title}>
      <div className="component-card-title">{title}</div>
      <dl className="component-audit-grid">
        <Metric label="Total" value={total} />
        <Metric label="Allowed" value={allowed} />
        <Metric label="Denied" value={denied} />
      </dl>
    </section>
  )
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <dt className="text-muted">{label}</dt>
      <dd>{value}</dd>
    </div>
  )
}

function MarkdownComponent({ content }: MarkdownProps) {
  // Deliberately render markdown as text. The bridge does not grant permission
  // to interpret arbitrary HTML embedded in a markdown response.
  const lines = content.split(/\r?\n/)
  return (
    <section className="component-card component-markdown" aria-label="Markdown">
      {lines.map((line, index) => (
        <p key={`${index}-${line}`}>{line || '\u00a0'}</p>
      ))}
    </section>
  )
}

function ApprovalCardComponent({ title = 'Approval required', requestId, description, risk }: ApprovalCardProps) {
  return <section className="component-card" aria-label={title}><div className="component-card-title">{title}</div>{requestId && <div>Request: {requestId}</div>}{risk && <div>Risk: {risk}</div>}{description && <div className="text-secondary">{description}</div>}</section>
}

function BrowserPreviewComponent({ title = 'Browser preview', url, description }: BrowserPreviewProps) {
  return <section className="component-card" aria-label={title}><div className="component-card-title">{title}</div><div>{url}</div>{description && <div className="text-secondary">{description}</div>}</section>
}

function PolicyDecisionComponent({ title = 'Policy decision', decision, reason }: PolicyDecisionProps) {
  return <section className="component-card" aria-label={title}><div className="component-card-title">{title}</div><div>{decision}</div>{reason && <div className="text-secondary">{reason}</div>}</section>
}

function JobStatusComponent({ title = 'Job status', jobId, status, detail, progress }: JobStatusProps) {
  return <section className="component-card" aria-label={title}><div className="component-card-title">{title}</div><div>{jobId}: {status}</div>{progress !== undefined && <div>Progress: {progress}%</div>}{detail && <div className="text-secondary">{detail}</div>}</section>
}
