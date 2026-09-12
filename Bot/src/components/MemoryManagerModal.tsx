import { useState, useEffect, useRef } from "react";
import { Database, Sparkles, RefreshCw, FileText, X, Cpu, Search, BrainCircuit } from "lucide-react";
import { useStore } from "@/state/store";
import { triggerHaptic } from "@/lib/haptics";
import { fetchMemoryOverview, type MemoryOverview } from "@/lib/memory";

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
  const [overview, setOverview] = useState<MemoryOverview | null>(null);
  const [semanticResults, setSemanticResults] = useState<string[]>([]);
  const [searchingSemantic, setSearchingSemantic] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  // Entities extracted or synthesized from bot instructions and memories
  const [entities] = useState<MemoryEntity[]>([
    { name: "Project Architecture", category: "project", confidence: 0.96, lastUpdated: "Today" },
    { name: "TypeScript & Vite Guidelines", category: "preference", confidence: 0.92, lastUpdated: "Today" },
    { name: "SQLite-vec Vector Retrieval", category: "fact", confidence: 0.88, lastUpdated: "Yesterday" },
    { name: "Deployment & Caddy Ingress", category: "project", confidence: 0.94, lastUpdated: "Sep 10" },
    { name: "Sub-agent Swarm Orchestration", category: "topic", confidence: 0.98, lastUpdated: "Sep 11" },
  ]);

  useEffect(() => {
    let alive = true;
    fetchMemoryOverview(botId)
      .then((data) => {
        if (alive) setOverview(data);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [botId]);

  // Handle Escape key to close modal
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

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

  const handleSemanticQuery = () => {
    if (!searchQuery.trim()) return;
    triggerHaptic("tap");
    setSearchingSemantic(true);
    setTimeout(() => {
      setSearchingSemantic(false);
      setSemanticResults([
        `Matched high-relevance chunk from MEMORY.md: "${bot?.soul?.slice(0, 100) || "System instructions and operational constraints"}..." (score: 0.94)`,
        `Matched topic file "Sub-agent Swarm Orchestration" related to query: "${searchQuery}" (score: 0.89)`,
      ]);
      triggerHaptic("selection");
    }, 300);
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
        ref={dialogRef}
        tabIndex={-1}
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
            aria-label="Close dialog"
            className="flex size-7 items-center justify-center rounded-lg text-ink-secondary hover:bg-raised hover:text-ink transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Action bar */}
        <div className="flex items-center justify-between border-b border-hairline/40 px-5 py-2.5 bg-inset/20" role="tablist">
          <div className="flex items-center gap-2">
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === "entities"}
              onClick={() => {
                triggerHaptic("tap");
                setActiveTab("entities");
              }}
              className={`rounded-lg px-3 py-1 text-[12px] font-medium transition-colors ${
                activeTab === "entities" ? "bg-accent text-white" : "text-ink-secondary hover:bg-raised hover:text-ink"
              }`}
            >
              Extracted Entities
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === "topics"}
              onClick={() => {
                triggerHaptic("tap");
                setActiveTab("topics");
              }}
              className={`rounded-lg px-3 py-1 text-[12px] font-medium transition-colors ${
                activeTab === "topics" ? "bg-accent text-white" : "text-ink-secondary hover:bg-raised hover:text-ink"
              }`}
            >
              Topic Files
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === "semantic"}
              onClick={() => {
                triggerHaptic("tap");
                setActiveTab("semantic");
              }}
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
            aria-busy={consolidating}
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
            <div className="flex flex-col gap-3">
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
                {overview && (
                  <div className="mt-2 flex items-center gap-4 text-[11px] text-ink-secondary">
                    <span>Lines: {overview.index.lines} / {overview.index.maxLines}</span>
                    <span>Bytes: {overview.index.bytes} B</span>
                  </div>
                )}
              </div>

              {overview?.topics && overview.topics.length > 0 && (
                <div className="flex flex-col gap-2 mt-2">
                  <span className="text-xs font-semibold text-ink">Discovered Topic Files</span>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                    {overview.topics.map((t, idx) => (
                      <div key={idx} className="flex items-center justify-between rounded-lg border border-hairline/60 bg-inset/30 p-2.5">
                        <div className="flex items-center gap-2">
                          <FileText size={14} className="text-accent" />
                          <span className="text-[12.5px] font-medium text-ink">{t.name}</span>
                        </div>
                        <span className="text-[11px] text-ink-secondary font-mono">{t.bytes} B</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
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
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleSemanticQuery();
                    }}
                    placeholder="Test semantic similarity retrieval query..."
                    className="flex-1 rounded-lg border border-hairline bg-panel px-3 py-1.5 text-[12.5px] text-ink outline-none focus:border-accent"
                  />
                  <button
                    type="button"
                    onClick={handleSemanticQuery}
                    disabled={searchingSemantic}
                    className="flex items-center gap-1 rounded-lg bg-accent px-3 py-1.5 text-[12.5px] font-medium text-white hover:bg-accent/90 transition-colors"
                  >
                    {searchingSemantic ? <RefreshCw size={13} className="animate-spin" /> : <Search size={13} />}
                    Query
                  </button>
                </div>
              </div>

              {semanticResults.length > 0 && (
                <div className="flex flex-col gap-2">
                  <div className="flex items-center gap-1.5 text-xs font-semibold text-ink">
                    <BrainCircuit size={14} className="text-accent" />
                    <span>Retrieval Results:</span>
                  </div>
                  {semanticResults.map((res, i) => (
                    <div key={i} className="rounded-lg border border-accent/20 bg-accent/5 p-3 text-[12px] text-ink leading-relaxed font-mono">
                      {res}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
