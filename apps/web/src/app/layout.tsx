import type { Metadata } from 'next'
import './globals.css'
import { SessionsProvider } from '@/hooks/useSessions'
import { Sidebar } from '@/components/Sidebar'

export const metadata: Metadata = {
  title: 'Fathom Dashboard',
  description: 'Control your Fathom research agent',
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body className="h-screen overflow-hidden">
        <SessionsProvider>
          <div className="flex h-full">
            <Sidebar />
            <main className="flex-1 flex flex-col min-w-0">{children}</main>
          </div>
        </SessionsProvider>
      </body>
    </html>
  )
}