import { CornerDownRight, Trash2, Zap } from 'lucide-react'

export interface QueuedMessage {
  id: string
  text: string
  timestamp: Date
}

interface QueuedComposerMessagesProps {
  items: QueuedMessage[]
  onSteerNow: (text: string) => void
  onSteerAll: () => void
  onRemove: (id: string) => void
  onClear: () => void
  isRunning: boolean
}

export function QueuedComposerMessages({
  items,
  onSteerNow,
  onSteerAll,
  onRemove,
  onClear,
  isRunning,
}: QueuedComposerMessagesProps) {
  if (!items.length) return null

  return (
    <div className="queued-messages-strip">
      <div className="queued-messages-header">
        <div className="queued-header-left">
          <span className="queued-pulse-dot" />
          <span className="queued-title">
            Held in Queue ({items.length})
          </span>
          <span className="queued-hint">
            {isRunning ? 'Will execute when current turn settles' : 'Ready to dispatch'}
          </span>
        </div>
        <div className="queued-header-actions">
          {isRunning && items.length > 1 && (
            <button className="queued-action-btn primary" onClick={onSteerAll} title="Steer running agent with all queued items combined">
              <Zap size={13} style={{ marginRight: 4 }} />
              Steer All Now
            </button>
          )}
          <button className="queued-action-btn" onClick={onClear} title="Discard all held items">
            Clear Queue
          </button>
        </div>
      </div>

      <div className="queued-messages-list">
        {items.map((item, index) => (
          <div key={item.id} className="queued-item">
            <span className="queued-index">#{index + 1}</span>
            <div className="queued-item-text" title={item.text}>
              {item.text}
            </div>
            <div className="queued-item-actions">
              {isRunning && (
                <button
                  className="queued-item-btn steer"
                  onClick={() => onSteerNow(item.text)}
                  title="Interrupt and steer agent with this instruction now"
                >
                  <CornerDownRight size={12} style={{ marginRight: 3 }} />
                  Steer
                </button>
              )}
              <button
                className="queued-item-btn remove"
                onClick={() => onRemove(item.id)}
                title="Remove this message"
              >
                <Trash2 size={12} />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
