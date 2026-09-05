'use client'

import { useEffect } from 'react'

export default function GlobalError({
  error,
  reset,
}: {
  error: Error & { digest?: string }
  reset: () => void
}) {
  useEffect(() => {
    console.error('Unhandled app error:', error)
  }, [error])

  return (
    <div className="flex flex-col items-center justify-center min-h-[60vh] p-6 text-center">
      <div className="w-12 h-12 rounded-full bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-400 mb-4 text-xl font-mono">
        !
      </div>
      <h2 className="text-lg font-medium text-white mb-2">Something went wrong</h2>
      <p className="text-xs text-gray-400 max-w-md mb-6">
        {error.message || 'An unexpected client error occurred while loading this view.'}
      </p>
      <button
        type="button"
        onClick={() => reset()}
        className="px-4 py-2 bg-white/10 hover:bg-white/15 text-xs text-white rounded font-medium transition-colors border border-white/10"
      >
        Try again
      </button>
    </div>
  )
}
