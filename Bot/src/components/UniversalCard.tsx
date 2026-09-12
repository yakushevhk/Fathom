import React from "react";
import {
  Activity,
  AlertCircle,
  BarChart3,
  Calendar,
  CheckCircle2,
  Clock,
  Flame,
  Globe,
  Heart,
  Info,
  Layers,
  Sparkles,
  TrendingDown,
  TrendingUp,
  Zap,
} from "lucide-react";
import { cn } from "@/lib/cn";

export type CardLayout = "stats" | "list" | "key-value" | "progress" | "timeline";

export interface MetricStat {
  label: string;
  value: string | number;
  unit?: string;
  change?: string;
  trend?: "up" | "down" | "neutral";
  status?: "normal" | "success" | "warning" | "danger" | "info";
  description?: string;
}

export interface ListItem {
  title: string;
  subtitle?: string;
  badge?: string;
  icon?: string;
  value?: string | number;
  status?: "normal" | "success" | "warning" | "danger" | "info";
  actionUrl?: string;
}

export interface ProgressItem {
  label: string;
  current: number;
  total: number;
  unit?: string;
  color?: "blue" | "green" | "amber" | "rose" | "purple";
}

export interface KeyValueItem {
  key: string;
  value: string | number;
  hint?: string;
}

export interface TimelineItem {
  time: string;
  title: string;
  description?: string;
  status?: "done" | "current" | "pending";
}

export interface UniversalCardData {
  type?: "card";
  layout?: CardLayout;
  title: string;
  subtitle?: string;
  badge?: string;
  accent?: "blue" | "green" | "emerald" | "amber" | "rose" | "purple" | "neutral";
  icon?: string;
  stats?: MetricStat[];
  items?: ListItem[];
  progress?: ProgressItem[];
  keyValue?: KeyValueItem[];
  timeline?: TimelineItem[];
  footer?: string;
}

const ACCENT_MAP: Record<string, { border: string; badge: string; iconBg: string; text: string; fill: string }> = {
  blue: {
    border: "border-blue-500/30",
    badge: "bg-blue-500/15 text-blue-300 border-blue-500/20",
    iconBg: "bg-blue-500/15 text-blue-400",
    text: "text-blue-400",
    fill: "bg-blue-500",
  },
  green: {
    border: "border-emerald-500/30",
    badge: "bg-emerald-500/15 text-emerald-300 border-emerald-500/20",
    iconBg: "bg-emerald-500/15 text-emerald-400",
    text: "text-emerald-400",
    fill: "bg-emerald-500",
  },
  emerald: {
    border: "border-emerald-500/30",
    badge: "bg-emerald-500/15 text-emerald-300 border-emerald-500/20",
    iconBg: "bg-emerald-500/15 text-emerald-400",
    text: "text-emerald-400",
    fill: "bg-emerald-500",
  },
  amber: {
    border: "border-amber-500/30",
    badge: "bg-amber-500/15 text-amber-300 border-amber-500/20",
    iconBg: "bg-amber-500/15 text-amber-400",
    text: "text-amber-400",
    fill: "bg-amber-500",
  },
  rose: {
    border: "border-rose-500/30",
    badge: "bg-rose-500/15 text-rose-300 border-rose-500/20",
    iconBg: "bg-rose-500/15 text-rose-400",
    text: "text-rose-400",
    fill: "bg-rose-500",
  },
  purple: {
    border: "border-purple-500/30",
    badge: "bg-purple-500/15 text-purple-300 border-purple-500/20",
    iconBg: "bg-purple-500/15 text-purple-400",
    text: "text-purple-400",
    fill: "bg-purple-500",
  },
  neutral: {
    border: "border-hairline/60",
    badge: "bg-raised text-ink-secondary border-hairline/40",
    iconBg: "bg-raised text-ink-secondary",
    text: "text-ink",
    fill: "bg-ink-secondary",
  },
};

