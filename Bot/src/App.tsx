import { Suspense, lazy, useCallback, useEffect, useRef, useState } from "react";
import { Loader2, Menu } from "lucide-react";
import { StoreProvider, useStore } from "@/state/store";
import { welcomeDue } from "@/lib/onboarding";
import { ThreadRefsProvider } from "@/components/ThreadRefs";
import { emailGateDone, initAnalytics } from "@/lib/analytics";
import { Sidebar } from "@/components/Sidebar";
import { ChatView } from "@/components/ChatView";
import { GroupView } from "@/components/GroupView";
import { preloadConnectedApps } from "@/components/PluginsPanel";
import { UpdateBanner } from "@/components/UpdateBanner";
import { DesktopCapabilitiesProvider } from "@/components/DesktopCapabilities";
import { NoEngines } from "@/components/NoEngines";
import { CommandPalette } from "@/components/CommandPalette";
import { setLocale } from "@/lib/i18n";
import { triggerHaptic } from "@/lib/haptics";
import { shouldOpenKeyboardShortcuts } from "@/lib/keyboard-shortcuts";

// Code splitting: secondary views and modals loaded lazily
const WelcomeFlow = lazy(() => import("@/components/onboarding/WelcomeFlow").then(m => ({ default: m.WelcomeFlow })));
const FirstConversationTour = lazy(() => import("@/components/onboarding/FirstConversationTour").then(m => ({ default: m.FirstConversationTour })));
const GuidedTour = lazy(() => import("@/components/onboarding/GuidedTour").then(m => ({ default: m.GuidedTour })));
const BotSettingsDialog = lazy(() => import("@/components/BotSettingsDialog").then(m => ({ default: m.BotSettingsDialog })));
const RemoteAgentSettingsPanel = lazy(() => import("@/components/RemoteAgentSettingsPanel").then(m => ({ default: m.RemoteAgentSettingsPanel })));
const NewBotDialog = lazy(() => import("@/components/NewBotDialog").then(m => ({ default: m.NewBotDialog })));
const PluginsPanel = lazy(() => import("@/components/PluginsPanel").then(m => ({ default: m.PluginsPanel })));
const ComputerPanel = lazy(() => import("@/components/ComputerPanel").then(m => ({ default: m.ComputerPanel })));
const RemoteDesktopPanel = lazy(() => import("@/components/remote-desktop-panel").then(m => ({ default: m.RemoteDesktopPanel })));
const InspectorPanel = lazy(() => import("@/components/InspectorPanel").then(m => ({ default: m.InspectorPanel })));
const SettingsModal = lazy(() => import("@/components/SettingsModal").then(m => ({ default: m.SettingsModal })));
const WorkspaceBackupRecovery = lazy(() => import("@/components/WorkspaceBackupSettings").then(m => ({ default: m.WorkspaceBackupRecovery })));
const RoutinesPage = lazy(() => import("@/components/RoutinesPage").then(m => ({ default: m.RoutinesPage })));
const KeyboardShortcutsModal = lazy(() => import("@/components/KeyboardShortcutsModal").then(m => ({ default: m.KeyboardShortcutsModal })));
const LocalVmWorkspace = lazy(() => import("@/components/LocalVmWorkspace").then(m => ({ default: m.LocalVmWorkspace })));
const TeamMapPage = lazy(() => import("@/components/TeamMapPage").then(m => ({ default: m.TeamMapPage })));
const SplitWorkspace = lazy(() => import("@/components/SplitWorkspace").then(m => ({ default: m.SplitWorkspace })));
const SwarmCapabilityMatrix = lazy(() => import("@/components/SwarmCapabilityMatrix").then(m => ({ default: m.SwarmCapabilityMatrix })));
const DagWorkflowVisualizer = lazy(() => import("@/components/DagWorkflowVisualizer").then(m => ({ default: m.DagWorkflowVisualizer })));
const MemoryManagerModal = lazy(() => import("@/components/MemoryManagerModal").then(m => ({ default: m.MemoryManagerModal })));

