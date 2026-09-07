import { useState, useEffect } from 'react'
import { FileText, Download, Copy, Check, ExternalLink } from 'lucide-react'
import { api, type SessionSummary } from '../lib/api'

interface ArtifactItem {
  id: string
  name: string
  path: string
  size?: number
  content?: string
  kind: 'markdown' | 'code' | 'json' | 'data'
}

interface ArtifactsDrawerProps {
  session: SessionSummary | null
}

export function ArtifactsDrawer({ session }: ArtifactsDrawerProps) {
  const [artifacts, setArtifacts] = useState<ArtifactItem[]>([])
  const [selectedArtifact, setSelectedArtifact] = useState<ArtifactItem | null>(null)
  const [copied, setCopied] = useState(false)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (!session) {
      setArtifacts([])
      setSelectedArtifact(null)
      return
    }

    setLoading(true)
    api.sessions.results(session.id)
      .then(res => {
        const items: ArtifactItem[] = []
        if (res?.summary) {
          items.push({
            id: 'summary',
            name: 'summary.md',
            path: `${session.output_dir || 'output'}/summary.md`,
            content: res.summary,
            kind: 'markdown',
          })
        }
        if (Array.isArray(res?.findings)) {
          res.findings.forEach((f, idx) => {
            items.push({
              id: `finding-${idx}`,
              name: f.file || `finding-${idx + 1}.md`,
              path: `${session.output_dir || 'output'}/findings/${f.file || `finding-${idx + 1}.md`}`,
              content: f.content,
              kind: 'markdown',
            })
          })
        }
        setArtifacts(items)
        setSelectedArtifact(items[0] || null)
      })
      .catch(() => {
        setArtifacts([])
      })
      .finally(() => setLoading(false))
  }, [session?.id])

  const handleCopy = () => {
    if (selectedArtifact?.content) {
      navigator.clipboard.writeText(selectedArtifact.content)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  const handleDownload = () => {
    if (selectedArtifact?.content) {
      const blob = new Blob([selectedArtifact.content], { type: 'text/markdown' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = selectedArtifact.name
      a.click()
      URL.revokeObjectURL(url)
    }
  }

  if (!session) {
    return <div className="artifacts-empty">Select an active or completed session to view artifacts.</div>
  }

  return (
    <div className="artifacts-drawer">
      <div className="artifacts-sidebar">
        <div className="artifacts-header">
          <FileText size={13} className="artifacts-icon" />
          <span>Artifacts ({artifacts.length})</span>
        </div>

        <div className="artifacts-list">
          {loading ? (
            <div className="artifacts-loading">Loading deliverables...</div>
          ) : artifacts.length === 0 ? (
            <div className="artifacts-empty-msg">No deliverables generated yet.</div>
          ) : (
            artifacts.map(item => (
              <div
                key={item.id}
                className={`artifact-list-item ${selectedArtifact?.id === item.id ? 'active' : ''}`}
                onClick={() => setSelectedArtifact(item)}
              >
                <FileText size={12} />
                <span className="artifact-item-name">{item.name}</span>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="artifacts-preview">
        {selectedArtifact ? (
          <>
            <div className="artifacts-preview-toolbar">
              <div className="artifact-file-path" title={selectedArtifact.path}>
                {selectedArtifact.path}
              </div>
              <div className="artifact-preview-actions">
                <button
                  className="artifact-btn"
                  onClick={handleCopy}
                  title="Copy to clipboard"
                >
                  {copied ? <Check size={12} color="var(--success)" /> : <Copy size={12} />}
                  <span>{copied ? 'Copied' : 'Copy'}</span>
                </button>
                <button
                  className="artifact-btn primary"
                  onClick={handleDownload}
                  title="Download artifact file"
                >
                  <Download size={12} />
                  <span>Export</span>
                </button>
              </div>
            </div>
            <div className="artifact-preview-body">
              <pre><code>{selectedArtifact.content || '(empty file)'}</code></pre>
            </div>
          </>
        ) : (
          <div className="artifact-no-selection">
            <p>Select a file to inspect its deliverables.</p>
          </div>
        )}
      </div>
    </div>
  )
}
