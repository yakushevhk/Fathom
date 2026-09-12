import React, { useState } from "react";
import {
  Activity,
  AlertCircle,
  BarChart3,
  Calendar,
  CheckCircle2,
  Clock,
  ExternalLink,
  Flame,
  Globe,
  Heart,
  Info,
  Layers,
  Pin,
  Send,
  Sparkles,
  TrendingDown,
  TrendingUp,
  Zap,
} from "lucide-react";
import { BorderBeam } from "border-beam";
import { cn } from "@/lib/cn";
import { useStore } from "@/state/store";
import { triggerHaptic } from "@/lib/haptics";

export type CardLayout = "stats" | "list" | "key-value" | "progress" | "timeline";

export interface MetricStat {
  label: string;
  value: string | number;
  unit?: string;
  change?: string;
  trend?: "up" | "down" | "neutral";
  status?: "normal" | "success" | "warning" | "danger" | "info";
  description?: string;
  /** Numeric trend points rendered as a sleek micro SVG curve */
  sparkline?: number[];
  /** Min-max bounded gauge with current point marker */
  range?: {
    min: number;
    max: number;
    current: number;
    optimalMin?: number;
    optimalMax?: number;
  };
  /** Multi-segment distribution breakdown (e.g. sleep stages, RAM usage) */
  segments?: Array<{
    label: string;
    value: string | number;
    percent: number;
    color?: string;
  }>;
  /** Circular progress ring (0-100) */
  ringPercent?: number;
  /** Allow tap-to-copy value */
  copyable?: boolean;
}

export interface ListItem {
  title: string;
  subtitle?: string;
  badge?: string;
  icon?: string;
  value?: string | number;
  status?: "normal" | "success" | "warning" | "danger" | "info";
  actionUrl?: string;
  copyable?: boolean;
}

export interface ProgressItem {
  label: string;
  current: number;
  total: number;
  unit?: string;
  color?: "blue" | "green" | "amber" | "rose" | "purple";
  segments?: Array<{
    label: string;
    value: number;
    color?: string;
  }>;
}

export interface KeyValueItem {
  key: string;
  value: string | number;
  hint?: string;
  badge?: string;
  status?: "normal" | "success" | "warning" | "danger";
  copyable?: boolean;
}

export interface TimelineItem {
  time: string;
  title: string;
  description?: string;
  status?: "done" | "current" | "pending";
}

export interface CardAction {
  label: string;
  prompt?: string;
  url?: string;
  style?: "primary" | "secondary" | "danger";
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
  actions?: CardAction[];
  footer?: string;
  /** Max visible items before collapsing with "Show more" */
  collapsedCount?: number;
}

const ACCENT_MAP: Record<string, { dot: string; badge: string; text: string; fill: string }> = {
  blue: {
    dot: "bg-blue-400",
    badge: "border border-blue-500/30 bg-blue-950/20 text-blue-300",
    text: "text-blue-400",
    fill: "bg-blue-500",
  },
  green: {
    dot: "bg-emerald-400",
    badge: "border border-emerald-500/30 bg-emerald-950/20 text-emerald-300",
    text: "text-emerald-400",
    fill: "bg-emerald-500",
  },
  emerald: {
    dot: "bg-emerald-400",
    badge: "border border-emerald-500/30 bg-emerald-950/20 text-emerald-300",
    text: "text-emerald-400",
    fill: "bg-emerald-500",
  },
  amber: {
    dot: "bg-amber-400",
    badge: "border border-amber-500/30 bg-amber-950/20 text-amber-300",
    text: "text-amber-400",
    fill: "bg-amber-500",
  },
  rose: {
    dot: "bg-rose-400",
    badge: "border border-rose-500/30 bg-rose-950/20 text-rose-300",
    text: "text-rose-400",
    fill: "bg-rose-500",
  },
  purple: {
    dot: "bg-purple-400",
    badge: "border border-purple-500/30 bg-purple-950/20 text-purple-300",
    text: "text-purple-400",
    fill: "bg-purple-500",
  },
  neutral: {
    dot: "bg-ink-secondary",
    badge: "border border-hairline/40 bg-control text-ink-secondary",
    text: "text-ink",
    fill: "bg-ink-secondary",
  },
};

