import { FileText, Image as ImageIcon, X } from 'lucide-react'

export interface ComposerAttachment {
  id: string
  name: string
  type: string
  size: number
  dataUrl?: string
}

interface ComposerAttachmentsProps {
  attachments: ComposerAttachment[]
  onRemove: (id: string) => void
}

export function ComposerAttachments({ attachments, onRemove }: ComposerAttachmentsProps) {
  if (!attachments.length) return null

  return (
    <div className="composer-attachments-row">
      {attachments.map(att => {
        const isImage = att.type.startsWith('image/')
        return (
          <div key={att.id} className="attachment-chip" title={`${att.name} (${Math.round(att.size / 1024)} KB)`}>
            {isImage && att.dataUrl ? (
              <img src={att.dataUrl} alt={att.name} className="attachment-thumbnail" />
            ) : (
              <div className="attachment-icon">
                {isImage ? <ImageIcon size={14} /> : <FileText size={14} />}
              </div>
            )}
            <span className="attachment-name">{att.name}</span>
            <button
              type="button"
              className="attachment-remove-btn"
              onClick={() => onRemove(att.id)}
              title="Remove attachment"
            >
              <X size={12} />
            </button>
          </div>
        )
      })}
    </div>
  )
}
