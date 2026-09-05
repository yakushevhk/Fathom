import Link from 'next/link'

export default function NotFound() {
  return (
    <div className="flex flex-col items-center justify-center min-h-[60vh] p-6 text-center">
      <div className="text-4xl font-mono text-gray-500 mb-2">404</div>
      <h2 className="text-base font-medium text-white mb-2">Page Not Found</h2>
      <p className="text-xs text-gray-400 max-w-sm mb-6">
        The requested resource or session view could not be located.
      </p>
      <Link
        href="/"
        className="px-4 py-2 bg-white/10 hover:bg-white/15 text-xs text-white rounded font-medium transition-colors border border-white/10"
      >
        Return to Overview
      </Link>
    </div>
  )
}
