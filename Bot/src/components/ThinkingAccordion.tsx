import { useState } from "react";
import {
  Brain,
  ChevronDown,
  ChevronRight,
  Sparkles,
  Terminal,
  CheckCircle2,
  XCircle,
  Loader2,
  Layers,
} from "lucide-react";
import { ThinkingOrb } from "thinking-orbs";
import { BorderBeam } from "border-beam";
import type { Message } from "@/state/store";
interface ThinkingAccordionProps {
  reasoning: string;
  isStreaming?: boolean;
  /** Activity / tool messages that occurred in the current turn before the final response */
  steps?: Message[];
}

export function ThinkingAccordion({
  reasoning,
  isStreaming = false,
  steps = [],
}: ThinkingAccordionProps) {
  const [open, setOpen] = useState(false);

  // If there's no reasoning, no active streaming, and no steps, keep hidden
  if (!reasoning && !isStreaming && steps.length === 0) return null;

  const charCount = reasoning.length;
  const estimatedTokens = Math.max(1, Math.round(charCount / 4));
  const toolStepsCount = steps.filter((s) => s.kind === "activity" && s.tool).length;
  const inFlightStep = steps.find(
    (s) => s.kind === "activity" && s.tool && s.tool.ok === undefined,
  );

  const content = (
    <div className="my-2.5 max-w-3xl overflow-hidden rounded-xl border border-hairline/40 bg-card shadow-xs transition-all">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between px-3 py-2 text-left text-[12px] font-mono text-ink-secondary hover:bg-raised/50 hover:text-ink transition-colors"
      >
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center gap-2">
            {isStreaming ? (
              <div className="flex size-5 shrink-0 items-center justify-center">
                <ThinkingOrb state="solving" size={20} theme="auto" />
              </div>
            ) : (
              <div className="flex size-5 shrink-0 items-center justify-center rounded bg-raised border border-hairline/40">
                <span className="text-[10px] font-mono text-ink-secondary">✦</span>
              </div>
            )}
            <span className="font-semibold text-ink">
              {isStreaming ? "Thinking…" : "Thought & Execution"}
            </span>
          </div>

          {/* Tokens indicator */}
          <span className="rounded bg-control border border-hairline/40 px-1.5 py-0.5 text-[10.5px] font-mono text-ink-secondary">
            ~{estimatedTokens} tokens
          </span>
          {/* Tool calls & steps count badge */}
          {toolStepsCount > 0 && (
            <span className="flex items-center gap-1 rounded-full bg-accent/15 px-2 py-0.5 text-[11px] font-medium text-accent">
              <Layers size={11} />
              <span>
                {toolStepsCount} {toolStepsCount === 1 ? "step" : "steps"}
              </span>
            </span>
          )}

          {/* Active in-flight action indicator */}
          {inFlightStep && inFlightStep.tool && (
            <span className="flex items-center gap-1.5 rounded-full bg-control px-2.5 py-0.5 text-[11px] text-ink-secondary max-w-[200px] sm:max-w-xs truncate">
              <Loader2 size={11} className="animate-spin text-accent shrink-0" />
              <span className="truncate">{inFlightStep.tool.name}</span>
            </span>
          )}
        </div>

        <div className="flex items-center gap-1 ml-2 shrink-0">
          <span className="text-[11px] text-ink-secondary/70">{open ? "Hide" : "Details"}</span>
          {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </div>
      </button>

      {open && (
        <div className="border-t border-hairline/40 bg-inset/25 p-3.5 space-y-3">
          {/* Tool execution steps log */}
          {steps.length > 0 && (
            <div className="space-y-1.5">
              <div className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-ink-secondary">
                <Terminal size={12} />
                <span>Tool Calls & Operations ({steps.length})</span>
              </div>
              <div className="divide-y divide-hairline/30 rounded-xl border border-hairline/40 bg-panel/80 overflow-hidden">
                {steps.map((step) => {
                  const tool = step.tool;
                  if (!tool) return null;
                  const isFailed = tool.ok === false;
                  const isRunning = tool.ok === undefined;

                  return (
                    <div
                      key={step.id}
                      className="flex items-center justify-between px-3 py-2 text-[12px] font-mono gap-2 hover:bg-raised/40 transition-colors"
                    >
                      <div className="flex items-center gap-2 min-w-0">
                        {isRunning ? (
                          <Loader2 size={13} className="animate-spin text-accent shrink-0" />
                        ) : isFailed ? (
                          <XCircle size={13} className="text-danger shrink-0" />
                        ) : (
                          <CheckCircle2 size={13} className="text-success shrink-0" />
                        )}
                        <span className="font-medium text-ink truncate">{tool.name}</span>
                        {tool.summary && tool.summary !== tool.name && (
                          <span className="text-ink-secondary truncate max-w-[280px]" title={tool.summary}>
                            {tool.summary}
                          </span>
                        )}
                      </div>
                      <div className="flex items-center gap-1 text-[10px] text-ink-secondary shrink-0">
                        {isRunning ? (
                          <span className="text-accent animate-pulse">Running</span>
                        ) : isFailed ? (
                          <span className="text-danger">Failed</span>
                        ) : (
                          <span className="text-success">Done</span>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Reasoning / thinking text */}
          <div>
            <div className="flex items-center gap-1.5 mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-ink-secondary">
              <Brain size={12} />
              <span>Internal Thought Stream</span>
            </div>
            <div className="rounded-xl border border-hairline/40 bg-inset/40 px-3 py-2.5 text-[12px] leading-relaxed text-ink/80 font-mono whitespace-pre-wrap max-h-64 overflow-y-auto select-text">
              {reasoning || (
                <div className="flex items-center gap-2 italic text-ink-secondary py-1">
                  <Sparkles size={13} className="animate-spin text-accent" />
                  <span>Synthesizing internal thought path...</span>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );

  if (isStreaming) {
    return (
      <BorderBeam size="sm" colorVariant="mono">
        {content}
      </BorderBeam>
    );
  }

  return content;
}
