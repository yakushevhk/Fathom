import React, { useRef } from "react";
import { X, Columns } from "lucide-react";
import { useStore, type Bot, type Group } from "@/state/store";
import { ChatView } from "@/components/ChatView";
import { GroupView } from "@/components/GroupView";
import { triggerHaptic } from "@/lib/haptics";

interface SplitWorkspaceProps {
  primaryBot?: Bot;
  primaryGroup?: Group;
  secondaryBot?: Bot;
  secondaryGroup?: Group;
}

export function SplitWorkspace({
  primaryBot,
  primaryGroup,
  secondaryBot,
  secondaryGroup,
}: SplitWorkspaceProps) {
  const { state, dispatch } = useStore();
  const draggingRef = useRef(false);

  const handlePointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    draggingRef.current = true;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    triggerHaptic("selection");
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    const container = e.currentTarget.parentElement;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    dispatch({ type: "setSplitRatio", ratio });
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    if (draggingRef.current) {
      draggingRef.current = false;
      try {
        (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // pointer capture released
      }
    }
  };

  const primaryPercent = `${Math.round(state.splitRatio * 100)}%`;
  const secondaryPercent = `${Math.round((1 - state.splitRatio) * 100)}%`;

  return (
    <div className="relative flex h-full min-w-0 flex-1 overflow-hidden">
      {/* Mobile view toggle for screens < md (where 2 columns cannot fit) */}
      <div className="flex md:hidden flex-col h-full w-full overflow-hidden">
        <div className="flex items-center justify-between border-b border-hairline/30 bg-panel/85 px-3 py-1.5 backdrop-blur-sm z-30">
          <div className="flex items-center gap-1 bg-control/60 p-0.5 rounded-lg border border-hairline/40">
            <button
              type="button"
              onClick={() => dispatch({ type: "focusPane", pane: "primary" })}
              className={`px-2.5 py-1 text-[12px] font-medium rounded-md transition-colors ${
                state.activeFocusPane === "primary" ? "bg-panel text-ink shadow-xs" : "text-ink-secondary hover:text-ink"
              }`}
            >
              {primaryGroup?.name || primaryBot?.name || "Primary"}
            </button>
            <button
              type="button"
              onClick={() => dispatch({ type: "focusPane", pane: "secondary" })}
              className={`px-2.5 py-1 text-[12px] font-medium rounded-md transition-colors ${
                state.activeFocusPane === "secondary" ? "bg-panel text-ink shadow-xs" : "text-ink-secondary hover:text-ink"
              }`}
            >
              {secondaryGroup?.name || secondaryBot?.name || "Secondary"}
            </button>
          </div>
          <button
            type="button"
            onClick={() => {
              triggerHaptic("tap");
              dispatch({ type: "closeSplitChat" });
            }}
            className="flex size-7 items-center justify-center rounded-lg text-ink-secondary hover:bg-control hover:text-ink"
            title="Close Split View"
          >
            <X size={16} />
          </button>
        </div>
        <div className="relative flex-1 min-h-0 overflow-hidden">
          {state.activeFocusPane === "primary" ? (
            primaryGroup ? (
              <GroupView key={primaryGroup.id} group={primaryGroup} />
            ) : primaryBot ? (
              <ChatView key={primaryBot.id} bot={primaryBot} />
            ) : null
          ) : (
            secondaryGroup ? (
              <GroupView key={secondaryGroup.id} group={secondaryGroup} />
            ) : secondaryBot ? (
              <ChatView key={secondaryBot.id} bot={secondaryBot} />
            ) : null
          )}
        </div>
      </div>

      {/* Desktop dual-column view (>= md) */}
      <div className="hidden md:flex h-full w-full min-w-0 overflow-hidden">
        {/* Primary Pane */}
        <div
          style={{ width: primaryPercent }}
          onClick={() => {
            if (state.activeFocusPane !== "primary") {
              dispatch({ type: "focusPane", pane: "primary" });
            }
          }}
          className={`relative flex h-full min-w-[280px] flex-col overflow-hidden transition-[border-color] duration-150 ${
            state.activeFocusPane === "primary" ? "ring-1 ring-accent/30" : "opacity-95"
          }`}
        >
          {primaryGroup ? (
            <GroupView key={primaryGroup.id} group={primaryGroup} />
          ) : primaryBot ? (
            <ChatView key={primaryBot.id} bot={primaryBot} />
          ) : null}
        </div>

        {/* Resizable Divider Handle */}
        <div
          role="separator"
          aria-orientation="vertical"
          aria-valuenow={Math.round(state.splitRatio * 100)}
          aria-valuemin={20}
          aria-valuemax={80}
          aria-label="Split pane resize handle"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "ArrowLeft") {
              e.preventDefault();
              dispatch({ type: "setSplitRatio", ratio: Math.max(0.2, state.splitRatio - 0.05) });
            } else if (e.key === "ArrowRight") {
              e.preventDefault();
              dispatch({ type: "setSplitRatio", ratio: Math.min(0.8, state.splitRatio + 0.05) });
            } else if (e.key === "Home") {
              e.preventDefault();
              dispatch({ type: "setSplitRatio", ratio: 0.5 });
            }
          }}
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onDoubleClick={() => {
            triggerHaptic("tap");
            dispatch({ type: "setSplitRatio", ratio: 0.5 });
          }}
          className="group relative z-20 flex w-2 shrink-0 cursor-col-resize items-center justify-center bg-hairline/30 hover:bg-accent/40 active:bg-accent transition-colors focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-accent"
          title="Drag or use Left/Right arrows to resize, double click to center"
        >
          <div className="h-8 w-1 rounded-full bg-hairline group-hover:bg-accent transition-colors" />
        </div>

        {/* Secondary Pane */}
        <div
          style={{ width: secondaryPercent }}
          onClick={() => {
            if (state.activeFocusPane !== "secondary") {
              dispatch({ type: "focusPane", pane: "secondary" });
            }
          }}
          className={`relative flex h-full min-w-[280px] flex-col overflow-hidden transition-[border-color] duration-150 ${
            state.activeFocusPane === "secondary" ? "ring-1 ring-accent/30" : "opacity-95"
          }`}
        >
          {/* Header toolbar with close split button */}
          <div className="absolute right-3 top-2.5 z-30 flex items-center gap-1 rounded-lg bg-panel/85 p-1 border border-hairline/40 shadow-xs backdrop-blur-sm">
            <div className="flex items-center gap-1 px-1 text-[11px] font-medium text-ink-secondary">
              <Columns size={12} className="text-accent" />
              <span>Split View</span>
            </div>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                triggerHaptic("tap");
                dispatch({ type: "closeSplitChat" });
              }}
              className="flex size-6 items-center justify-center rounded text-ink-secondary hover:bg-raised hover:text-ink transition-colors"
              title="Close Split View"
            >
              <X size={14} />
            </button>
          </div>

          {secondaryGroup ? (
            <GroupView key={secondaryGroup.id} group={secondaryGroup} />
          ) : secondaryBot ? (
            <ChatView key={secondaryBot.id} bot={secondaryBot} />
          ) : null}
        </div>
      </div>
    </div>
  );
}
