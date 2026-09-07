import { useState, useEffect } from 'react'
import { Volume2, VolumeX } from 'lucide-react'

interface SpeakButtonProps {
  text: string
  className?: string
}

export function SpeakButton({ text, className = '' }: SpeakButtonProps) {
  const [speaking, setSpeaking] = useState(false)

  // Clean speakable text: strip code blocks, markdown tags, and URLs
  const cleanText = (raw: string): string => {
    return raw
      .replace(/```[\s\S]*?```/g, ' [code block elided] ')
      .replace(/`[^`]+`/g, ' ')
      .replace(/https?:\/\/\S+/g, ' ')
      .replace(/[#*_~\[\]()]/g, ' ')
      .trim()
  }

  const handleToggleSpeak = () => {
    if (typeof window === 'undefined' || !('speechSynthesis' in window)) return

    if (speaking) {
      window.speechSynthesis.cancel()
      setSpeaking(false)
      return
    }

    window.speechSynthesis.cancel() // cancel any existing utterance
    const speech = new SpeechSynthesisUtterance(cleanText(text))
    speech.rate = 1.05
    speech.pitch = 1.0

    speech.onend = () => setSpeaking(false)
    speech.onerror = () => setSpeaking(false)

    setSpeaking(true)
    window.speechSynthesis.speak(speech)
  }

  useEffect(() => {
    return () => {
      if (typeof window !== 'undefined' && 'speechSynthesis' in window) {
        window.speechSynthesis.cancel()
      }
    }
  }, [])

  return (
    <button
      type="button"
      className={`speak-button ${speaking ? 'speaking' : ''} ${className}`}
      onClick={handleToggleSpeak}
      title={speaking ? 'Stop speaking' : 'Read aloud with Text-to-Speech'}
    >
      {speaking ? <VolumeX size={13} /> : <Volume2 size={13} />}
    </button>
  )
}
