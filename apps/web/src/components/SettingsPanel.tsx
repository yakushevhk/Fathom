'use client'

import { useState } from 'react'

export function SettingsPanel() {
  const [url, setUrl] = useState(() =>
    typeof window !== 'undefined' ? localStorage.getItem('fathom_base_url') ?? 'http://127.0.0.1:8080' : 'http://127.0.0.1:8080'
  )
  const [key, setKey] = useState(() =>
    typeof window !== 'undefined' ? localStorage.getItem('fathom_api_key') ?? '' : ''
  )
  const [saved, setSaved] = useState(false)

  const save = () => {
    localStorage.setItem('fathom_base_url', url)
    if (key) localStorage.setItem('fathom_api_key', key)
    else localStorage.removeItem('fathom_api_key')
    setSaved(true)
    setTimeout(() => setSaved(false), 2000)
  }

  return (
    <div className="p-6 space-y-4">
      <h2 className="text-sm font-semibold text-gray-300">Fathom connection settings</h2>

      <div className="space-y-1">
        <label className="text-xs text-gray-500">Fathom worker runtime URL</label>
        <input value={url} onChange={e => setUrl(e.target.value)}
          className="w-full p-2 rounded-md bg-[#141414] border border-white/[0.06] text-sm text-gray-200 outline-none focus:border-gray-500" />
      </div>

      <div className="space-y-1">
        <label className="text-xs text-gray-500">API Key (optional)</label>
        <input value={key} onChange={e => setKey(e.target.value)} type="password"
          className="w-full p-2 rounded-md bg-[#141414] border border-white/[0.06] text-sm text-gray-200 outline-none focus:border-gray-500" />
      </div>

      <button onClick={save}
        className="px-4 py-2 rounded-md bg-gray-600 text-black text-xs font-semibold hover:bg-gray-400 transition-colors">
        {saved ? 'Saved ✓' : 'Save'}
      </button>
    </div>
  )
}