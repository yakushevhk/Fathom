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
import { cn } from "@/lib/cn";
import { WorkingTimer } from "@/components/WorkingIndicator";
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
  const toolSteps = steps.filter((s) => s.kind === "activity" && s.tool);
  const toolStepsCount = toolSteps.length;
  const runningStepsCount = toolSteps.filter((s) => s.tool && s.tool.ok === undefined).length;
  const doneStepsCount = toolSteps.filter((s) => s.tool && s.tool.ok === true).length;
  const inFlightStep = toolSteps.find((s) => s.tool && s.tool.ok === undefined);
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
            <span className="flex items-center gap-1.5 rounded-full bg-accent/15 px-2.5 py-0.5 text-[11px] font-medium text-accent border border-accent/25">
              <Layers size={11} />
              <span>
                {toolStepsCount} {toolStepsCount === 1 ? "step" : "steps"}
                {runningStepsCount > 0 && ` (${runningStepsCount} running${doneStepsCount > 0 ? `, ${doneStepsCount} done` : ""})`}
                {runningStepsCount === 0 && doneStepsCount > 0 && ` (${doneStepsCount} done)`}
              </span>
            </span>
          )}

          {/* Active in-flight action indicator */}
          {inFlightStep && inFlightStep.tool && (() => {
            const isSubagent = inFlightStep.tool.name.toLowerCase().includes("task") || inFlightStep.tool.name.toLowerCase().includes("agent");
            const label = inFlightStep.tool.summary || inFlightStep.tool.name;
            return (
              <span className="flex items-center gap-1.5 rounded-full bg-control border border-hairline/40 px-2.5 py-0.5 text-[11px] text-ink max-w-[240px] sm:max-w-sm truncate animate-pulse">
                <Loader2 size={11} className="animate-spin text-accent shrink-0" />
                <span className="font-semibold text-accent shrink-0">{isSubagent ? "Agent" : inFlightStep.tool.name}:</span>
                <span className="truncate text-ink-secondary">{label}</span>
              </span>
            );
          })()}
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
              <div className="flex items-center justify-between text-[11px] font-semibold uppercase tracking-wider text-ink-secondary">
                <div className="flex items-center gap-1.5">
                  <Terminal size={12} />
                  <span>Tool Calls & Operations ({steps.length})</span>
                </div>
                <div className="flex items-center gap-2 font-mono text-[10.5px] normal-case tracking-normal">
                  {runningStepsCount > 0 && (
                    <span className="text-accent flex items-center gap-1">
                      <span className="size-1.5 rounded-full bg-accent animate-ping" />
                      {runningStepsCount} in progress
                    </span>
                  )}
                  {doneStepsCount > 0 && (
                    <span className="text-success">
                      {doneStepsCount} completed
                    </span>
                  )}
                </div>
              </div>
              <div className="divide-y divide-hairline/30 rounded-xl border border-hairline/40 bg-panel/80 overflow-hidden">
                {steps.map((step, idx) => {
                  const tool = step.tool;
                  if (!tool) return null;
                  const isFailed = tool.ok === false;
                  const isRunning = tool.ok === undefined;

                  const isTask = tool.name.toLowerCase().includes("task") || tool.name.toLowerCase().includes("agent");
                  const isBash = tool.name.toLowerCase().includes("bash") || tool.name.toLowerCase().includes("terminal");

                  return (
                    <div
                      key={step.id}
                      className={cn(
                        "flex items-center justify-between px-3 py-2 text-[12px] font-mono gap-2 hover:bg-raised/40 transition-colors",
                        isRunning && "bg-accent/[0.04]"
                      )}
                    >
                      <div className="flex items-center gap-2.5 min-w-0">
                        {isRunning ? (
                          <span className="relative flex size-3.5 items-center justify-center shrink-0">
                            <span className="absolute size-full rounded-full bg-accent/30 animate-ping" />
                            <Loader2 size={12} className="animate-spin text-accent" />
                          </span>
                        ) : isFailed ? (
                          <XCircle size={13} className="text-danger shrink-0" />
                        ) : (
                          <CheckCircle2 size={13} className="text-success shrink-0" />
                        )}

                        <span className={cn(
                          "rounded px-1.5 py-0.2 text-[10px] font-bold uppercase tracking-wider shrink-0 border",
                          isTask
                            ? "border-accent/40 bg-accent/15 text-accent"
                            : isBash
                            ? "border-hairline/50 bg-control text-ink"
                            : "border-hairline/30 bg-raised text-ink-secondary"
                        )}>
                          {isTask ? "🤖 Subagent #" + (idx + 1) : isBash ? "$ " + tool.name : tool.name}
                        </span>

                        {tool.summary && tool.summary !== tool.name ? (
                          <span className="text-ink font-medium truncate max-w-[340px]" title={tool.summary}>
                            {tool.summary}
                          </span>
                        ) : (
                          <span className="text-ink-secondary truncate max-w-[300px]">
                            {isTask ? `Subagent task #${idx + 1} execution…` : isBash ? "Shell command execution…" : "Tool execution in progress…"}
                          </span>
                        )}
                      </div>

                      <div className="flex items-center gap-2 text-[10.5px] font-mono shrink-0">
                        {step.at && (
                          <span className="text-ink-secondary/60 text-[10px] tabular-nums">
                            {isRunning ? <WorkingTimer since={step.at} /> : null}
                          </span>
                        )}
                        {isRunning ? (
                          <span className="inline-flex items-center gap-1 rounded bg-accent/10 border border-accent/20 px-1.5 py-0.2 text-accent animate-pulse font-semibold">
                            <span className="size-1.5 rounded-full bg-accent animate-ping" />
                            active
                          </span>
                        ) : isFailed ? (
                          <span className="rounded bg-danger/10 border border-danger/20 px-1.5 py-0.2 text-danger font-medium">Failed</span>
                        ) : (
                          <span className="rounded bg-success/10 border border-success/20 px-1.5 py-0.2 text-success font-medium">✓ Done</span>
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
            <div className="rounded-xl border border-hairline/40 bg-inset/40 px-3.5 py-3 text-[12.5px] leading-relaxed text-ink/90 max-h-72 overflow-y-auto select-text font-sans">
              {reasoning ? (
                <div className="space-y-2 whitespace-pre-wrap font-sans [&_strong]:text-ink [&_strong]:font-semibold">
                  {reasoning}
                </div>
              ) : (
                <div className="flex items-center gap-2 italic text-ink-secondary py-1 font-mono text-[12px]">
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