function Shell() {
  const { state, dispatch } = useStore();
  const unreadCount =
    state.bots.filter((bot) => !bot.hidden && bot.unread).length +
    state.groups.filter((group) => group.unread).length;
  const remoteClient = window.ogb?.remoteClient?.active === true;
  // Mobile-only drawer state. Above md, none of these properties are emitted
  // at all — Sidebar scopes every mobile class with max-md: rather than
  // cancelling them with md:, which would still emit a translate value and
  // turn the aside into a containing block for its fixed descendants (see
  // Sidebar.tsx's className comment).
  const [drawerOpen, setDrawerOpen] = useState(false);
  // Apply the configured UI language the moment config arrives or changes;
  // "" follows the system. The epoch bump re-renders extracted strings —
  // t() reads a module variable, so React needs this nudge.
  const language = state.config?.language ?? "";
  const [, setLocaleEpoch] = useState(0);
  useEffect(() => {
    setLocale(language || globalThis.navigator?.language);
    setLocaleEpoch((epoch) => epoch + 1);
  }, [language]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [capabilityMatrixOpen, setCapabilityMatrixOpen] = useState(false);
  const [dagVisualizerOpen, setDagVisualizerOpen] = useState(false);
  const [memoryManagerOpen, setMemoryManagerOpen] = useState(false);
  const [zenMode, setZenMode] = useState(false);

  useEffect(() => {
    const onOpenMatrix = () => setCapabilityMatrixOpen(true);
    const onOpenDag = () => setDagVisualizerOpen(true);
    const onOpenMemory = () => setMemoryManagerOpen(true);
    const onToggleZen = () => setZenMode((z) => !z);
    window.addEventListener("open-swarm-matrix", onOpenMatrix);
    window.addEventListener("open-dag-visualizer", onOpenDag);
    window.addEventListener("open-memory-manager", onOpenMemory);
    window.addEventListener("toggle-zen-mode", onToggleZen);
    return () => {
      window.removeEventListener("open-swarm-matrix", onOpenMatrix);
      window.removeEventListener("open-dag-visualizer", onOpenDag);
      window.removeEventListener("open-memory-manager", onOpenMemory);
      window.removeEventListener("toggle-zen-mode", onToggleZen);
    };
  }, []);

  const [localVmWorkspaceBotId, setLocalVmWorkspaceBotId] = useState<string | null>(null);
  // the Browser tab, expanded into the main column (the small preview in
  // the panel hands off to this and back)
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const previousViewRef = useRef(state.activeView);
  const calendarOriginRef = useRef<"chat" | "team-map">("chat");
  const group = state.groups.find((g) => g.id === state.selectedId);
  const bot = group ? undefined : (state.bots.find((b) => b.id === state.selectedId) ?? state.bots[0]);
  const calendarFocus = state.activeView === "routines";

  // Nothing on this machine can run a bot. A missing cloud login does not
  // count — that CLI can still host a local model. Wait for the first
  // /api/instances response before deciding: an empty list means "not asked
  // yet", and flashing the setup screen at every launch would be worse.
  const noEngines =
    state.connected &&
    state.instances.length > 0 &&
    !state.instances.some((i) => i.snapshot.state === "available");

  // App-wide shortcuts: ⌘N new bot · ⌘1–9 jump to bot · ⌘⇧[ / ⌘⇧] prev/next · ⌘/ or ? shortcuts cheat sheet.
  // Kept deliberately small; every panel already closes on Esc.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing || state.shortcutsOpen) return;
      if (shouldOpenKeyboardShortcuts(e)) {
        e.preventDefault();
        dispatch({ type: "toggleShortcuts", open: true });
        return;
      }

      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      const bots = state.bots.filter((b) => !b.hidden);
      if (e.key === "n" && !e.shiftKey) {
        e.preventDefault();
        dispatch({ type: "toggleNewBot", open: true });
      } else if (/^[1-9]$/.test(e.key)) {
        const target = bots[Number(e.key) - 1];
        if (target) {
          e.preventDefault();
          dispatch({ type: "select", id: target.id });
        }
      } else if (e.shiftKey && (e.key === "[" || e.key === "]")) {
        const idx = bots.findIndex((b) => b.id === state.selectedId);
        const next = bots[(idx + (e.key === "]" ? 1 : -1) + bots.length) % bots.length];
        if (next) {
          e.preventDefault();
          dispatch({ type: "select", id: next.id });
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [state.bots, state.selectedId, state.shortcutsOpen, dispatch]);

  useEffect(() => {
    window.ogb?.setUnreadCount?.(unreadCount);
  }, [unreadCount]);

  // Warm connected-account state as soon as the local server is available.
  // The modal then opens with the correct Connect/Add account buttons and
  // quietly revalidates instead of rediscovering every account from scratch.
  useEffect(() => {
    if (!state.connected) return;
    void preloadConnectedApps().catch(() => {});
  }, [state.connected]);

  // Picking a conversation closes the drawer: on a phone the chat is what you
  // asked for, and leaving the list up would hide it. Watching activeView too
  // catches re-selecting the bot that is already current from another view —
  // the reducer switches the view without changing selectedId. pluginsOpen
  // and settingsOpen cover the same idea from a different trigger: close the
  // drawer whenever an action opens something over the chat.
  useEffect(() => {
    setDrawerOpen(false);
  }, [state.selectedId, bot?.threadId, group?.threadId, state.activeView, state.pluginsOpen, state.settingsOpen]);

  useEffect(() => {
    if (state.activeView === "routines" && previousViewRef.current !== "routines") {
      calendarOriginRef.current = previousViewRef.current;
    }
    previousViewRef.current = state.activeView;
  }, [state.activeView]);

  useEffect(() => {
    if (
      localVmWorkspaceBotId &&
      (state.activeView !== "chat" || state.selectedId !== localVmWorkspaceBotId)
    ) {
      setLocalVmWorkspaceBotId(null);
    }
  }, [localVmWorkspaceBotId, state.activeView, state.selectedId]);

  const openLocalVmWorkspace = (botId: string) => {
    dispatch({ type: "toggleComputer", open: false });
    setLocalVmWorkspaceBotId(botId);
  };

  const openComputerFromWorkspace = (botId: string) => {
    setLocalVmWorkspaceBotId(null);
    dispatch({ type: "select", id: botId });
    dispatch({ type: "toggleComputer", open: true });
  };

  const closeCalendar = useCallback(() => {
    if (calendarOriginRef.current === "team-map") {
      dispatch({ type: "showTeamMap" });
      return;
    }
    dispatch({ type: "select", id: state.selectedId });
  }, [dispatch, state.selectedId]);
  const openCalendarRoom = useCallback((id: string) => {
    dispatch({ type: "select", id });
  }, [dispatch]);

  const nativeViewOverlayOpen =
    drawerOpen ||
    paletteOpen ||
    state.settingsOpen ||
    state.computerOpen ||
    state.inspectorOpen ||
    state.appSettingsOpen ||
    state.pluginsOpen;

  // The viewer outlives ComputerPanel and can target any bot, so release control
  // here (always mounted) when a bot's viewer closes. release() is idempotent.
  useEffect(() => {
    return window.ogb?.desktopViewer?.onState((viewer) => {
      if (viewer.open || !viewer.contextId) return;
      const botId = viewer.contextId;
      void fetch(`/api/bots/${botId}/computer/control`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ action: "release" }),
      })
        .then((res) => (res.ok ? res.json() : null))
        .then((snap) => {
          if (snap) dispatch({ type: "computerControl", botId, held: snap.held === true, helpReason: snap.helpReason ?? null });
        })
        .catch(() => {});
      void fetch(`/api/bots/${botId}/computer/viewer-close`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: "{}",
      }).catch(() => {});
    });
  }, [dispatch]);

  return (
    <div className="flex h-full flex-col">
      {/* fixed-position popup, bottom-left — outside the layout flow */}
      <UpdateBanner />
      <div className={`relative flex min-h-0 flex-1 ${zenMode ? "zen-mode-active" : ""}`}>
      {!calendarFocus && !zenMode && <button
        type="button"
        ref={menuButtonRef}
        aria-label="Open bot list"
        aria-expanded={drawerOpen}
        onClick={() => {
          triggerHaptic("tap");
          setDrawerOpen(true);
        }}
        className="absolute left-2 top-[calc(0.625rem+env(safe-area-inset-top,0px))] z-30 flex size-11 items-center justify-center rounded-xl bg-panel/85 text-ink-secondary backdrop-blur-sm border border-hairline/40 shadow-sm active:scale-95 hover:bg-raised hover:text-ink md:hidden"
      >
        <Menu size={20} />
      </button>}
      {drawerOpen && !calendarFocus && !zenMode && (
        <div
          aria-hidden
          onClick={() => setDrawerOpen(false)}
          onMouseDown={(e) => e.target === e.currentTarget && setDrawerOpen(false)}
          className="animate-drawer-backdrop absolute inset-0 z-30 bg-black/60 backdrop-blur-[3px] md:hidden"
        />
      )}
      {!calendarFocus && !zenMode && <Sidebar
        open={drawerOpen}
        onClose={() => {
          setDrawerOpen(false);
          menuButtonRef.current?.focus();
        }}
      />}
      <Suspense fallback={null}>
        {state.activeView === "team-map" ? (
          <TeamMapPage />
        ) : state.activeView === "routines" ? (
          <RoutinesPage onBack={closeCalendar} onOpenRoom={openCalendarRoom} />
        ) : !remoteClient && localVmWorkspaceBotId ? (
          <LocalVmWorkspace
            primaryBotId={localVmWorkspaceBotId}
            overlayOpen={nativeViewOverlayOpen}
            onClose={() => setLocalVmWorkspaceBotId(null)}
            onOpenComputer={openComputerFromWorkspace}
          />
        ) : noEngines ? (
          <NoEngines />
        ) : state.secondarySelectedId ? (
          <SplitWorkspace
            primaryBot={bot}
            primaryGroup={group}
            secondaryBot={state.bots.find((b) => b.id === state.secondarySelectedId)}
            secondaryGroup={state.groups.find((g) => g.id === state.secondarySelectedId)}
          />
        ) : group ? (
          <GroupView key={group.id} group={group} />
        ) : bot ? (
          <ChatView bot={bot} />
        ) : (
          <main className="flex h-full min-w-0 flex-1 flex-col items-center justify-center gap-3 bg-app text-ink-secondary">
            <Loader2 size={20} className="animate-spin" />
            <div className="text-[14px]">
              {state.connected ? "No bots yet" : "Connecting to the bot server…"}
            </div>
            {!state.connected && (
              <div className="text-[12px]">
                Start it with <code className="rounded bg-raised px-1.5 py-0.5">pnpm dev:server</code>
              </div>
            )}
          </main>
        )}
      </Suspense>
      <Suspense fallback={null}>
        {state.settingsOpen && bot && (
          remoteClient
            ? <RemoteAgentSettingsPanel bot={bot} />
            : <BotSettingsDialog key={bot.id} bot={bot} />
        )}
        {state.computerOpen && bot && (
          remoteClient ? (
            <RemoteDesktopPanel key={bot.id} bot={bot} />
          ) : (
            <ComputerPanel
              key={bot.id}
              bot={bot}
              onOpenVmWorkspace={openLocalVmWorkspace}
            />
          )
        )}
        {!remoteClient && state.inspectorOpen && bot && <InspectorPanel key={bot.threadId} bot={bot} />}
        {state.appSettingsOpen && <SettingsModal />}
        {state.pluginsOpen && <PluginsPanel />}
        {state.newBotOpen && <NewBotDialog />}
        {state.shortcutsOpen && (
          <KeyboardShortcutsModal
            open={state.shortcutsOpen}
            onClose={() => dispatch({ type: "toggleShortcuts", open: false })}
          />
        )}
        {capabilityMatrixOpen && (
          <SwarmCapabilityMatrix onClose={() => setCapabilityMatrixOpen(false)} />
        )}
        {dagVisualizerOpen && (
          <DagWorkflowVisualizer onClose={() => setDagVisualizerOpen(false)} />
        )}
        {memoryManagerOpen && bot && (
          <MemoryManagerModal botId={bot.id} onClose={() => setMemoryManagerOpen(false)} />
        )}
      </Suspense>
      {/* mounted after the modals: same z-50 tier, so DOM order keeps the
          palette on top when one of them is open underneath */}
      <CommandPalette onOpenChange={setPaletteOpen} />
      </div>
    </div>
  );
}

