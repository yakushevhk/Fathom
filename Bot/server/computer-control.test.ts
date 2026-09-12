// The who-is-driving record. What these tests pin is the authority split:
// the person's three moves (take, release, dismiss) all work, the bot's one
// move (requestHelp) never grants anything, and a release settles the help
// request in the same change the person made.
import { describe, expect, it } from "vitest";

import { ComputerControl, type ControlSnapshot } from "./computer-control.ts";

function tracked() {
  const changes: Array<{ botId: string; snapshot: ControlSnapshot }> = [];
  const control = new ComputerControl((botId, snapshot) => changes.push({ botId, snapshot }));
  return { control, changes };
}

describe("computer control", () => {
  it("starts disengaged for an unknown bot", () => {
    const { control } = tracked();
    expect(control.snapshot("b1")).toEqual({ held: false, helpReason: null, heldSinceMs: null });
  });

  it("take → held; release → disengaged, each broadcast once", () => {
    const { control, changes } = tracked();
    const held = control.take("b1");
    expect(held.held).toBe(true);
    expect(held.heldSinceMs).not.toBeNull();
    expect(control.release("b1").held).toBe(false);
    expect(changes.map((c) => c.snapshot.held)).toEqual([true, false]);
  });

  it("a second take does not reset how long the hold has lasted", () => {
    let clock = 1000;
    const control = new ComputerControl(() => {}, () => clock);
    control.take("b1");
    clock = 5000;
    expect(control.take("b1").heldSinceMs).toBe(1000);
  });

  it("atomically acquires a workspace lease without exposing its id", () => {
    const { control, changes } = tracked();
    const leaseId = "5b6bbbd2-b88b-4c50-a748-ec87f332662f";
    const acquired = control.acquireLease("b1", leaseId);
    expect(acquired).toMatchObject({ owned: true, acquired: true, snapshot: { held: true } });
    expect(acquired.snapshot).not.toHaveProperty("controlLeaseId");
    expect(JSON.stringify(changes)).not.toContain(leaseId);

    const sameLease = control.acquireLease("b1", leaseId);
    expect(sameLease).toMatchObject({ owned: true, acquired: false });
    expect(changes).toHaveLength(1);
  });

  it("does not acquire or release a hold owned by another surface", () => {
    const { control, changes } = tracked();
    control.take("b1");
    const leaseId = "57c7f3ef-e41d-4adf-bbda-0bd25bb03893";

    expect(control.acquireLease("b1", leaseId)).toMatchObject({
      owned: false,
      acquired: false,
      snapshot: { held: true },
    });
    expect(control.releaseLease("b1", leaseId)).toMatchObject({
      released: false,
      snapshot: { held: true },
    });
    expect(changes.map((change) => change.snapshot.held)).toEqual([true]);
  });

  it("conditionally releases only the matching workspace lease", () => {
    const { control, changes } = tracked();
    const owner = "33e62f3a-89d9-4117-b48a-15f7deae3252";
    const other = "ed602995-306f-480a-8817-e8d8c8fe7d90";
    control.acquireLease("b1", owner);

    expect(control.releaseLease("b1", other).released).toBe(false);
    expect(control.snapshot("b1").held).toBe(true);
    expect(control.releaseLease("b1", owner)).toMatchObject({
      released: true,
      snapshot: { held: false },
    });
    expect(changes.map((change) => change.snapshot.held)).toEqual([true, false]);
  });

  it("requestHelp surfaces the plea but never grants control", () => {
    const { control } = tracked();
    const snapshot = control.requestHelp("b1", "  please log in for me  ");
    expect(snapshot.held).toBe(false);
    expect(snapshot.helpReason).toBe("please log in for me");
  });

  it("an empty reason still reads as a plea", () => {
    const { control } = tracked();
    expect(control.requestHelp("b1", undefined).helpReason).toBe("the bot asked you to take over");
  });

  it("a shouted second reason cannot clobber the one the person is reading", () => {
    const { control } = tracked();
    control.requestHelp("b1", "first");
    expect(control.requestHelp("b1", "second").helpReason).toBe("first");
  });

  it("expires only the help request that owns the timeout", () => {
    const { control, changes } = tracked();
    const first = control.requestHelpLease("b1", "first");
    expect(control.expireHelp("b1", "some-older-request").helpReason).toBe("first");
    expect(changes).toHaveLength(1);
    expect(control.expireHelp("b1", first.requestId).helpReason).toBeNull();
    expect(changes).toHaveLength(2);
  });

  it("an old timeout cannot dismiss a newer plea", () => {
    const { control } = tracked();
    const first = control.requestHelpLease("b1", "first");
    control.dismissHelp("b1");
    const second = control.requestHelpLease("b1", "second");
    expect(second.requestId).not.toBe(first.requestId);
    expect(control.expireHelp("b1", first.requestId).helpReason).toBe("second");
  });

  it("a novel-length reason is cut to card size", () => {
    const { control } = tracked();
    const reason = "x".repeat(2000);
    expect(control.requestHelp("b1", reason).helpReason?.length).toBe(280);
  });

  it("release settles an open help request in the same change", () => {
    const { control } = tracked();
    control.requestHelp("b1", "stuck on a captcha");
    control.take("b1");
    const after = control.release("b1");
    expect(after).toEqual({ held: false, helpReason: null, heldSinceMs: null });
  });

  it("dismiss clears the plea without taking control", () => {
    const { control } = tracked();
    control.requestHelp("b1", "stuck");
    const after = control.dismissHelp("b1");
    expect(after.helpReason).toBeNull();
    expect(after.held).toBe(false);
  });

  it("dismiss while driving keeps the hold", () => {
    const { control } = tracked();
    control.take("b1");
    control.requestHelp("b1", "also this");
    const after = control.dismissHelp("b1");
    expect(after.held).toBe(true);
    expect(after.helpReason).toBeNull();
  });

  it("dismissing nothing is silent — no phantom broadcast", () => {
    const { control, changes } = tracked();
    control.dismissHelp("b1");
    expect(changes).toEqual([]);
  });

  it("bots are independent", () => {
    const { control } = tracked();
    control.take("b1");
    expect(control.snapshot("b2").held).toBe(false);
  });

  it("forget clears a hold and tells the listeners", () => {
    const { control, changes } = tracked();
    control.take("b1");
    control.forget("b1");
    expect(control.snapshot("b1").held).toBe(false);
    expect(changes.at(-1)?.snapshot).toEqual({ held: false, helpReason: null, heldSinceMs: null });
  });

  it("forgetting an unknown bot is silent", () => {
    const { control, changes } = tracked();
    control.forget("ghost");
    expect(changes).toEqual([]);
  });

  describe("human control timeout watchdog", () => {
    it("expires hold in snapshot after HUMAN_CONTROL_TIMEOUT_MS", () => {
      let clock = 1000;
      const changes: Array<{ botId: string; snapshot: ControlSnapshot }> = [];
      const control = new ComputerControl((botId, snapshot) => changes.push({ botId, snapshot }), () => clock);
      
      control.take("b1");
      expect(control.snapshot("b1").held).toBe(true);

      // Advance clock past 10 minutes
      clock += 10 * 60 * 1000 + 1;
      const snap = control.snapshot("b1");
      expect(snap.held).toBe(false);
      expect(snap.heldSinceMs).toBeNull();
      expect(changes.at(-1)?.snapshot.held).toBe(false);
    });

    it("expires timed-out hold on take() so person can re-take cleanly", () => {
      let clock = 1000;
      const control = new ComputerControl(() => {}, () => clock);
      control.take("b1");
      expect(control.snapshot("b1").heldSinceMs).toBe(1000);

      // Advance past timeout
      clock += 15 * 60 * 1000;
      // take() checks snapshot / cleans up and takes fresh control at new time
      const fresh = control.take("b1");
      expect(fresh.held).toBe(true);
      expect(fresh.heldSinceMs).toBe(clock);
    });

    it("expires timed-out hold on acquireLease()", () => {
      let clock = 1000;
      const control = new ComputerControl(() => {}, () => clock);
      control.take("b1");

      clock += 15 * 60 * 1000;
      const leaseResult = control.acquireLease("b1", "lease-123");
      expect(leaseResult.acquired).toBe(true);
      expect(leaseResult.owned).toBe(true);
      expect(leaseResult.snapshot.held).toBe(true);
      expect(leaseResult.snapshot.heldSinceMs).toBe(clock);
    });

    it("clears held state on dismissHelp if hold timed out", () => {
      let clock = 1000;
      const control = new ComputerControl(() => {}, () => clock);

      control.take("b1");
      control.requestHelp("b1", "help me");
      expect(control.snapshot("b1").held).toBe(true);

      clock += 15 * 60 * 1000;
      // Hold timed out. When dismissHelp is called, entry.heldSinceMs should not prevent cleanup
      const after = control.dismissHelp("b1");
      expect(after.held).toBe(false);
      expect(after.helpReason).toBeNull();
    });

    it("clears held state on expireHelp if hold timed out", () => {
      let clock = 1000;
      const { control } = (() => {
        return { control: new ComputerControl(() => {}, () => clock) };
      })();

      control.take("b1");
      const { requestId } = control.requestHelpLease("b1", "help me");

      clock += 15 * 60 * 1000;
      const after = control.expireHelp("b1", requestId);
      expect(after.held).toBe(false);
      expect(after.helpReason).toBeNull();
    });
  });
});
