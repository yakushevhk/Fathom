import { ShieldAlert, Check, X } from 'lucide-react'

export interface PendingDecision {
  requestId: string
  sessionId: string
  toolName?: string
  argsPreview?: string
  question?: string
  kind: 'approval' | 'question'
}

interface PendingApprovalBannerProps {
  decision: PendingDecision | null
  onApprove: (requestId: string) => void
  onDeny: (requestId: string) => void
  onAnswer?: (requestId: string, answer: string) => void
}

export function PendingApprovalBanner({
  decision,
  onApprove,
  onDeny,
}: PendingApprovalBannerProps) {
  if (!decision) return null

  return (
    <div className="pending-decision-banner">
      <div className="pending-banner-left">
        <div className="pending-pulse-badge">
          <ShieldAlert size={14} className="pending-icon" />
          <span className="pending-tag">Action Required</span>
        </div>
        <div className="pending-banner-info">
          <div className="pending-banner-title">
            {decision.kind === 'approval'
              ? `Agent requested approval to run: ${decision.toolName || 'Tool'}`
              : 'Agent asked a question'}
          </div>
          {decision.argsPreview && (
            <div className="pending-banner-args" title={decision.argsPreview}>
              {decision.argsPreview.length > 120
                ? `${decision.argsPreview.slice(0, 120)}...`
                : decision.argsPreview}
            </div>
          )}
        </div>
      </div>

      <div className="pending-banner-actions">
        <button
          type="button"
          className="pending-btn deny"
          onClick={() => onDeny(decision.requestId)}
          title="Deny execution (Cancel)"
        >
          <X size={12} style={{ marginRight: 4 }} />
          Deny
        </button>
        <button
          type="button"
          className="pending-btn approve"
          onClick={() => onApprove(decision.requestId)}
          title="Allow agent to proceed"
        >
          <Check size={12} style={{ marginRight: 4 }} />
          Allow Action
        </button>
      </div>
    </div>
  )
}
