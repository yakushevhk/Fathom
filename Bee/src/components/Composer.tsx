'use client'

import { useRef, useState } from 'react'
import { SendHorizonal } from 'lucide-react'
import type { Channel } from '@/lib/types'

export function Composer({ channel, onSent }: { channel: Channel; onSent: () => void }) {
  const [text, setText] = useState('')
  const [busy, setBusy] = useState(false)
  const ta = useRef<HTMLTextAreaElement>(null)

  const send = async () => {
    const body = text.trim()
    if (!body || busy) return
    setBusy(true)
    setText('')
    const r = await fetch(`/api/channels/${channel.id}/messages`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ body }),
    })
    setBusy(false)
    if (!r.ok) {
      setText(body)
      return
    }
    onSent()
    ta.current?.focus()
  }

  const readonly = channel.kind === 'announcement'

  return (
    <div className="border-t border-line bg-panel/60 px-4 py-3">
      {readonly ? (
        <div className="card flex items-center gap-2 px-3 py-2.5 text-[12px] text-dim">
          <SendHorizonal size={13} /> Broadcast room — only coordinators can post.
        </div>
      ) : (
        <div className="card flex items-end gap-2 px-3 py-2 focus-within:border-acc">
          <textarea
            ref={ta}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                send()
              }
            }}
            rows={1}
            placeholder={`Message ${channel.kind === 'dm' ? channel.name : `#${channel.slug}`} — agents in this room hear you`}
            className="max-h-40 min-h-[22px] flex-1 resize-none bg-transparent text-[13.5px] outline-none placeholder:text-dim"
          />
          <button
            onClick={send}
            disabled={busy || !text.trim()}
            className="btn btn-primary !rounded-lg !p-1.5"
            title="Send (Enter)"
          >
            <SendHorizonal size={15} />
          </button>
        </div>
      )}
      <div className="mt-1 flex items-center justify-between px-1 text-[9.5px] text-dim">
        <span>Enter to send · Shift+Enter for newline · mention @atlas @forge @sonar @beacon @quill @ping</span>
        <span className="mono">signed with your key</span>
      </div>
    </div>
  )
}
