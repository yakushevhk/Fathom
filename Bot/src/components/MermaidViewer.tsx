import { useEffect, useRef, useState } from "react";
import mermaid from "mermaid";
import { AlertTriangle, Check, Copy } from "lucide-react";
import { triggerHaptic } from "@/lib/haptics";

interface MermaidViewerProps {
  code: string;
}

let mermaidInitialized = false;

function initMermaidOnce() {
  if (!mermaidInitialized) {
    mermaid.initialize({
      startOnLoad: false,
      theme: "dark",
      themeVariables: {
        darkMode: true,
        background: "transparent",
        primaryColor: "#3b82f6",
        primaryTextColor: "#f3f4f6",
        primaryBorderColor: "#60a5fa",
        lineColor: "#9ca3af",
        secondaryColor: "#1e293b",
        tertiaryColor: "#0f172a",
      },
      securityLevel: "loose",
      fontFamily: "inherit",
    });
    mermaidInitialized = true;
  }
}

export function MermaidViewer({ code }: MermaidViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [svg, setSvg] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<"diagram" | "code">("diagram");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    initMermaidOnce();
    let cancelled = false;

    const renderDiagram = async () => {
      try {
        setError(null);
        const id = `mermaid-${Math.random().toString(36).substring(2, 9)}`;
        const { svg: renderedSvg } = await mermaid.render(id, code.trim());
        if (!cancelled) {
          setSvg(renderedSvg);
        }
      } catch (err: unknown) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }
    };

    renderDiagram();
    return () => {
      cancelled = true;
    };
  }, [code]);

  const copyCode = () => {
    triggerHaptic("tap");
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <div className="my-3 overflow-hidden rounded-xl border border-hairline/40 bg-inset shadow-xs">
      <div className="flex items-center justify-between border-b border-hairline/30 bg-raised/50 px-3 py-1.5 text-xs">
        <div className="flex items-center gap-2">
          <span className="font-semibold text-accent tracking-wide uppercase text-[10.5px]">
            Mermaid Diagram
          </span>
          <div className="inline-flex rounded border border-hairline/40 bg-raised p-0.5 text-[11px]">
            <button
              type="button"
              onClick={() => {
                triggerHaptic("selection");
                setViewMode("diagram");
              }}
              className={`rounded px-1.5 py-0.5 transition-colors ${
                viewMode === "diagram" ? "bg-accent/15 text-accent font-medium" : "text-ink-secondary hover:text-ink"
              }`}
            >
              Diagram
            </button>
            <button
              type="button"
              onClick={() => {
                triggerHaptic("selection");
                setViewMode("code");
              }}
              className={`rounded px-1.5 py-0.5 transition-colors ${
                viewMode === "code" ? "bg-accent/15 text-accent font-medium" : "text-ink-secondary hover:text-ink"
              }`}
            >
              Code
            </button>
          </div>
        </div>
        <button
          type="button"
          onClick={copyCode}
          className="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[11px] text-ink-secondary hover:bg-raised hover:text-ink transition-colors"
          title="Copy Mermaid Code"
        >
          {copied ? (
            <>
              <Check size={12} className="text-success" />
              <span className="text-success">Copied!</span>
            </>
          ) : (
            <>
              <Copy size={12} />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>

      {viewMode === "diagram" ? (
        <div className="p-4 flex items-center justify-center overflow-x-auto min-h-[140px] bg-card/40">
          {error ? (
            <div className="flex flex-col items-center gap-2 p-3 text-center text-xs text-danger">
              <AlertTriangle size={20} />
              <span className="font-medium">Failed to render Mermaid diagram</span>
              <pre className="max-w-md text-[11px] opacity-80 whitespace-pre-wrap">{error}</pre>
            </div>
          ) : svg ? (
            <div
              ref={containerRef}
              className="max-w-full [&_svg]:max-w-full [&_svg]:h-auto transition-all"
              dangerouslySetInnerHTML={{ __html: svg }}
            />
          ) : (
            <span className="text-xs text-ink-secondary animate-pulse">Rendering diagram...</span>
          )}
        </div>
      ) : (
        <pre className="p-3 text-[12.5px] leading-relaxed overflow-x-auto text-ink font-mono bg-inset">
          {code}
        </pre>
      )}
    </div>
  );
}
