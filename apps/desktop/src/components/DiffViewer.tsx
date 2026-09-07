import { useMemo } from 'react'

interface DiffViewerProps {
  original?: string
  modified?: string
  filePath?: string
}

interface DiffLine {
  type: 'add' | 'remove' | 'same'
  text: string
  origNum?: number
  modNum?: number
}

export function DiffViewer({ original = '', modified = '', filePath }: DiffViewerProps) {
  const lines: DiffLine[] = useMemo(() => {
    const origLines = original ? original.split('\n') : []
    const modLines = modified ? modified.split('\n') : []

    // Fast path: pure addition or pure deletion
    if (!origLines.length) {
      return modLines.map((text, idx) => ({ type: 'add', text, modNum: idx + 1 }))
    }
    if (!modLines.length) {
      return origLines.map((text, idx) => ({ type: 'remove', text, origNum: idx + 1 }))
    }

    // Bounded line diff (prevent freeze on huge files > 10,000 lines)
    if (origLines.length * modLines.length > 200_000) {
      return [
        ...origLines.map((text, idx) => ({ type: 'remove' as const, text, origNum: idx + 1 })),
        ...modLines.map((text, idx) => ({ type: 'add' as const, text, modNum: idx + 1 })),
      ]
    }

    const diff: DiffLine[] = []
    let i = 0
    let j = 0

    while (i < origLines.length && j < modLines.length) {
      if (origLines[i] === modLines[j]) {
        diff.push({ type: 'same', text: origLines[i], origNum: i + 1, modNum: j + 1 })
        i++
        j++
      } else {
        // Look ahead for match
        let foundMatch = false
        for (let k = 1; k < 5; k++) {
          if (i + k < origLines.length && origLines[i + k] === modLines[j]) {
            for (let m = 0; m < k; m++) {
              diff.push({ type: 'remove', text: origLines[i + m], origNum: i + m + 1 })
            }
            i += k
            foundMatch = true
            break
          } else if (j + k < modLines.length && origLines[i] === modLines[j + k]) {
            for (let m = 0; m < k; m++) {
              diff.push({ type: 'add', text: modLines[j + m], modNum: j + m + 1 })
            }
            j += k
            foundMatch = true
            break
          }
        }
        if (!foundMatch) {
          diff.push({ type: 'remove', text: origLines[i], origNum: i + 1 })
          diff.push({ type: 'add', text: modLines[j], modNum: j + 1 })
          i++
          j++
        }
      }
    }

    while (i < origLines.length) {
      diff.push({ type: 'remove', text: origLines[i], origNum: i + 1 })
      i++
    }
    while (j < modLines.length) {
      diff.push({ type: 'add', text: modLines[j], modNum: j + 1 })
      j++
    }

    return diff
  }, [original, modified])

  return (
    <div className="diff-viewer">
      {filePath && (
        <div className="diff-viewer-header">
          <span className="diff-file-path">{filePath}</span>
        </div>
      )}
      <div className="diff-viewer-body">
        {lines.map((line, idx) => (
          <div key={idx} className={`diff-row ${line.type}`}>
            <span className="diff-line-no orig">{line.origNum || ''}</span>
            <span className="diff-line-no mod">{line.modNum || ''}</span>
            <span className="diff-marker">
              {line.type === 'add' ? '+' : line.type === 'remove' ? '-' : ' '}
            </span>
            <span className="diff-code">{line.text}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
