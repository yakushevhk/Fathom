import { useState } from "react";
import { Play, Check, AlertCircle, Clock, Code, Wrench, X } from "lucide-react";
import { api } from "@/state/store";
import { triggerHaptic } from "@/lib/haptics";

interface McpTool {
  name: string;
  description?: string;
  inputSchema?: Record<string, unknown>;
}

interface McpToolInspectorProps {
  serverName: string;
  tools?: McpTool[];
  onClose: () => void;
}

export function McpToolInspector({ serverName, tools = [], onClose }: { serverName: string; tools?: McpTool[]; onClose: () => void }) {
  const [selectedTool, setSelectedTool] = useState<McpTool | null>(tools[0] ?? null);
  const [jsonInput, setJsonInput] = useState<string>("{}");
  const [executing, setExecuting] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; data?: unknown; durationMs?: number; error?: string } | null>(null);

  const handleSelectTool = (tool: McpTool) => {
    triggerHaptic("selection");
    setSelectedTool(tool);
    setResult(null);
    // Generate sample template from schema properties if available
    const schema = tool.inputSchema as { properties?: Record<string, { type?: string; description?: string }> } | undefined;
    if (schema?.properties) {
      const template: Record<string, unknown> = {};
      for (const [key, prop] of Object.entries(schema.properties)) {
        template[key] = prop.type === "string" ? "" : prop.type === "number" ? 0 : prop.type === "boolean" ? false : null;
      }
      setJsonInput(JSON.stringify(template, null, 2));
    } else {
      setJsonInput("{}");
    }
  };

  const executeToolTest = async () => {
    if (!selectedTool) return;
    triggerHaptic("tap");
    setExecuting(true);
    setResult(null);
    const start = performance.now();

    try {
      const parsedArgs = JSON.parse(jsonInput || "{}");
      const res = await api(`/api/mcp/servers/${encodeURIComponent(serverName)}/test`, {
        method: "POST",
        body: JSON.stringify({
          tool: selectedTool.name,
          arguments: parsedArgs,
        }),
      });
      const durationMs = Math.round(performance.now() - start);
      setResult({ ok: true, data: res, durationMs });
    } catch (err: unknown) {
      const durationMs = Math.round(performance.now() - start);
      setResult({
        ok: false,
        error: err instanceof Error ? err.message : String(err),
        durationMs,
      });
    } finally {
      setExecuting(false);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="MCP Tool Inspector & Sandbox"
      onClick={onClose}
      className="modal-backdrop fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-6"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="flex w-full max-w-4xl h-[80vh] flex-col overflow-hidden rounded-2xl border border-hairline/60 bg-panel shadow-2xl animate-pop-in"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-hairline/40 px-5 py-3.5">
          <div className="flex items-center gap-2">
            <Wrench className="text-accent" size={18} />
            <div>
              <h2 className="text-sm font-semibold text-ink">MCP Tool Inspector & Test Sandbox</h2>
              <p className="text-[12px] text-ink-secondary">Server: <span className="font-mono text-accent">{serverName}</span> ({tools.length} discovered tools)</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="flex size-7 items-center justify-center rounded-lg text-ink-secondary hover:bg-raised hover:text-ink transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content Layout */}
        <div className="flex flex-1 min-h-0 divide-x divide-hairline/40">
          {/* Tool List Sidebar */}
          <div className="w-64 shrink-0 overflow-y-auto p-2 bg-card/30">
            <div className="px-2 py-1 text-[10.5px] font-semibold text-ink-secondary uppercase tracking-wider">
              Discovered Tools
            </div>
            {tools.length === 0 ? (
              <div className="p-3 text-xs text-ink-secondary">No tools declared by server</div>
            ) : (
              <div className="space-y-0.5">
                {tools.map((tool) => (
                  <button
                    key={tool.name}
                    type="button"
                    onClick={() => handleSelectTool(tool)}
                    className={`w-full text-left rounded-lg px-2.5 py-1.5 text-xs transition-colors truncate block ${
                      selectedTool?.name === tool.name
                        ? "bg-accent/15 text-accent font-medium"
                        : "text-ink hover:bg-raised/60"
                    }`}
                  >
                    <div className="font-mono">{tool.name}</div>
                    {tool.description && (
                      <div className="text-[10.5px] text-ink-secondary truncate font-sans">
                        {tool.description}
                      </div>
                    )}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Test Runner Main View */}
          <div className="flex flex-1 min-w-0 flex-col overflow-y-auto p-4 space-y-4">
            {selectedTool ? (
              <>
                <div>
                  <h3 className="text-base font-medium font-mono text-ink">{selectedTool.name}</h3>
                  <p className="text-xs text-ink-secondary mt-1">{selectedTool.description || "No description provided."}</p>
                </div>

                {/* Input Parameters Sandbox */}
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-xs font-semibold text-ink-secondary">
                    <span>Arguments (JSON Payload)</span>
                    <button
                      type="button"
                      onClick={() => setJsonInput("{}")}
                      className="text-[11px] text-ink-secondary hover:text-ink"
                    >
                      Clear
                    </button>
                  </div>
                  <textarea
                    value={jsonInput}
                    onChange={(e) => setJsonInput(e.target.value)}
                    rows={6}
                    className="w-full rounded-xl border border-hairline/40 bg-inset p-2.5 font-mono text-xs text-ink focus:outline-none focus:ring-1 focus:ring-accent"
                  />
                </div>

                {/* Run Action */}
                <div>
                  <button
                    type="button"
                    disabled={executing}
                    onClick={() => void executeToolTest()}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-accent text-white px-3.5 py-1.5 text-xs font-medium hover:bg-accent/90 disabled:opacity-50 transition-colors shadow-xs"
                  >
                    <Play size={13} />
                    <span>{executing ? "Executing Tool..." : "Run Test"}</span>
                  </button>
                </div>

                {/* Response / Result Output */}
                {result && (
                  <div className="space-y-2 border-t border-hairline/40 pt-3">
                    <div className="flex items-center justify-between text-xs">
                      <div className="flex items-center gap-1.5">
                        {result.ok ? (
                          <Check size={14} className="text-success" />
                        ) : (
                          <AlertCircle size={14} className="text-danger" />
                        )}
                        <span className={result.ok ? "text-success font-medium" : "text-danger font-medium"}>
                          {result.ok ? "Executed Successfully" : "Execution Failed"}
                        </span>
                      </div>
                      {result.durationMs !== undefined && (
                        <div className="flex items-center gap-1 text-[11px] text-ink-secondary">
                          <Clock size={12} />
                          <span>{result.durationMs}ms</span>
                        </div>
                      )}
                    </div>
                    <pre className="max-h-60 overflow-y-auto rounded-xl border border-hairline/40 bg-inset p-3 font-mono text-xs text-ink leading-relaxed">
                      {JSON.stringify(result.ok ? result.data : result.error, null, 2)}
                    </pre>
                  </div>
                )}
              </>
            ) : (
              <div className="flex h-full items-center justify-center text-xs text-ink-secondary">
                Select a tool to inspect schema and test
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
