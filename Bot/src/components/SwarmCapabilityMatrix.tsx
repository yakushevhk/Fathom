import { Check, Shield, X } from "lucide-react";
import { useStore, type Bot } from "@/state/store";
import { BotAvatar } from "@/components/Avatar";
import { triggerHaptic } from "@/lib/haptics";

export function SwarmCapabilityMatrix({ onClose }: { onClose: () => void }) {
  const { state, dispatch } = useStore();
  const visibleBots = state.bots.filter((b) => !b.hidden);

  const getCanCoordinate = (bot: Bot) => {
    const engine = state.instances.find((instance) => instance.instanceId === bot.modelSelection?.instanceId);
    return engine?.capabilities?.agentsMcp === true;
  };

  const toggleChief = (bot: Bot) => {
    triggerHaptic("tap");
    if (!bot.chiefOfStaff && !getCanCoordinate(bot)) return;
    dispatch({
      type: "updateBot",
      botId: bot.id,
      patch: { chiefOfStaff: !bot.chiefOfStaff },
    });
  };

  const toggleApproval = (bot: Bot) => {
    triggerHaptic("tap");
    const nextMode = bot.approvalMode === "auto" ? "ask" : "auto";
    dispatch({
      type: "updateBot",
      botId: bot.id,
      patch: { approvalMode: nextMode },
    });
  };

  const togglePeerComms = (bot: Bot) => {
    triggerHaptic("tap");
    if (!bot.approvePeerComms && !getCanCoordinate(bot)) return;
    dispatch({
      type: "updateBot",
      botId: bot.id,
      patch: { approvePeerComms: !bot.approvePeerComms },
    });
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Swarm Capability Matrix"
      onClick={onClose}
      className="modal-backdrop fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-6"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="flex w-full max-w-4xl flex-col overflow-hidden rounded-2xl border border-hairline/60 bg-panel shadow-2xl animate-pop-in"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-hairline/40 px-5 py-3.5">
          <div className="flex items-center gap-2">
            <Shield className="text-accent" size={18} />
            <div>
              <h2 className="text-sm font-semibold text-ink">Swarm Capability & Permissions Matrix</h2>
              <p className="text-[12px] text-ink-secondary">
                Audit and toggle permissions and access levels across all bots in one place
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

        {/* Matrix Table */}
        <div className="max-h-[65vh] overflow-x-auto overflow-y-auto p-4">
          <table className="w-full text-left text-xs border-collapse">
            <thead>
              <tr className="border-b border-hairline/40 text-[11px] font-semibold text-ink-secondary uppercase tracking-wider">
                <th className="py-2.5 px-3">Agent / Bot</th>
                <th className="py-2.5 px-3">Model</th>
                <th className="py-2.5 px-3 text-center">Chief of Staff</th>
                <th className="py-2.5 px-3 text-center">Approval Mode</th>
                <th className="py-2.5 px-3 text-center">Peer Delegation</th>
                <th className="py-2.5 px-3 text-center">MCP Servers</th>
                <th className="py-2.5 px-3 text-center">Quick Action</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-hairline/30 text-ink">
              {visibleBots.map((bot) => (
                <tr key={bot.id} className="hover:bg-raised/40 transition-colors">
                  <td className="py-3 px-3">
                    <div className="flex items-center gap-2.5">
                      <BotAvatar bot={bot} state="idle" size={28} />
                      <div className="min-w-0">
                        <div className="font-medium text-ink truncate max-w-[140px]">{bot.name}</div>
                        <div className="text-[11px] text-ink-secondary truncate max-w-[140px]">
                          {bot.title || "Specialist"}
                        </div>
                      </div>
                    </div>
                  </td>

                  <td className="py-3 px-3">
                    <span className="rounded bg-raised px-1.5 py-0.5 font-mono text-[11px] text-ink-secondary">
                      {bot.modelSelection?.model || "default"}
                    </span>
                  </td>

                  <td className="py-3 px-3 text-center">
                    {(() => {
                      const canCoord = getCanCoordinate(bot);
                      const disabled = !bot.chiefOfStaff && !canCoord;
                      return (
                        <button
                          type="button"
                          disabled={disabled}
                          title={disabled ? "This engine cannot contact teammates" : undefined}
                          onClick={() => toggleChief(bot)}
                          className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium border transition-colors ${
                            disabled
                              ? "opacity-50 cursor-not-allowed border-hairline/20 bg-inset text-ink-secondary"
                              : bot.chiefOfStaff
                              ? "border-accent/40 bg-accent/15 text-accent"
                              : "border-hairline/30 bg-inset text-ink-secondary hover:text-ink"
                          }`}
                        >
                          {bot.chiefOfStaff ? (
                            <>
                              <Check size={11} /> Chief
                            </>
                          ) : (
                            "Member"
                          )}
                        </button>
                      );
                    })()}
                  </td>

                  <td className="py-3 px-3 text-center">
                    <button
                      type="button"
                      onClick={() => toggleApproval(bot)}
                      className={`rounded-full px-2.5 py-0.5 text-[11px] font-medium border transition-colors ${
                        bot.approvalMode === "full"
                          ? "border-danger/40 bg-danger/15 text-danger"
                          : bot.approvalMode === "auto"
                          ? "border-success/40 bg-success/15 text-success"
                          : "border-warning/40 bg-warning/15 text-warning"
                      }`}
                    >
                      {bot.approvalMode || "auto"}
                    </button>
                  </td>

                  <td className="py-3 px-3 text-center">
                    {(() => {
                      const canCoord = getCanCoordinate(bot);
                      const disabled = bot.approvePeerComms && !canCoord;
                      return (
                        <button
                          type="button"
                          disabled={disabled}
                          title={disabled ? "This engine cannot contact other bots" : undefined}
                          onClick={() => togglePeerComms(bot)}
                          className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium border transition-colors ${
                            disabled
                              ? "opacity-50 cursor-not-allowed border-hairline/20 bg-inset text-ink-secondary"
                              : !bot.approvePeerComms
                              ? "border-accent/40 bg-accent/15 text-accent"
                              : "border-hairline/30 bg-inset text-ink-secondary hover:text-ink"
                          }`}
                        >
                          {!bot.approvePeerComms ? "Autonomous" : "Ask"}
                        </button>
                      );
                    })()}
                  </td>

                  <td className="py-3 px-3 text-center">
                    <span className="rounded-full bg-raised px-2 py-0.5 text-[11px] text-ink-secondary font-mono">
                      {!bot.mcpServers
                        ? "All Servers"
                        : `${bot.mcpServers.length} Mounted`}
                    </span>
                  </td>

                  <td className="py-3 px-3 text-center">
                    <button
                      type="button"
                      onClick={() => {
                        triggerHaptic("tap");
                        dispatch({ type: "select", id: bot.id });
                        onClose();
                      }}
                      className="rounded bg-control hover:bg-raised px-2 py-1 text-[11px] font-medium text-ink transition-colors"
                    >
                      Open Chat
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between border-t border-hairline/40 px-5 py-3 bg-raised/20 text-xs text-ink-secondary">
          <span>{visibleBots.length} active bots configured in this workspace</span>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg bg-control hover:bg-raised px-3 py-1.5 text-xs font-medium text-ink transition-colors"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
