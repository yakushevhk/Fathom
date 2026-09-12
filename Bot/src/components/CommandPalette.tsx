import { useEffect, useRef, useState } from "react";
import {
  Bot as BotIcon,
  Calculator,
  Columns,
  Database,
  Download,
  FolderPlus,
  HelpCircle,
  Keyboard,
  Layers,
  MessageSquare,
  Monitor,
  Network,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Shield,
  Terminal,
  Trash2,
  Users,
  Wrench,
} from "lucide-react";
import { api, useStore, type Bot, type Group } from "@/state/store";
import { rankByName } from "@/lib/palette-rank";
import { cn } from "@/lib/cn";
import type { SearchHit } from "@/lib/search-hit";
import { landOnSearchHit } from "@/lib/focus-message";
import { triggerHaptic } from "@/lib/haptics";

export interface ActionCommand {
  id: string;
  title: string;
  subtitle?: string;
  shortcut?: string;
  icon: React.ComponentType<{ size: number; className?: string }>;
  perform: () => void;
}

type PaletteEntry =
  | { kind: "calc"; expression: string; result: string }
  | { kind: "action"; action: ActionCommand }
  | { kind: "bot"; bot: Bot }
  | { kind: "room"; group: Group }
  | { kind: "message"; hit: SearchHit };

function evaluateCalculator(query: string): { expression: string; result: string } | null {
  const clean = query.trim();
  if (!/^[\d\s+\-*/().%^]+$/.test(clean) || clean.length < 3) return null;
  // Ensure there is at least one operator and numbers
  if (!/[+\-*/%^]/.test(clean) || !/\d/.test(clean)) return null;

  try {
    // Safe evaluation using Function with strictly sanitized arithmetic tokens
    const expr = clean.replace(/\^/g, "**");
    // eslint-disable-next-line @typescript-eslint/no-implied-eval
    const val = Function(`'use strict'; return (${expr})`)();
    if (typeof val === "number" && !Number.isNaN(val) && Number.isFinite(val)) {
      return { expression: clean, result: String(val) };
    }
  } catch {
    // Malformed math string
  }
  return null;
}