/** Micro SVG Sparkline */
function SparklineView({ points, color = "#22c55e" }: { points: number[]; color?: string }) {
  if (!points || points.length < 2) return null;
  const min = Math.min(...points);
  const max = Math.max(...points);
  const range = max - min || 1;
  const width = 64;
  const height = 22;
  const coords = points.map((p, i) => {
    const x = (i / (points.length - 1)) * width;
    const y = height - 2 - ((p - min) / range) * (height - 6);
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });
  const pathD = `M ${coords.join(" L ")}`;

  return (
    <svg width={width} height={height} className="shrink-0 overflow-visible">
      <path d={pathD} fill="none" stroke={color} strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
      {/* End dot */}
      {points.length > 0 && (
        <circle
          cx={coords[coords.length - 1].split(",")[0]}
          cy={coords[coords.length - 1].split(",")[1]}
          r="2"
          fill={color}
        />
      )}
    </svg>
  );
}

/** Min-Max Range Bar with current position dot */
function RangeBarView({
  range,
}: {
  range: { min: number; max: number; current: number; optimalMin?: number; optimalMax?: number };
}) {
  const span = range.max - range.min || 1;
  const currentPos = Math.min(100, Math.max(0, ((range.current - range.min) / span) * 100));

  return (
    <div className="mt-2 w-full space-y-1">
      <div className="relative h-1.5 w-full rounded-full bg-control overflow-hidden">
        {range.optimalMin !== undefined && range.optimalMax !== undefined && (
          <div
            className="absolute top-0 bottom-0 bg-success/25"
            style={{
              left: `${Math.max(0, ((range.optimalMin - range.min) / span) * 100)}%`,
              width: `${Math.min(100, ((range.optimalMax - range.optimalMin) / span) * 100)}%`,
            }}
          />
        )}
        <div
          className="absolute top-0 bottom-0 w-1 bg-ink rounded-full"
          style={{ left: `calc(${currentPos}% - 2px)` }}
        />
      </div>
      <div className="flex justify-between text-[9.5px] font-mono text-ink-secondary">
        <span>{range.min}</span>
        <span className="text-ink font-medium">{range.current}</span>
        <span>{range.max}</span>
      </div>
    </div>
  );
}

