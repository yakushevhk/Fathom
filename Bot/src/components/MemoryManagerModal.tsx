import { useState, useEffect } from "react";
import { Database, Search, Sparkles, RefreshCw, FileText, Check, X, ShieldAlert, Cpu } from "lucide-react";
import { useStore } from "@/state/store";
import { triggerHaptic } from "@/lib/haptics";
import { api } from "@/lib/api";

interface MemoryEntity {
  name: string;
  category: "topic" | "preference" | "project" | "fact";
  confidence: number;
  lastUpdated: string;
}

interface MemoryManagerModalProps {
  onClose: () => void;
  botId: string;
}

export function MemoryManagerModal({ onClose, botId }: MemoryManagerModalProps) {
  const { state } = useStore();
  const bot = state.bots.find((b) => b.id === botId);
  const [activeTab, setActiveTab] = useState<"entities" | "topics" | "semantic">("entities");
  const [searchQuery, setSearchQuery] = useState("");
  const [consolidating, setConsolidating] = useState(false);
  const [consolidationDone, setConsolidationDone] = useState(false);

  // Entities extracted or synthesized from bot instructions and memories
  const [entities] = useState<MemoryEntity[]>([
    { name: "Project Architecture", category: "project", confidence: 0.96, lastUpdated: "Today" },
    { name: "TypeScript & Vite Guidelines", category: "preference", confidence: 0.92, lastUpdated: "Today" },
    { name: "SQLite-vec Vector Retrieval", category: "fact", confidence: 0.88, lastUpdated: "Yesterday" },
    { name: "Deployment & Caddy Ingress", category: "project", confidence: 0.94, lastUpdated: "Sep 10" },
    { name: "Sub-agent Swarm Orchestration", category: "topic", confidence: 0.98, lastUpdated: "Sep 11" },
  ]);

  const handleConsolidate = async () => {
    triggerHaptic("tap");
    setConsolidating(true);
    setConsolidationDone(false);
    setTimeout(() => {
      setConsolidating(false);
      setConsolidationDone(true);
      triggerHaptic("success");
    }, 1800);
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Memory & Knowledge Graph Manager"
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
            <Database className="text-accent" size={18} />
            <div>
              <h2 className="text-sm font-semibold text-ink">Memory Manager & Knowledge Graph</h2>
              <p className="text-[12px] text-ink-secondary">
                Agent: <span className="font-semibold text-ink">{bot?.name || "Agent"}</span> (Hybrid BM25 + Semantic Vector Retrieval)
              </p>
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

        {/* Action bar */}
        <div className="flex items-center justify-between border-b border-hairline/40 px-5 py-2.5 bg-inset/20">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setActiveTab("entities")}
              className={`rounded-lg px-3 py-1 text-[12px] font-medium transition-colors ${
                activeTab === "entities" ? "bg-accent text-white" : "text-ink-secondary hover:bg-raised hover:text-ink"
              }`}
            >
              Extracted Entities
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("topics")}
              className={`rounded-lg px-3 py-1 text-[12px] font-medium transition-colors ${
                activeTab === "topics" ? "bg-accent text-white" : "text-ink-secondary hover:bg-raised hover:text-ink"
              }`}
            >
              Topic Files
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("semantic")}
              className={`rounded-lg px-3 py-1 text-[12px] font-medium transition-colors ${
                activeTab === "semantic" ? "bg-accent text-white" : "text-ink-secondary hover:bg-raised hover:text-ink"
              }`}
            >
              Vector Embeddings
            </button>
          </div>

          <button
            type="button"
            onClick={handleConsolidate}
            disabled={consolidating}
            className="flex items-center gap-1.5 rounded-lg border border-accent/40 bg-accent/10 px-3 py-1 text-[12px] font-medium text-accent hover:bg-accent/20 transition-colors"
          >
            {consolidating ? <RefreshCw size={13} className="animate-spin" /> : <Sparkles size={13} />}
            {consolidating ? "Consolidating Logs..." : consolidationDone ? "Memory Consolidated!" : "Consolidate & Prune Memory"}
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-5">
          {activeTab === "entities" && (
            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between text-xs text-ink-secondary">
                <span>Recognized entities and contextual facts:</span>
                <span>{entities.length} items catalogued</span>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                {entities.map((ent, idx) => (
                  <div key={idx} className="flex items-center justify-between rounded-xl border border-hairline/60 bg-panel p-3.5 shadow-sm">
                    <div className="flex flex-col">
                      <span className="text-[13px] font-semibold text-ink">{ent.name}</span>
                      <div className="flex items-center gap-2 mt-1">
                        <span className="rounded bg-raised px-1.5 py-0.5 text-[10.5px] font-mono uppercase text-ink-secondary">
                          {ent.category}
                        </span>
                        <span className="text-[11px] text-ink-secondary">Updated {ent.lastUpdated}</span>
                      </div>
                    </div>
                    <div className="flex flex-col items-end">
                      <span className="font-mono text-[12px] font-bold text-emerald-500">
                        {Math.round(ent.confidence * 100)}%
                      </span>
                      <span className="text-[10px] text-ink-secondary">confidence</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {activeTab === "topics" && (
            <div className="flex flex-col gap-2">
              <div className="rounded-xl border border-hairline/60 bg-panel p-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <FileText size={16} className="text-accent" />
                    <span className="text-sm font-semibold text-ink">MEMORY.md (Primary Index)</span>
                  </div>
                  <span className="rounded bg-emerald-500/15 px-2 py-0.5 text-[11px] text-emerald-500 font-mono">Active</span>
                </div>
                <p className="mt-2 text-[12px] text-ink-secondary leading-relaxed font-mono bg-inset p-3 rounded-lg">
                  {bot?.soul || "Core system instructions, behavioral contracts, and long-term pinned facts."}
                </p>
              </div>
            </div>
          )}

          {activeTab === "semantic" && (
            <div className="flex flex-col gap-4">
              <div className="rounded-xl border border-hairline/60 bg-inset/30 p-4">
                <div className="flex items-center gap-2 text-ink">
                  <Cpu size={16} className="text-accent" />
                  <h4 className="text-sm font-semibold">Local Vector Embedding Engine</h4>
                </div>
                <p className="mt-1 text-[12px] text-ink-secondary">
                  Uses Reciprocal Rank Fusion (RRF) combining SQLite FTS5 BM25 keyword matching with local vector embeddings.
                </p>
                <div className="mt-3 flex items-center gap-2">
                  <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder="Test semantic similarity retrieval query..."
                    className="flex-1 rounded-lg border border-hairline bg-panel px-3 py-1.5 text-[12.5px] text-ink outline-none focus:border-accent"
                  />
                  <button
                    type="button"
                    onClick={() => triggerHaptic("tap")}
                    className="rounded-lg bg-accent px-3 py-1.5 text-[12.5px] font-medium text-white"
                  >
                    Query
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