/** Opens the welcome flow on a fresh workspace (the server's onboarding
 * record says so) or on request from Settings. The decision waits for the
 * config to arrive, so a returning user never sees the tour flash. */
function WelcomeGate() {
  const { state, dispatch } = useStore();
  const [dismissed, setDismissed] = useState(false);
  const due =
    !dismissed &&
    welcomeDue(state.config, {
      remoteClient: window.ogb?.remoteClient?.active === true,
      legacyDone: emailGateDone(),
    });
  if (!state.welcomeOpen && !due) return null;
  const bot = state.bots.find((b) => !b.hidden) ?? null;
  const replay = state.welcomeOpen && !due;
  return (
    <WelcomeFlow
      bot={bot}
      replay={replay}
      onDone={() => {
        setDismissed(true);
        dispatch({ type: "toggleWelcome", open: false });
        // the first real finish hands over to the guided tour; a replay does not
        if (!replay) dispatch({ type: "toggleTour", open: true });
      }}
    />
  );
}

function Application() {
  useEffect(() => {
    initAnalytics();
  }, []);
  return (
    <DesktopCapabilitiesProvider>
      <StoreProvider>
        <ThreadRefsProvider>
          <Shell />
        </ThreadRefsProvider>
        <WelcomeGate />
        <GuidedTour />
        <FirstConversationTour />
      </StoreProvider>
    </DesktopCapabilitiesProvider>
  );
}

export default function App() {
  return <WorkspaceBackupRecovery><Application /></WorkspaceBackupRecovery>;
}
