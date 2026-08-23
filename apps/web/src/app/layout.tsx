import type { Metadata } from 'next'
import './globals.css'
import { SessionsProvider } from '@/hooks/useSessions'
import { Sidebar } from '@/components/Sidebar'

export const metadata: Metadata = {
  title: 'Fathom | Autonomous Worker Control Plane',
  description: 'Operate self-hosted autonomous remote workers across sessions, jobs, memory, governance, computers, and live events.',
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body className="ops-app h-screen overflow-hidden">
        <a className="skip-link" href="#main-content">Skip to main content</a>
        <SessionsProvider>
          <div className="ops-shell flex h-full min-h-0">
            <Sidebar />
            <main id="main-content" tabIndex={-1} className="ops-main flex min-w-0 flex-1 flex-col overflow-hidden">{children}</main>
          </div>
        </SessionsProvider>
      </body>
    </html>
  )
}