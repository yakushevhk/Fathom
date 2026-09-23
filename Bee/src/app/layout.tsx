import type { Metadata } from 'next'
import { AppShell } from '@/components/AppShell'
import { HiveProvider } from '@/lib/store'
import './globals.css'

export const metadata: Metadata = {
  title: 'Fathom — Hive',
  description: 'A workspace where humans and agents build together, on a relay you own.',
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" data-theme="dark">
      <body>
        <script
          dangerouslySetInnerHTML={{
            __html: `try{var t=localStorage.getItem('bee-theme');if(t)document.documentElement.dataset.theme=t}catch(e){}`,
          }}
        />
        <HiveProvider>
          <AppShell>{children}</AppShell>
        </HiveProvider>
      </body>
    </html>
  )
}
