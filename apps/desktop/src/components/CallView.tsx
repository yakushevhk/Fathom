import { useState, useEffect, useRef } from 'react'
import { PhoneOff, Mic, MicOff, Volume2, Sparkles } from 'lucide-react'

interface CallViewProps {
  onEndCall: () => void
  onSendUtterance: (text: string) => void
  lastAssistantMessage?: string
  agentName?: string
}

export function CallView({
  onEndCall,
  onSendUtterance,
  lastAssistantMessage,
  agentName = 'Fathom Worker',
}: CallViewProps) {
  const [isMuted, setIsMuted] = useState(false)
  const [callDuration, setCallDuration] = useState(0)
  const [transcript, setTranscript] = useState<string>('')
  const [speakingAgent, setSpeakingAgent] = useState(false)
  const recognitionRef = useRef<unknown>(null)

  // Timer
  useEffect(() => {
    const timer = setInterval(() => setCallDuration(d => d + 1), 1000)
    return () => clearInterval(timer)
  }, [])

  // Web Speech Recognition
  useEffect(() => {
    if (typeof window !== 'undefined' && !isMuted) {
      interface SpeechWindow extends Window {
        SpeechRecognition?: new () => {
          continuous: boolean
          interimResults: boolean
          lang: string
          start: () => void
          stop: () => void
          onresult: (e: { resultIndex: number; results: Array<Array<{ transcript: string }> & { isFinal?: boolean }> }) => void
          onerror: () => void
          onend: () => void
        }
        webkitSpeechRecognition?: new () => {
          continuous: boolean
          interimResults: boolean
          lang: string
          start: () => void
          stop: () => void
          onresult: (e: { resultIndex: number; results: Array<Array<{ transcript: string }> & { isFinal?: boolean }> }) => void
          onerror: () => void
          onend: () => void
        }
      }
      const win = window as unknown as SpeechWindow
      const SpeechConstructor = win.SpeechRecognition || win.webkitSpeechRecognition
      if (SpeechConstructor) {
        const recognition = new SpeechConstructor()
        recognition.continuous = true
        recognition.interimResults = true
        recognition.lang = 'en-US'

        recognition.onresult = (e) => {
          for (let i = e.resultIndex; i < e.results.length; ++i) {
            const piece = e.results[i]?.[0]?.transcript || ''
            if (e.results[i]?.isFinal) {
              setTranscript(piece)
              onSendUtterance(piece)
            } else {
              setTranscript(piece)
            }
          }
        }

        try {
          recognition.start()
          recognitionRef.current = recognition
        } catch {
          // ignore
        }
      }
    }

    return () => {
      const rec = recognitionRef.current as { stop: () => void } | null
      rec?.stop()
    }
  }, [isMuted, onSendUtterance])

  // Speak assistant response
  useEffect(() => {
    if (!lastAssistantMessage || typeof window === 'undefined' || !('speechSynthesis' in window)) return
    window.speechSynthesis.cancel()
    const clean = lastAssistantMessage.replace(/```[\s\S]*?```/g, '').replace(/[#*_`]/g, '')
    const utterance = new SpeechSynthesisUtterance(clean.slice(0, 300))
    utterance.rate = 1.05
    utterance.onstart = () => setSpeakingAgent(true)
    utterance.onend = () => setSpeakingAgent(false)
    utterance.onerror = () => setSpeakingAgent(false)
    window.speechSynthesis.speak(utterance)
  }, [lastAssistantMessage])

  const formatTime = (secs: number) => {
    const m = Math.floor(secs / 60).toString().padStart(2, '0')
    const s = (secs % 60).toString().padStart(2, '0')
    return `${m}:${s}`
  }

  return (
    <div className="call-view-overlay">
      <div className="call-view-container">
        {/* Header */}
        <div className="call-header">
          <div className="call-agent-tag">
            <Sparkles size={14} className="call-sparkle" />
            <span>Active Voice Call</span>
          </div>
          <div className="call-timer">{formatTime(callDuration)}</div>
        </div>

        {/* Central visualizer avatar */}
        <div className="call-avatar-stage">
          <div className={`call-avatar-circle ${speakingAgent ? 'agent-speaking' : !isMuted ? 'user-listening' : ''}`}>
            <span className="call-avatar-initial">{agentName[0]}</span>
          </div>

          <div className="call-soundwave">
            <span className="bar bar-1" />
            <span className="bar bar-2" />
            <span className="bar bar-3" />
            <span className="bar bar-4" />
            <span className="bar bar-5" />
            <span className="bar bar-6" />
            <span className="bar bar-7" />
          </div>

          <div className="call-agent-name">{agentName}</div>
          <div className="call-status-label">
            {speakingAgent ? 'Agent speaking...' : isMuted ? 'Microphone muted' : 'Listening to you...'}
          </div>
        </div>

        {/* Real-time caption transcript */}
        <div className="call-captions">
          {transcript ? (
            <p className="caption-text">&ldquo;{transcript}&rdquo;</p>
          ) : (
            <p className="caption-placeholder">Speak naturally to direct your autonomous worker...</p>
          )}
        </div>

        {/* Controls */}
        <div className="call-controls">
          <button
            type="button"
            className={`call-control-btn ${isMuted ? 'muted' : ''}`}
            onClick={() => setIsMuted(!isMuted)}
            title={isMuted ? 'Unmute microphone' : 'Mute microphone'}
          >
            {isMuted ? <MicOff size={20} /> : <Mic size={20} />}
          </button>

          <button
            type="button"
            className="call-control-btn end-call"
            onClick={() => {
              if (typeof window !== 'undefined' && 'speechSynthesis' in window) {
                window.speechSynthesis.cancel()
              }
              onEndCall()
            }}
            title="End voice call"
          >
            <PhoneOff size={22} />
          </button>

          <button
            type="button"
            className="call-control-btn"
            onClick={() => {
              if (lastAssistantMessage && 'speechSynthesis' in window) {
                window.speechSynthesis.cancel()
                const u = new SpeechSynthesisUtterance(lastAssistantMessage.slice(0, 200))
                window.speechSynthesis.speak(u)
              }
            }}
            title="Repeat last response"
          >
            <Volume2 size={20} />
          </button>
        </div>
      </div>
    </div>
  )
}
