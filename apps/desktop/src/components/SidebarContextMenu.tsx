import { Pin, Download, Copy, Trash2, Edit2 } from 'lucide-react'
import type { SessionSummary } from '../lib/api'

export interface SidebarContextMenuProps {
  x: number
  y: number
  session: SessionSummary
  isPinned: boolean
  onPin: (id: string) => void
  onRename: (id: string) => void
  onCopyId: (id: string) => void
  onExport: (id: string) => void
  onDelete: (id: string) => void
  onClose: () => void
}

export function SidebarContextMenu({
  x,
  y,
  session,
  isPinned,
  onPin,
  onRename,
  onCopyId,
  onExport,
  onDelete,
  onClose,
}: SidebarContextMenuProps) {
  return (
    <div
      className="sidebar-context-backdrop"
      onClick={onClose}
      onContextMenu={e => {
        e.preventDefault()
        onClose()
      }}
    >
      <div
        className="sidebar-context-menu"
        style={{ top: Math.min(y, window.innerHeight - 200), left: Math.min(x, window.innerWidth - 220) }}
        onClick={e => e.stopPropagation()}
      >
        <button
          className="context-item"
          onClick={() => {
            onPin(session.id)
            onClose()
          }}
        >
          <Pin size={13} />
          <span>{isPinned ? 'Unpin from Top' : 'Pin to Top'}</span>
        </button>

        <button
          className="context-item"
          onClick={() => {
            onRename(session.id)
            onClose()
          }}
        >
          <Edit2 size={13} />
          <span>Rename Session</span>
        </button>

        <button
          className="context-item"
          onClick={() => {
            onCopyId(session.id)
            onClose()
          }}
        >
          <Copy size={13} />
          <span>Copy Session ID</span>
        </button>

        <button
          className="context-item"
          onClick={() => {
            onExport(session.id)
            onClose()
          }}
        >
          <Download size={13} />
          <span>Export Transcript</span>
        </button>

        <div className="context-divider" />

        <button
          className="context-item danger"
          onClick={() => {
            onDelete(session.id)
            onClose()
          }}
        >
          <Trash2 size={13} />
          <span>Cancel & Delete</span>
        </button>
      </div>
    </div>
  )
}
