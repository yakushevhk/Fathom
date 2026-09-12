import { useState } from "react";
import { Brain, ChevronDown, ChevronRight, Sparkles } from "lucide-react";

interface ThinkingAccordionProps {
  reasoning: string;
  isStreaming?: boolean;
}

export function ThinkingAccordion({ reasoning, isStreaming = false }: ThinkingAccordionProps) {
  const [open, setOpen] = useState(false);
  if (!reasoning && !isStreaming) return null;

  const charCount = reasoning.length;
  const estimatedTokens = Math.max(1, Math.round(charCount / 4));

  return (
    <div className="my-2 max-w-3xl overflow-hidden rounded-xl border border-hairline/60 bg-panel/70 backdrop-blur-xs transition-all">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between px-3.5 py-2 text-left text-[12.5px] font-medium text-ink-secondary hover:bg-raised/50 hover:text-ink transition-colors"
      >
        <div className="flex items-center gap-2">
          <Brain size={14} className={isStreaming ? "animate-pulse text-accent" : "text-ink-secondary"} />
          <span>{isStreaming ? "Thinking in progress…" : "Thought process"}</span>
          <span className="rounded-full bg-raised/80 px-2 py-0.5 text-[11px] font-mono text-ink-secondary">
            ~{estimatedTokens} tokens
          </span>
        </div>
        <div className="flex items-center gap-1">
          <span className="text-[11px] text-ink-secondary/70">{open ? "Hide" : "Show"}</span>
          {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </div>
      </button>

      {open && (
        <div className="border-t border-hairline/40 px-3.5 py-3 text-[12.5px] leading-relaxed text-ink/80 font-mono whitespace-pre-wrap max-h-72 overflow-y-auto select-text bg-inset/30">
          {reasoning || (
            <div className="flex items-center gap-2 italic text-ink-secondary">
              <Sparkles size={13} className="animate-spin" />
              Synthesizing internal thought path...
            </div>
          )}
        </div>
      )}
    </div>
  );
}