export function CommandPalette({ onOpenChange }: { onOpenChange?: (open: boolean) => void }) {
  const { state, dispatch } = useStore();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [messageHits, setMessageHits] = useState<SearchHit[]>([]);
  const [cursor, setCursor] = useState(0);
  const selectedRef = useRef<HTMLButtonElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k" && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        triggerHaptic("tap");
        setOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setMessageHits([]);
    setCursor(0);
  }, [open]);

  useEffect(() => {
    onOpenChange?.(open);
  }, [onOpenChange, open]);

  const q = query.trim().toLowerCase();

  useEffect(() => {
    if (!open || !q || evaluateCalculator(query)) {
      setMessageHits([]);
      return;
    }
    setMessageHits([]);
    let alive = true;
    const timer = setTimeout(() => {
      api(`/api/search?q=${encodeURIComponent(q)}&limit=10`)
        .then((result: { hits?: SearchHit[] }) => alive && setMessageHits(result.hits ?? []))
        .catch(() => alive && setMessageHits([]));
    }, 150);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [open, q, query]);

  useEffect(() => setCursor(0), [q]);

  useEffect(() => {
    selectedRef.current?.scrollIntoView({ block: "nearest" });
  }, [cursor, messageHits]);

  // Trap focus inside modal
  useEffect(() => {
    if (!open) return;
    const handleTab = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const modal = dialogRef.current;
      if (!modal) return;
      const focusable = modal.querySelectorAll<HTMLElement>(
        'input, button:not([disabled]), [tabindex]:not([tabindex="-1"])'
      );
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", handleTab);
    return () => window.removeEventListener("keydown", handleTab);
  }, [open]);

  if (!open) return null;

  const currentBot = state.bots.find((b) => b.id === state.selectedId);

  // Available Action Commands
  const actions: ActionCommand[] = [
    {
      id: "new-task",
      title: "New Conversation / Thread",
      subtitle: currentBot ? `Start fresh task for ${currentBot.name}` : "Start fresh task",
      shortcut: "⌘T",
      icon: Plus,
      perform: () => {
        if (currentBot) {
          dispatch({ type: "newTask", botId: currentBot.id });
        }
      },
    },
    {
      id: "toggle-split",
      title: state.secondarySelectedId ? "Close Split View" : "Open Split Dual Chat",
      subtitle: state.secondarySelectedId ? "Return to single conversation" : "Side-by-side comparison",
      icon: Columns,
      perform: () => {
        if (state.secondarySelectedId) {
          dispatch({ type: "closeSplitChat" });
        } else {
          const alternate = state.bots.find((b) => b.id !== state.selectedId && !b.hidden);
          if (alternate) {
            dispatch({ type: "openSplitChat", id: alternate.id });
          }
        }
      },
    },
    {
      id: "toggle-zen-mode",
      title: "Toggle Zen Mode (Distraction-Free)",
      subtitle: "Focus purely on active chat transcript without peripheral chrome",
      shortcut: "⌘⇧D",
      icon: Layers,
      perform: () => {
        window.dispatchEvent(new CustomEvent("toggle-zen-mode"));
      },
    },
    {
      id: "open-memory-manager",
      title: "Open Memory & Knowledge Graph Manager",
      subtitle: "Inspect entities, topic files, and vector embeddings",
      icon: Database,
      perform: () => {
        window.dispatchEvent(new CustomEvent("open-memory-manager"));
      },
    },
    {
      id: "open-dag-visualizer",
      title: "Open Swarm DAG Workflow Visualizer",
      subtitle: "Inspect multi-agent DAG pipeline, consensus debate & step debugger",
      icon: Network,
      perform: () => {
        window.dispatchEvent(new CustomEvent("open-dag-visualizer"));
      },
    },
    {
      id: "toggle-computer",
      title: state.computerOpen ? "Close Computer & Desktop Panel" : "Open Computer & Desktop Panel",
      subtitle: "Remote VM or browser control viewport",
      shortcut: "⌘E",
      icon: Monitor,
      perform: () => {
        dispatch({ type: "toggleComputer" });
      },
    },
    {
      id: "toggle-inspector",
      title: state.inspectorOpen ? "Close Inspector Panel" : "Open Inspector Panel",
      subtitle: "Inspect raw stream events and agent telemetry",
      shortcut: "⌘⇧I",
      icon: Terminal,
      perform: () => {
        dispatch({ type: "toggleInspector" });
      },
    },
    {
      id: "open-app-settings",
      title: "Open Settings",
      subtitle: "General, Appearance, Engines, MCP, and Cloud",
      shortcut: "⌘,",
      icon: Settings,
      perform: () => {
        dispatch({ type: "toggleAppSettings", open: true });
      },
    },
    {
      id: "open-mcp-plugins",
      title: "Manage MCP Servers & Tools",
      subtitle: "Model Context Protocol integrations",
      icon: Wrench,
      perform: () => {
        dispatch({ type: "togglePlugins", open: true, surface: "mcp" });
      },
    },
    {
      id: "swarm-matrix",
      title: "Swarm Capability Matrix",
      subtitle: "View and edit team permissions, chief of staff, and tool access",
      icon: Shield,
      perform: () => {
        // Dispatch custom event for opening matrix
        window.dispatchEvent(new CustomEvent("open-swarm-matrix"));
      },
    },
    {
      id: "view-shortcuts",
      title: "Keyboard Shortcuts",
      subtitle: "View all app shortcuts and navigation keys",
      shortcut: "?",
      icon: Keyboard,
      perform: () => {
        dispatch({ type: "toggleShortcuts", open: true });
      },
    },
    {
      id: "new-bot",
      title: "Create New Agent Bot",
      subtitle: "Add a specialized persona or coding teammate",
      shortcut: "⌘N",
      icon: BotIcon,
      perform: () => {
        dispatch({ type: "toggleNewBot", open: true });
      },
    },
  ];

  const filteredActions = q
    ? actions.filter(
        (a) =>
          a.title.toLowerCase().includes(q) ||
          (a.subtitle && a.subtitle.toLowerCase().includes(q))
      )
    : actions;

  const calcHit = evaluateCalculator(query);
  const bots = rankByName(state.bots.filter((b) => !b.hidden), q);
  const rooms = rankByName(state.groups, q);

  const entries: PaletteEntry[] = [
    ...(calcHit ? [{ kind: "calc" as const, ...calcHit }] : []),
    ...filteredActions.map((action): PaletteEntry => ({ kind: "action", action })),
    ...bots.map((bot): PaletteEntry => ({ kind: "bot", bot })),
    ...rooms.map((group): PaletteEntry => ({ kind: "room", group })),
    ...(q && !calcHit ? messageHits.map((hit): PaletteEntry => ({ kind: "message", hit })) : []),
  ];

  const selected = entries.length ? Math.min(cursor, entries.length - 1) : 0;

  const activate = async (entry: PaletteEntry) => {
    triggerHaptic("selection");
    if (entry.kind === "calc") {
      navigator.clipboard.writeText(entry.result);
      setOpen(false);
      return;
    }
    if (entry.kind === "action") {
      entry.action.perform();
      setOpen(false);
      return;
    }
    if (entry.kind === "message") {
      const hit = entry.hit;
      try {
        await landOnSearchHit(hit, state, dispatch);
      } catch (error) {
        dispatch({ type: "error", message: error instanceof Error ? error.message : String(error) });
      }
    } else {
      dispatch({ type: "select", id: entry.kind === "bot" ? entry.bot.id : entry.group.id });
    }
    setOpen(false);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setCursor(entries.length ? (selected + 1) % entries.length : 0);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setCursor(entries.length ? (selected - 1 + entries.length) % entries.length : 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const entry = entries[selected];
      if (entry) void activate(entry);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command Palette"
      onClick={() => setOpen(false)}
      className="modal-backdrop fixed inset-0 z-50 flex items-start justify-center p-3 sm:p-6 sm:pt-20"
    >
      <div
        ref={dialogRef}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        className="flex w-full max-w-xl flex-col overflow-hidden rounded-2xl border border-hairline/60 bg-panel shadow-2xl animate-pop-in"
      >
        <div className="flex items-center gap-2 border-b border-hairline/40 px-3.5 py-2.5">
          <Search size={16} className="text-ink-secondary shrink-0" />
          <input
            autoFocus
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Type a command, search bots & messages, or calculate (e.g. 1920/2)..."
            className="w-full bg-transparent text-[14.5px] text-ink placeholder:text-ink-secondary focus:outline-none"
          />
          <kbd className="hidden sm:inline-block rounded border border-hairline/60 bg-raised px-1.5 py-0.5 text-[10px] font-mono text-ink-secondary">
            ESC
          </kbd>
        </div>

        <div className="max-h-[60vh] overflow-y-auto p-1.5">
          {entries.length === 0 ? (
            <div className="px-3 py-8 text-center text-xs text-ink-secondary">
              No results matching "{query}"
            </div>
          ) : (
            entries.map((entry, idx) => {
              const isSelected = idx === selected;
              return (
                <button
                  key={
                    entry.kind === "calc"
                      ? `calc-${entry.expression}`
                      : entry.kind === "action"
                      ? `action-${entry.action.id}`
                      : entry.kind === "bot"
                      ? `bot-${entry.bot.id}`
                      : entry.kind === "room"
                      ? `room-${entry.group.id}`
                      : `msg-${entry.hit.messageId}`
                  }
                  ref={isSelected ? selectedRef : undefined}
                  type="button"
                  onClick={() => void activate(entry)}
                  onMouseEnter={() => setCursor(idx)}
                  className={cn(
                    "flex w-full items-center justify-between rounded-xl px-3 py-2 text-left text-[13px] transition-colors",
                    isSelected ? "bg-accent/15 text-accent font-medium" : "text-ink hover:bg-raised/60"
                  )}
                >
                  <div className="flex min-w-0 items-center gap-2.5">
                    {entry.kind === "calc" ? (
                      <Calculator size={16} className="shrink-0 text-accent" />
                    ) : entry.kind === "action" ? (
                      <entry.action.icon size={16} className="shrink-0 text-accent" />
                    ) : entry.kind === "bot" ? (
                      <BotIcon size={16} className="shrink-0 text-ink-secondary" />
                    ) : entry.kind === "room" ? (
                      <Users size={16} className="shrink-0 text-ink-secondary" />
                    ) : (
                      <MessageSquare size={16} className="shrink-0 text-ink-secondary" />
                    )}

                    <div className="min-w-0 flex-1 truncate">
                      {entry.kind === "calc" ? (
                        <div className="flex items-center gap-2">
                          <span className="text-ink">{entry.expression} =</span>
                          <span className="font-bold text-accent">{entry.result}</span>
                          <span className="text-[11px] text-ink-secondary">(Press Enter to copy)</span>
                        </div>
                      ) : entry.kind === "action" ? (
                        <div>
                          <div>{entry.action.title}</div>
                          {entry.action.subtitle && (
                            <div className="text-[11px] text-ink-secondary font-normal truncate">
                              {entry.action.subtitle}
                            </div>
                          )}
                        </div>
                      ) : entry.kind === "bot" ? (
                        <div>
                          <div>{entry.bot.name}</div>
                          {entry.bot.title && (
                            <div className="text-[11px] text-ink-secondary font-normal truncate">
                              {entry.bot.title}
                            </div>
                          )}
                        </div>
                      ) : entry.kind === "room" ? (
                        <div>
                          <div>{entry.group.name}</div>
                          <div className="text-[11px] text-ink-secondary font-normal">
                            Group Room · {entry.group.memberIds.length} members
                          </div>
                        </div>
                      ) : (
                        <div>
                          <div className="truncate text-ink">{entry.hit.snippet}</div>
                          <div className="text-[11px] text-ink-secondary font-normal">
                            Message in thread
                          </div>
                        </div>
                      )}
                    </div>
                  </div>

                  {entry.kind === "action" && entry.action.shortcut && (
                    <kbd className="shrink-0 rounded border border-hairline/50 bg-raised px-1.5 py-0.5 text-[10px] font-mono text-ink-secondary">
                      {entry.action.shortcut}
                    </kbd>
                  )}
                </button>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
