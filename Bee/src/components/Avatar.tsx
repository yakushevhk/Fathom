import { cn } from '@/lib/cn'
import { initials } from '@/lib/format'
import type { Member } from '@/lib/types'

const PRESENCE_COLOR: Record<string, string> = {
  online: 'var(--ok)',
  busy: 'var(--danger)',
  away: 'var(--gold)',
  offline: 'var(--dim)',
}

// Agents wear a hexagon (hive cell); humans wear a circle.
export function Avatar({ member, size = 30, className }: { member: Member; size?: number; className?: string }) {
  const isAgent = member.kind === 'agent'
  return (
    <span className={cn('relative inline-flex shrink-0', className)} style={{ width: size, height: size }}>
      <span
        className={cn('flex items-center justify-center font-bold', isAgent ? 'hex' : 'rounded-full')}
        style={{
          width: size,
          height: size,
          fontSize: size * 0.38,
          color: '#04231f',
          background: `linear-gradient(135deg, ${member.accent}, ${member.accent}cc)`,
        }}
        title={member.displayName}
      >
        {initials(member.displayName)}
      </span>
      <span
        className="absolute rounded-full border"
        style={{
          width: Math.max(8, size * 0.3),
          height: Math.max(8, size * 0.3),
          right: -1,
          bottom: -1,
          background: PRESENCE_COLOR[member.presence] ?? PRESENCE_COLOR.offline,
          borderColor: 'var(--bg)',
          borderWidth: 2,
        }}
      />
    </span>
  )
}
