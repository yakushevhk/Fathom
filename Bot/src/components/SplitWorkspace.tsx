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
        tabIndex={0}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onDoubleClick={() => {
          triggerHaptic("tap");
          dispatch({ type: "setSplitRatio", ratio: 0.5 });
        }}
        className="group relative z-20 flex w-2 shrink-0 cursor-col-resize items-center justify-center bg-hairline/30 hover:bg-accent/40 active:bg-accent transition-colors"
        title="Drag to resize, double click to center"
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
  );
}
