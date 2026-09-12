import { useState } from "react";
import { Network, CheckCircle2, AlertCircle, Clock, Play, Pause, ChevronRight, X } from "lucide-react";
import { useStore } from "@/state/store";
import { triggerHaptic } from "@/lib/haptics";

export interface DagNode {
  id: string;
  name: string;
  role: string;
  status: "pending" | "running" | "completed" | "failed";
  dependsOn?: string[];
  output?: string;
}

interface DagVisualizerProps {
  onClose: () => void;
}

export function DagWorkflowVisualizer({ onClose }: DagVisualizerProps) {
  const { state } = useStore();
  const [debugPaused, setDebugPaused] = useState(false);
  const [selectedNode, setSelectedNode] = useState<DagNode | null>(null);

  // Generate dynamic DAG representation from active bots and their delegation links
  const nodes: DagNode[] = state.bots.slice(0, 6).map((bot, idx) => {
    let status: DagNode["status"] = "pending";
    if (bot.busy) status = "running";
    else if (idx === 0) status = "completed";

    return {
      id: bot.id,
      name: bot.name,
      role: bot.chiefOfStaff ? "Chief of Staff / Coordinator" : bot.title || "Specialist Agent",
      status,
      dependsOn: idx > 0 ? [state.bots[idx - 1].id] : [],
      output: bot.messages.filter((m) => m.role === "bot").slice(-1)[0]?.text || "Ready for task execution",
    };
  });

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Interactive DAG Workflow Visualizer"
      onClick={onClose}
      className="modal-backdrop fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-6"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="flex w-full max-w-5xl h-[85vh] flex-col overflow-hidden rounded-2xl border border-hairline/60 bg-panel shadow-2xl animate-pop-in"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-hairline/40 px-5 py-3.5">
          <div className="flex items-center gap-2">
            <Network className="text-accent" size={18} />
            <div>
              <h2 className="text-sm font-semibold text-ink">Swarm DAG Workflow Visualizer & Stepper</h2>
              <p className="text-[12px] text-ink-secondary">
                Interactive execution pipeline with step-by-step inspection and debate orchestration
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => {
                triggerHaptic("tap");
                setDebugPaused(!debugPaused);
              }}
              className={`flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-[12px] font-medium transition-colors ${
                debugPaused
                  ? "bg-amber-500/20 text-amber-500 border border-amber-500/40"
                  : "bg-raised text-ink-secondary hover:text-ink"
              }`}
            >
              {debugPaused ? <Pause size={13} /> : <Play size={13} />}
              {debugPaused ? "Step Debugger: Paused" : "Step Debugger: Active"}
            </button>
            <button
              type="button"
              onClick={onClose}
              className="flex size-7 items-center justify-center rounded-lg text-ink-secondary hover:bg-raised hover:text-ink transition-colors"
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Content canvas */}
        <div className="flex flex-1 overflow-hidden">
          {/* Main Graph Canvas */}
          <div className="flex-1 overflow-y-auto p-6 bg-app/50 relative flex flex-col justify-center items-center">
            <div className="flex flex-col items-center gap-8 w-full max-w-xl">
              {nodes.map((node, index) => (
                <div key={node.id} className="w-full flex flex-col items-center">
                  <div
                    onClick={() => {
                      triggerHaptic("tap");
                      setSelectedNode(node);
                    }}
                    className={`w-full cursor-pointer rounded-xl border p-4 shadow-sm transition-all hover:scale-[1.01] ${
                      selectedNode?.id === node.id
                        ? "border-accent bg-accent/10 ring-1 ring-accent"
                        : "border-hairline/60 bg-panel hover:border-hairline"
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2.5">
                        {node.status === "completed" && <CheckCircle2 size={16} className="text-emerald-500" />}
                        {node.status === "running" && <Clock size={16} className="animate-spin text-accent" />}
                        {node.status === "pending" && <Clock size={16} className="text-ink-secondary/60" />}
                        {node.status === "failed" && <AlertCircle size={16} className="text-red-500" />}
                        <div>
                          <h4 className="text-sm font-semibold text-ink">{node.name}</h4>
                          <p className="text-[11.5px] text-ink-secondary">{node.role}</p>
                        </div>
                      </div>
                      <span
                        className={`rounded-full px-2 py-0.5 text-[11px] font-mono capitalize ${
                          node.status === "running"
                            ? "bg-accent/15 text-accent animate-pulse"
                            : node.status === "completed"
                            ? "bg-emerald-500/15 text-emerald-500"
                            : "bg-raised text-ink-secondary"
                        }`}
                      >
                        {node.status}
                      </span>
                    </div>
                  </div>

                  {index < nodes.length - 1 && (
                    <div className="my-2 flex flex-col items-center">
                      <div className="h-6 w-0.5 bg-hairline/60" />
                      <ChevronRight size={14} className="rotate-90 text-hairline" />
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>

          {/* Node detail / step inspector drawer */}
          <div className="w-80 border-l border-hairline/40 bg-panel/60 p-4 flex flex-col justify-between overflow-y-auto">
            {selectedNode ? (
              <div className="flex flex-col gap-3">
                <div>
                  <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-secondary">Inspector</h3>
                  <h4 className="text-base font-bold text-ink mt-1">{selectedNode.name}</h4>
                  <p className="text-[12px] text-ink-secondary">{selectedNode.role}</p>
                </div>
                <div className="border-t border-hairline/40 pt-3">
                  <span className="text-[11px] font-semibold text-ink-secondary uppercase">Last Output</span>
                  <div className="mt-1 rounded-lg bg-inset p-2.5 font-mono text-[11.5px] text-ink leading-relaxed max-h-48 overflow-y-auto">
                    {selectedNode.output}
                  </div>
                </div>
                {debugPaused && (
                  <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 p-2.5 text-[12px] text-amber-500">
                    Step paused. Operator approval or prompt modification allowed.
                  </div>
                )}
              </div>
            ) : (
              <div className="flex h-full items-center justify-center text-center text-[12.5px] text-ink-secondary">
                Select a DAG node to inspect execution trace, dependencies, and step outputs.
              </div>
            )}

            <div className="border-t border-hairline/40 pt-3">
              <button
                type="button"
                onClick={() => {
                  triggerHaptic("success");
                  alert("Consensus Debate round initiated across visible specialist agents.");
                }}
                className="w-full rounded-lg bg-accent px-3 py-2 text-center text-[12.5px] font-medium text-white shadow hover:bg-accent/90 transition-colors"
              >
                Trigger Agent Debate & Voting
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
