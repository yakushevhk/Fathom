import { rmSync, mkdirSync } from "node:fs";
import { beforeEach, describe, expect, it } from "vitest";
import { DATA_DIR } from "./config.ts";
import { closeMessageDb } from "./message-db.ts";
import { saveFact, listFacts } from "./structured-memory.ts";
import { executeBotSleep, getBotSleepState } from "./memory-sleep.ts";

describe("memory-sleep", () => {
  beforeEach(() => {
    closeMessageDb();
    rmSync(DATA_DIR, { recursive: true, force: true });
    mkdirSync(DATA_DIR, { recursive: true });
  });

  it("consolidates memory during sleep: prunes duplicates and adjusts confidence", async () => {
    // Record facts with duplicate entities
    saveFact("bot-1", {
      category: "preference",
      entity: "city",
      fact: "Lives in Moscow.",
    });

    const initialFacts = listFacts("bot-1");
    expect(initialFacts).toHaveLength(1);

    // Initial sleep state
    const beforeSleep = getBotSleepState("bot-1");
    expect(beforeSleep.sleepCycleCount).toBe(0);
    expect(beforeSleep.lastSleptAt).toBeNull();

    // Execute sleep
    const report = await executeBotSleep("bot-1");
    expect(report.botId).toBe("bot-1");
    expect(report.factsProcessed).toBe(1);
    expect(report.summary).toContain("Сон завершен");

    const afterSleep = getBotSleepState("bot-1");
    expect(afterSleep.sleepCycleCount).toBe(1);
    expect(afterSleep.lastSleptAt).toBeTypeOf("number");
  });

  it("decays confidence on 30+ day unreinforced facts without resetting updatedAt", async () => {
    const fortyDaysAgo = Date.now() - 40 * 24 * 60 * 60_000;
    saveFact("bot-1", {
      category: "preference",
      entity: "framework",
      fact: "Uses jQuery.",
      confidence: 1.0,
      updatedAt: fortyDaysAgo,
    });

    const before = listFacts("bot-1");
    expect(before[0].confidence).toBe(1.0);
    expect(before[0].updatedAt).toBe(fortyDaysAgo);

    // Execute sleep
    const report = await executeBotSleep("bot-1");
    expect(report.confidenceAdjusted).toBe(1);

    const after = listFacts("bot-1");
    expect(after[0].confidence).toBe(0.8);
    // Age timestamp must be preserved so repeated sleeps don't reset age to 0 days
    expect(after[0].updatedAt).toBe(fortyDaysAgo);
  });
});
