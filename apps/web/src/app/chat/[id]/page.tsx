'use client'

import { use, useState } from 'react'
import { ChatView } from '@/components/ChatView'

function getBase(): string {
  if (typeof window !== 'undefined') {
    return localStorage.getItem('fathom_base_url') || 'http://127.0.0.1:8080'
  }
  return 'http://127.0.0.1:8080'
}

export default function ChatPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params)
  const [baseUrl] = useState(getBase)

  return <ChatView key={id} sessionId={id} baseUrl={baseUrl} />
}