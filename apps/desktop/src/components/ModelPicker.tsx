import { useState, useRef, useEffect } from 'react'
import { ChevronDown, Cpu, Sparkles, Check } from 'lucide-react'

export type EffortLevel = 'none' | 'low' | 'medium' | 'high' | 'xhigh'

export interface ModelOption {
  id: string
  label: string
  provider: string
  description: string
  tier: 'cloud' | 'local'
  badge?: string
}

export const AVAILABLE_MODELS: ModelOption[] = [
  {
    id: 'anthropic/claude-3-7-sonnet',
    label: 'Claude 3.7 Sonnet',
    provider: 'Anthropic',
    description: 'Hybrid reasoning & code specialist with extended thinking budget',
    tier: 'cloud',
    badge: 'Reasoning',
  },
  {
    id: 'deepseek/deepseek-chat',
    label: 'DeepSeek V3',
    provider: 'DeepSeek',
    description: 'Fast, cost-efficient 671B MoE model for high-throughput tasks',
    tier: 'cloud',
    badge: 'Fast',
  },
  {
    id: 'deepseek/deepseek-reasoner',
    label: 'DeepSeek R1',
    provider: 'DeepSeek',
    description: 'Deep mathematical and algorithmic chain-of-thought model',
    tier: 'cloud',
    badge: 'R1',
  },
  {
    id: 'openai/gpt-4o',
    label: 'GPT-4o',
    provider: 'OpenAI',
    description: 'Multimodal vision and versatile general intelligence',
    tier: 'cloud',
  },
  {
    id: 'ollama/llama3.3',
    label: 'Llama 3.3 70B',
    provider: 'Local Ollama',
    description: 'On-device open weights running locally via Ollama loopback',
    tier: 'local',
    badge: 'Private',
  },
]

const EFFORT_LEVELS: { level: EffortLevel; label: string; desc: string }[] = [
  { level: 'none', label: 'Off', desc: 'Direct response without chain-of-thought' },
  { level: 'low', label: 'Low', desc: 'Brief thinking (up to 2k tokens)' },
  { level: 'medium', label: 'Med', desc: 'Standard reasoning (up to 8k tokens)' },
  { level: 'high', label: 'High', desc: 'Deep planning & verification (up to 16k tokens)' },
  { level: 'xhigh', label: 'Max', desc: 'Exhaustive verification (up to 32k tokens)' },
]

interface ModelPickerProps {
  selectedModel: string
  onSelectModel: (modelId: string) => void
  effort: EffortLevel
  onSelectEffort: (effort: EffortLevel) => void
  compact?: boolean
}

export function ModelPicker({
  selectedModel,
  onSelectModel,
  effort,
  onSelectEffort,
  compact = false,
}: ModelPickerProps) {
  const [open, setOpen] = useState(false)
  const [activeTab, setActiveTab] = useState<'cloud' | 'local'>('cloud')
  const [search, setSearch] = useState('')
  const pickerRef = useRef<HTMLDivElement>(null)

  const current = AVAILABLE_MODELS.find(m => m.id === selectedModel) || AVAILABLE_MODELS[0]

  useEffect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    if (open) {
      document.addEventListener('mousedown', handleOutsideClick)
    }
    return () => document.removeEventListener('mousedown', handleOutsideClick)
  }, [open])

  const filtered = AVAILABLE_MODELS.filter(m => {
    const matchTier = m.tier === activeTab
    const q = search.trim().toLowerCase()
    const matchSearch = !q || m.label.toLowerCase().includes(q) || m.provider.toLowerCase().includes(q)
    return matchTier && matchSearch
  })

  return (
    <div className="model-picker-container" ref={pickerRef}>
      <button
        type="button"
        className={`model-picker-trigger ${open ? 'open' : ''} ${compact ? 'compact' : ''}`}
        onClick={() => setOpen(!open)}
        title="Change AI Model & Reasoning Effort"
      >
        <Sparkles size={13} className="model-sparkle" />
        <span className="model-trigger-name">{current.label}</span>
        {effort !== 'none' && (
          <span className="effort-tag">
            {effort.toUpperCase()}
          </span>
        )}
        <ChevronDown size={12} className="chevron" />
      </button>

      {open && (
        <div className="model-picker-popover">
          {/* Header & Rail Tabs */}
          <div className="model-popover-header">
            <div className="model-rail-tabs">
              <button
                className={`rail-tab ${activeTab === 'cloud' ? 'active' : ''}`}
                onClick={() => setActiveTab('cloud')}
              >
                <Sparkles size={12} />
                <span>Cloud Models</span>
              </button>
              <button
                className={`rail-tab ${activeTab === 'local' ? 'active' : ''}`}
                onClick={() => setActiveTab('local')}
              >
                <Cpu size={12} />
                <span>Local Engines</span>
              </button>
            </div>
            <input
              type="text"
              className="model-search-input"
              placeholder="Search models..."
              value={search}
              onChange={e => setSearch(e.target.value)}
              autoFocus
            />
          </div>

          {/* Model Options List */}
          <div className="model-options-list">
            {filtered.map(model => {
              const isSelected = model.id === selectedModel
              return (
                <div
                  key={model.id}
                  className={`model-option-card ${isSelected ? 'selected' : ''}`}
                  onClick={() => {
                    onSelectModel(model.id)
                  }}
                >
                  <div className="model-option-info">
                    <div className="model-option-title-row">
                      <span className="model-option-label">{model.label}</span>
                      <span className="model-option-provider">{model.provider}</span>
                      {model.badge && <span className="model-badge">{model.badge}</span>}
                    </div>
                    <div className="model-option-desc">{model.description}</div>
                  </div>
                  {isSelected && <Check size={14} className="check-icon" />}
                </div>
              )
            })}
            {filtered.length === 0 && (
              <div className="model-empty-msg">No models matching &ldquo;{search}&rdquo;</div>
            )}
          </div>

          {/* Reasoning Effort Row (EffortRow parity) */}
          <div className="effort-row-container">
            <div className="effort-row-label">
              <span>Thinking Budget</span>
              <span className="effort-help">Reasoning tokens</span>
            </div>
            <div className="effort-button-group">
              {EFFORT_LEVELS.map(item => (
                <button
                  key={item.level}
                  type="button"
                  className={`effort-pill ${effort === item.level ? 'active' : ''}`}
                  onClick={() => onSelectEffort(item.level)}
                  title={item.desc}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
