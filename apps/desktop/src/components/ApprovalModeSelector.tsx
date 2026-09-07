import { ShieldAlert, ShieldCheck, Hand, Settings } from 'lucide-react'

export type ApprovalMode = 'ask' | 'auto' | 'full' | 'custom'

interface ApprovalModeOption {
  mode: ApprovalMode
  label: string
  badge: string
  description: string
  icon: typeof Hand
}

const MODES: ApprovalModeOption[] = [
  {
    mode: 'ask',
    label: 'Ask First',
    badge: 'Safe',
    description: 'Prompts for confirmation on every command, file write, and external tool action.',
    icon: Hand,
  },
  {
    mode: 'auto',
    label: 'Auto Review',
    badge: 'Standard',
    description: 'Pre-screens actions with security guardrails; asks only when risk thresholds are exceeded.',
    icon: ShieldCheck,
  },
  {
    mode: 'full',
    label: 'Autonomous',
    badge: 'Fast',
    description: 'Full autonomous execution for trusted workflows without interruption.',
    icon: ShieldAlert,
  },
  {
    mode: 'custom',
    label: 'Custom Policy',
    badge: 'Granular',
    description: 'Enforces custom YAML/JSON rule-sets from the Governance panel.',
    icon: Settings,
  },
]

interface ApprovalModeSelectorProps {
  currentMode: ApprovalMode
  onChange: (mode: ApprovalMode) => void
  compact?: boolean
}

export function ApprovalModeSelector({ currentMode, onChange, compact = false }: ApprovalModeSelectorProps) {
  return (
    <div className={`approval-mode-selector ${compact ? 'compact' : ''}`}>
      {MODES.map(opt => {
        const Icon = opt.icon
        const isSelected = currentMode === opt.mode
        return (
          <button
            key={opt.mode}
            type="button"
            className={`approval-mode-pill ${isSelected ? 'active' : ''}`}
            onClick={() => onChange(opt.mode)}
            title={opt.description}
          >
            <Icon size={13} className="mode-icon" />
            <span className="mode-label">{opt.label}</span>
            <span className="mode-badge">{opt.badge}</span>
          </button>
        )
      })}
    </div>
  )
}