/** Multi-Segment Breakdown Bar (e.g. Sleep stages, Docker RAM) */
function SegmentedBarView({
  segments,
}: {
  segments: Array<{ label: string; value: string | number; percent: number; color?: string }>;
}) {
  const colorMap: Record<string, string> = {
    purple: "bg-purple-500",
    blue: "bg-blue-500",
    emerald: "bg-emerald-500",
    amber: "bg-amber-500",
    rose: "bg-rose-500",
    neutral: "bg-ink-secondary",
  };

  return (
    <div className="mt-2 w-full space-y-1.5">
      <div className="flex h-2 w-full overflow-hidden rounded-md bg-control gap-0.5">
        {segments.map((seg, idx) => (
          <div
            key={idx}
            className={cn("h-full transition-all duration-300", colorMap[seg.color || "neutral"] || "bg-accent")}
            style={{ width: `${seg.percent}%` }}
            title={`${seg.label}: ${seg.value} (${seg.percent}%)`}
          />
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] text-ink-secondary">
        {segments.map((seg, idx) => (
          <div key={idx} className="flex items-center gap-1.5">
            <span
              className={cn("size-1.5 rounded-full shrink-0", colorMap[seg.color || "neutral"] || "bg-accent")}
            />
            <span className="font-mono text-ink font-medium">{seg.label}</span>
            <span className="text-ink-secondary">{seg.value}</span>
            <span className="text-ink-secondary/70">({seg.percent}%)</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/** Circular Ring Gauge (e.g. Activity, Target %) */
function RadialRingView({ percent, color = "#22c55e" }: { percent: number; color?: string }) {
  const size = 38;
  const stroke = 3.5;
  const radius = (size - stroke) / 2;
  const circumference = 2 * Math.PI * radius;
  const strokeDashoffset = circumference - (Math.min(100, Math.max(0, percent)) / 100) * circumference;

  return (
    <div className="relative flex items-center justify-center shrink-0" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90">
        <circle cx={size / 2} cy={size / 2} r={radius} stroke="currentColor" className="text-hairline/40" strokeWidth={stroke} fill="none" />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          stroke={color.startsWith("#") ? color : undefined}
          className={cn("transition-all duration-500", !color.startsWith("#") && (color.startsWith("bg-") ? color.replace("bg-", "text-") : "text-success"))}
          strokeWidth={stroke}
          strokeDasharray={circumference}
          strokeDashoffset={strokeDashoffset}
          strokeLinecap="round"
          fill="none"
        />
      </svg>
      <span className="absolute text-[9.5px] font-mono font-bold text-ink">{percent}%</span>
    </div>
  );
}

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

export function UniversalCard({
  card,
  botId,
  threadId,
  onPin,
  isPinned = false,
}: {
  card: UniversalCardData;
  botId?: string;
  threadId?: string;
  onPin?: (card: UniversalCardData) => void;
  isPinned?: boolean;
}) {
  const { state, dispatch } = useStore();
  const accentKey = card.accent || "blue";
  const colors = ACCENT_MAP[accentKey] || ACCENT_MAP.blue;
  const HeaderIcon = resolveIcon(card.icon);
  const layout = card.layout || (card.stats ? "stats" : card.progress ? "progress" : card.keyValue ? "key-value" : card.timeline ? "timeline" : "list");


  const handleActionClick = (action: CardAction) => {
    triggerHaptic("tap");
    if (action.url) {
      window.open(action.url, "_blank", "noopener,noreferrer");
      return;
    }
    if (action.prompt) {
      const activeBot = botId ? state.bots.find((b) => b.id === botId) : state.bots[0];
      if (activeBot) {
        dispatch({
          type: "send",
          botId: activeBot.id,
          threadId: threadId || activeBot.threadId,
          text: action.prompt,
        });
      }
    }
  };

  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [isExpanded, setIsExpanded] = useState(false);

  const copyToClipboard = (text: string, id: string) => {
    triggerHaptic("tap");
    void navigator.clipboard?.writeText(text);
    setCopiedKey(id);
    setTimeout(() => setCopiedKey(null), 1600);
  };

  const cardBody = (
    <div className="my-3 w-full max-w-2xl overflow-hidden rounded-xl border border-hairline/40 bg-card shadow-lg transition-all hover:border-hairline/80">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-hairline/30 px-3.5 py-2.5 bg-raised/50">
        <div className="flex items-center gap-2.5 min-w-0">
          <div className="flex size-6 shrink-0 items-center justify-center rounded border border-hairline/40 bg-control text-ink">
            <HeaderIcon size={13} />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className={cn("size-1.5 rounded-full shrink-0", colors.dot)} />
              <h3 className="text-[13px] font-mono font-semibold text-ink truncate">{card.title}</h3>
              {card.badge && (
                <span className={cn("rounded px-1.5 py-0.2 text-[9.5px] font-mono font-medium", colors.badge)}>
                  {card.badge}
                </span>
              )}
            </div>
            {card.subtitle && (
              <p className="text-[11px] font-mono text-ink-secondary truncate">{card.subtitle}</p>
            )}
          </div>
        </div>

        {/* Pin Control */}
        <div className="flex items-center gap-1.5 shrink-0 ml-2">
          {onPin && (
            <button
              type="button"
              onClick={() => {
                triggerHaptic("tap");
                onPin(card);
              }}
              title={isPinned ? "Unpin widget" : "Pin widget to top"}
              className={cn(
                "flex size-6 items-center justify-center rounded border transition-colors",
                isPinned ? "border-accent bg-accent text-accent-ink" : "border-hairline/40 bg-control text-ink-secondary hover:text-ink hover:border-hairline"
              )}
            >
              <Pin size={11} className={isPinned ? "fill-current" : ""} />
            </button>
          )}
        </div>
      </div>

      {/* Body Layouts */}
      <div className="p-3.5 space-y-3">
        {/* Layout: stats */}
        {layout === "stats" && card.stats && (
          <div className={cn(
            "grid gap-2",
            card.stats.length === 4
              ? "grid-cols-2" // symmetric 2x2 grid
              : card.stats.length > 2
              ? "grid-cols-2 sm:grid-cols-3"
              : card.stats.length === 2
              ? "grid-cols-2"
              : "grid-cols-1"
          )}>
            {card.stats.map((stat, idx) => {
              const copyId = `stat-${idx}`;
              const isCopied = copiedKey === copyId;

              return (
                <div
                  key={idx}
                  onClick={() => stat.copyable !== false && copyToClipboard(String(stat.value), copyId)}
                  className="group relative flex flex-col justify-between rounded-lg border border-hairline/30 bg-inset/50 p-3 transition-colors hover:border-hairline/70 cursor-pointer"
                  title="Click to copy value"
                >
                  <div className="flex items-center justify-between gap-1">
                    <span className="text-[11px] font-mono text-ink-secondary truncate" title={stat.label}>
                      {stat.label}
                    </span>
                    {isCopied ? (
                      <span className="text-[9px] font-mono text-success">✓ copied</span>
                    ) : stat.ringPercent !== undefined ? (
                      <RadialRingView percent={stat.ringPercent} color={colors.fill.replace("bg-", "")} />
                    ) : null}
                  </div>

                  <div className="mt-1 flex items-baseline justify-between gap-2">
                    <div className="flex items-baseline gap-1.5 flex-wrap">
                      <span className="text-[17px] font-mono font-bold tracking-tight text-ink">
                        {stat.value}
                      </span>
                      {stat.unit && <span className="text-[10.5px] font-mono text-ink-secondary">{stat.unit}</span>}
                    </div>
                    {stat.sparkline && (
                      <SparklineView points={stat.sparkline} color="#22c55e" />
                    )}
                  </div>

                  {/* Range gauge if present */}
                  {stat.range && (
                    <RangeBarView range={stat.range} />
                  )}

                  {/* Segments if present */}
                  {stat.segments && (
                    <SegmentedBarView segments={stat.segments} />
                  )}

                  {(stat.change || stat.description) && !stat.range && !stat.segments && (
                    <div className="mt-1.5 flex items-center gap-1 text-[10px] font-mono text-ink-secondary truncate">
                      {stat.trend === "up" && <TrendingUp size={11} className="text-success shrink-0" />}
                      {stat.trend === "down" && <TrendingDown size={11} className="text-danger shrink-0" />}
                      <span className="truncate">{stat.change || stat.description}</span>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {/* Layout: progress */}
        {layout === "progress" && card.progress && (
          <div className="space-y-2">
            {card.progress.map((prog, idx) => {
              const percent = Math.min(100, Math.max(0, Math.round((prog.current / (prog.total || 1)) * 100)));
              const unitText = prog.unit ? prog.unit.trim() : "";
              const alreadyHasPercent = unitText.includes("%");

              return (
                <div key={idx} className="rounded-lg border border-hairline/30 bg-inset/50 p-2.5">
                  <div className="flex items-center justify-between text-[11.5px] font-mono text-ink mb-1.5">
                    <span className="truncate">{prog.label}</span>
                    <span className="text-ink-secondary">
                      {prog.current} / {prog.total} {unitText} {!alreadyHasPercent && `(${percent}%)`}
                    </span>
                  </div>
                  {prog.segments ? (
                    <SegmentedBarView
                      segments={prog.segments.map((s) => ({
                        label: s.label,
                        value: s.value,
                        percent: Math.round((s.value / (prog.total || 1)) * 100),
                        color: s.color,
                      }))}
                    />
                  ) : (
                    <div className="h-1.5 w-full overflow-hidden rounded-full bg-control">
                      <div
                        className="h-full bg-accent transition-all duration-500 rounded-full"
                        style={{ width: `${percent}%` }}
                      />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {/* Layout: key-value */}
        {layout === "key-value" && card.keyValue && (() => {
          const limit = card.collapsedCount || 5;
          const hasMore = card.keyValue.length > limit;
          const visibleRows = isExpanded || !hasMore ? card.keyValue : card.keyValue.slice(0, limit);

          return (
            <div className="space-y-1.5">
              <div className="rounded-lg border border-hairline/30 bg-inset/50 divide-y divide-hairline/20 overflow-hidden">
                {visibleRows.map((kv, idx) => {
                  const copyId = `kv-${idx}`;
                  const isCopied = copiedKey === copyId;

                  return (
                    <div
                      key={idx}
                      onClick={() => kv.copyable !== false && copyToClipboard(String(kv.value), copyId)}
                      className="flex items-center justify-between px-3 py-2 text-[12px] font-mono hover:bg-raised/40 transition-colors cursor-pointer group"
                      title="Click to copy"
                    >
                      <span className="text-ink-secondary truncate mr-2">{kv.key}</span>
                      <div className="text-right shrink-0 flex items-center gap-2">
                        {isCopied ? (
                          <span className="text-[10px] text-success">✓ copied</span>
                        ) : (
                          <>
                            <span className="font-semibold text-ink">{kv.value}</span>
                            {kv.badge && (
                              <span className="rounded border border-hairline/40 bg-control px-1 py-0.2 text-[9.5px] text-ink-secondary">
                                {kv.badge}
                              </span>
                            )}
                          </>
                        )}
                        {kv.hint && <span className="text-[10px] text-ink-secondary/70">({kv.hint})</span>}
                      </div>
                    </div>
                  );
                })}
              </div>
              {hasMore && (
                <button
                  type="button"
                  onClick={() => {
                    triggerHaptic("tap");
                    setIsExpanded(!isExpanded);
                  }}
                  className="w-full rounded border border-hairline/30 bg-control/50 py-1 text-center text-[10.5px] font-mono text-ink-secondary hover:text-ink hover:border-hairline transition-colors"
                >
                  {isExpanded ? "Свернуть" : `Показать еще ${card.keyValue.length - limit} параметров`}
                </button>
              )}
            </div>
          );
        })()}

        {/* Layout: timeline */}
        {layout === "timeline" && card.timeline && (
          <div className="relative pl-4 space-y-3.5 before:absolute before:left-1.5 before:top-2 before:bottom-2 before:w-px before:bg-hairline/40">
            {card.timeline.map((tItem, idx) => (
              <div key={idx} className="relative">
                <span className={cn(
                  "absolute -left-[14px] top-1.5 size-2 rounded-full ring-2 ring-card",
                  tItem.status === "done" ? "bg-success" : tItem.status === "current" ? "bg-accent animate-pulse" : "bg-hairline"
                )} />
                <div className="flex items-baseline gap-2">
                  <span className="text-[10.5px] font-mono font-medium text-ink-secondary">{tItem.time}</span>
                  <span className="text-[12px] font-mono font-medium text-ink">{tItem.title}</span>
                </div>
                {tItem.description && (
                  <p className="mt-0.5 text-[11px] font-mono text-ink-secondary">{tItem.description}</p>
                )}
              </div>
            ))}
          </div>
        )}

        {/* Layout: list */}
        {layout === "list" && card.items && (
          <div className="space-y-1">
            {card.items.map((item, idx) => {
              const ItemIcon = resolveIcon(item.icon);
              return (
                <div
                  key={idx}
                  className="flex items-center justify-between gap-3 rounded-lg border border-hairline/30 bg-inset/50 px-3 py-2 hover:border-hairline/60 transition-colors"
                >
                  <div className="flex items-center gap-2 min-w-0">
                    <div className="flex size-5 shrink-0 items-center justify-center rounded bg-control text-ink-secondary">
                      <ItemIcon size={12} />
                    </div>
                    <div className="min-w-0">
                      <div className="flex items-center gap-1.5">
                        <span className="text-[12px] font-mono font-medium text-ink truncate">{item.title}</span>
                        {item.badge && (
                          <span className="rounded border border-hairline/40 bg-control px-1 py-0.2 text-[9px] font-mono text-ink-secondary">
                            {item.badge}
                          </span>
                        )}
                      </div>
                      {item.subtitle && <p className="text-[10.5px] font-mono text-ink-secondary/70 truncate">{item.subtitle}</p>}
                    </div>
                  </div>
                  {item.value !== undefined && (
                    <span className="font-mono text-[12px] font-semibold text-ink shrink-0">
                      {item.value}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Quick Action Buttons */}
      {card.actions && card.actions.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5 border-t border-hairline/30 bg-raised/30 px-3.5 py-2">
          {card.actions.map((act, idx) => {
            const isDanger = act.style === "danger";
            const isPrimary = act.style === "primary";

            return (
              <button
                key={idx}
                type="button"
                onClick={() => handleActionClick(act)}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-[11.5px] font-mono font-medium transition-all active:scale-95 cursor-pointer",
                  isDanger
                    ? "border-danger/40 bg-danger/10 text-danger hover:bg-danger/20"
                    : isPrimary
                    ? "border-accent bg-accent text-accent-ink hover:brightness-105"
                    : "border-hairline/40 bg-control text-ink hover:border-hairline hover:bg-raised"
                )}
              >
                {act.url ? <ExternalLink size={10} className="opacity-70" /> : <Send size={10} className="opacity-70" />}
                <span>{act.label}</span>
              </button>
            );
          })}
        </div>
      )}

      {/* Footer */}
      {card.footer && (
        <div className="border-t border-hairline/30 bg-raised/20 px-3.5 py-1.5 text-[10.5px] font-mono text-ink-secondary flex items-center gap-1.5">
          <Info size={11} className="shrink-0 text-ink-secondary/70" />
          <span className="truncate">{card.footer}</span>
        </div>
      )}
    </div>
  );

  if (isPinned) {
    return (
      <BorderBeam size="md" colorVariant="mono">
        {cardBody}
      </BorderBeam>
    );
  }

  return cardBody;
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