function resolveIcon(name?: string): React.ElementType {
  if (!name) return Sparkles;
  switch (name.toLowerCase()) {
    case "heart": return Heart;
    case "activity":
    case "pulse": return Activity;
    case "zap":
    case "flash": return Zap;
    case "flame":
    case "fire": return Flame;
    case "chart":
    case "analytics": return BarChart3;
    case "calendar": return Calendar;
    case "clock":
    case "time": return Clock;
    case "globe":
    case "web": return Globe;
    case "layers": return Layers;
    case "check": return CheckCircle2;
    case "alert": return AlertCircle;
    default: return Sparkles;
  }
}

export function UniversalCard({ card }: { card: UniversalCardData }) {
  const accentKey = card.accent || "blue";
  const colors = ACCENT_MAP[accentKey] || ACCENT_MAP.blue;
  const HeaderIcon = resolveIcon(card.icon);
  const layout = card.layout || (card.stats ? "stats" : card.progress ? "progress" : card.keyValue ? "key-value" : card.timeline ? "timeline" : "list");

  return (
    <div className={cn("my-3 w-full max-w-2xl overflow-hidden rounded-2xl border bg-panel/85 shadow-sm backdrop-blur-md transition-all hover:shadow-md", colors.border)}>
      {/* Header */}
      <div className="flex items-center justify-between border-b border-hairline/40 px-4 py-3 bg-raised/20">
        <div className="flex items-center gap-2.5 min-w-0">
          <div className={cn("flex size-8 shrink-0 items-center justify-center rounded-xl", colors.iconBg)}>
            <HeaderIcon size={16} />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h3 className="text-[13.5px] font-semibold text-ink truncate">{card.title}</h3>
              {card.badge && (
                <span className={cn("rounded-full px-2 py-0.5 text-[10.5px] font-medium border", colors.badge)}>
                  {card.badge}
                </span>
              )}
            </div>
            {card.subtitle && (
              <p className="text-[11.5px] text-ink-secondary truncate">{card.subtitle}</p>
            )}
          </div>
        </div>
      </div>

      {/* Body Layouts */}
      <div className="p-4">
        {layout === "stats" && card.stats && (
          <div className={cn("grid gap-2.5", card.stats.length > 2 ? "grid-cols-2 sm:grid-cols-3" : card.stats.length === 2 ? "grid-cols-2" : "grid-cols-1")}>
            {card.stats.map((stat, idx) => (
              <div key={idx} className="flex flex-col justify-between rounded-xl border border-hairline/40 bg-inset/40 p-3 transition-colors hover:bg-inset/70">
                <span className="text-[11px] font-medium text-ink-secondary truncate" title={stat.label}>
                  {stat.label}
                </span>
                <div className="mt-1 flex items-baseline gap-1.5 flex-wrap">
                  <span className="text-lg font-semibold tracking-tight text-ink font-mono">
                    {stat.value}
                  </span>
                  {stat.unit && <span className="text-[11px] text-ink-secondary font-medium">{stat.unit}</span>}
                </div>
                {(stat.change || stat.description) && (
                  <div className="mt-1.5 flex items-center gap-1 text-[10.5px] text-ink-secondary truncate">
                    {stat.trend === "up" && <TrendingUp size={12} className="text-emerald-400 shrink-0" />}
                    {stat.trend === "down" && <TrendingDown size={12} className="text-rose-400 shrink-0" />}
                    <span className="truncate">{stat.change || stat.description}</span>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {layout === "progress" && card.progress && (
          <div className="space-y-3">
            {card.progress.map((prog, idx) => {
              const percent = Math.min(100, Math.max(0, Math.round((prog.current / (prog.total || 1)) * 100)));
              const barColor = prog.color ? (ACCENT_MAP[prog.color] || colors).fill : colors.fill;
              return (
                <div key={idx} className="rounded-xl border border-hairline/40 bg-inset/40 p-3">
                  <div className="flex items-center justify-between text-xs font-medium text-ink mb-1.5">
                    <span className="truncate">{prog.label}</span>
                    <span className="font-mono text-ink-secondary">
                      {prog.current} / {prog.total} {prog.unit || ""} ({percent}%)
                    </span>
                  </div>
                  <div className="h-2 w-full overflow-hidden rounded-full bg-raised/80">
                    <div className={cn("h-full transition-all duration-500 rounded-full", barColor)} style={{ width: `${percent}%` }} />
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {layout === "key-value" && card.keyValue && (
          <div className="divide-y divide-hairline/30 rounded-xl border border-hairline/40 bg-inset/30 overflow-hidden">
            {card.keyValue.map((kv, idx) => (
              <div key={idx} className="flex items-center justify-between px-3.5 py-2.5 text-xs hover:bg-raised/30 transition-colors">
                <span className="font-medium text-ink-secondary">{kv.key}</span>
                <div className="text-right">
                  <span className="font-semibold text-ink font-mono">{kv.value}</span>
                  {kv.hint && <p className="text-[10px] text-ink-secondary">{kv.hint}</p>}
                </div>
              </div>
            ))}
          </div>
        )}

        {layout === "timeline" && card.timeline && (
          <div className="relative pl-5 space-y-4 before:absolute before:left-2 before:top-2 before:bottom-2 before:w-0.5 before:bg-hairline/60">
            {card.timeline.map((tItem, idx) => (
              <div key={idx} className="relative">
                <span className={cn(
                  "absolute -left-5 top-1 size-2.5 rounded-full ring-4 ring-panel",
                  tItem.status === "done" ? "bg-emerald-400" : tItem.status === "current" ? "bg-accent animate-pulse" : "bg-hairline"
                )} />
                <div className="flex items-baseline gap-2">
                  <span className="text-[11px] font-mono font-medium text-accent">{tItem.time}</span>
                  <span className="text-[12.5px] font-medium text-ink">{tItem.title}</span>
                </div>
                {tItem.description && (
                  <p className="mt-0.5 text-[11.5px] text-ink-secondary">{tItem.description}</p>
                )}
              </div>
            ))}
          </div>
        )}

        {layout === "list" && card.items && (
          <div className="space-y-1.5">
            {card.items.map((item, idx) => {
              const ItemIcon = resolveIcon(item.icon);
              return (
                <div
                  key={idx}
                  className="flex items-center justify-between gap-3 rounded-xl border border-hairline/40 bg-inset/30 px-3.5 py-2.5 hover:bg-inset/60 transition-colors"
                >
                  <div className="flex items-center gap-2.5 min-w-0">
                    <div className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-raised text-ink-secondary">
                      <ItemIcon size={14} />
                    </div>
                    <div className="min-w-0">
                      <div className="flex items-center gap-1.5">
                        <span className="text-[12.5px] font-medium text-ink truncate">{item.title}</span>
                        {item.badge && (
                          <span className="rounded-full bg-raised px-1.5 py-0.2 text-[9.5px] text-ink-secondary border border-hairline/40">
                            {item.badge}
                          </span>
                        )}
                      </div>
                      {item.subtitle && <p className="text-[11px] text-ink-secondary truncate">{item.subtitle}</p>}
                    </div>
                  </div>
                  {item.value !== undefined && (
                    <span className="font-mono text-[12.5px] font-semibold text-ink shrink-0">
                      {item.value}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Footer */}
      {card.footer && (
        <div className="border-t border-hairline/30 bg-raised/20 px-4 py-2 text-[11.5px] text-ink-secondary flex items-center gap-1.5">
          <Info size={13} className="shrink-0 text-ink-secondary/70" />
          <span className="truncate">{card.footer}</span>
        </div>
      )}
    </div>
  );
}

export function parseUniversalCardJson(raw: string): UniversalCardData | null {
  try {
    const trimmed = raw.trim();
    if (!trimmed.startsWith("{") || !trimmed.endsWith("}")) return null;
    const parsed = JSON.parse(trimmed);
    if (parsed && typeof parsed === "object" && typeof parsed.title === "string") {
      return parsed as UniversalCardData;
    }
  } catch {
    // Ignore invalid JSON
  }
  return null;
}
