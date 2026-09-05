'use client'

import { SettingsPanel } from '@/components/SettingsPanel'

export default function SettingsPage() {
  return (
    <div className="flex-1 overflow-y-auto p-6 max-w-4xl mx-auto">
      <div className="mb-6">
        <h1 className="text-xl font-semibold text-white tracking-tight">Settings</h1>
        <p className="text-xs text-gray-400 mt-1">
          Configure API endpoints, authentication credentials, and client runtime options.
        </p>
      </div>
      <SettingsPanel />
    </div>
  )
}
