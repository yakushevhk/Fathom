import { rmSync, mkdirSync } from "node:fs";
import { beforeEach, describe, expect, it } from "vitest";
import { DATA_DIR } from "./config.ts";
import { closeMessageDb } from "./message-db.ts";
import {
  saveFact,
  listFacts,
  deleteFact,
  formatFactsAsPromptSection,
} from "./structured-memory.ts";

describe("structured-memory", () => {
  beforeEach(() => {
    closeMessageDb();
    rmSync(DATA_DIR, { recursive: true, force: true });
    mkdirSync(DATA_DIR, { recursive: true });
  });

  it("saves, lists, and formats categorized facts for a bot", () => {
    const fact = saveFact("bot-1", {
      category: "preference",
      entity: "user",
      fact: "Prefers concise TypeScript code without any tiny functions.",
    });

    expect(fact.id).toBeDefined();
    expect(fact.botId).toBe("bot-1");

    const facts = listFacts("bot-1");
    expect(facts).toHaveLength(1);
    expect(facts[0].fact).toContain("concise TypeScript");

    const formatted = formatFactsAsPromptSection("bot-1");
    expect(formatted).toContain("[preference] user:");

    // Scoping to botId: another bot sees 0 facts
    expect(listFacts("bot-2")).toEqual([]);

    deleteFact("bot-1", fact.id);
    expect(listFacts("bot-1")).toEqual([]);
  });

  it("deduplicates repeated saves of the same natural key (botId, category, entity)", () => {
    const first = saveFact("bot-1", {
      category: "preference",
      entity: "editor",
      fact: "Uses VSCode.",
    });
    expect(first.fact).toBe("Uses VSCode.");

    // Save again without ID: should update in place rather than inserting a duplicate
    const second = saveFact("bot-1", {
      category: "preference",
      entity: "editor",
      fact: "Switched to Cursor / Parallel.",
    });
    expect(second.id).toBe(first.id);

    const facts = listFacts("bot-1");
    expect(facts).toHaveLength(1);
    expect(facts[0].fact).toBe("Switched to Cursor / Parallel.");
  });

  it("enforces prompt capacity limits and newline stripping", () => {
    saveFact("bot-1", {
      category: "system",
      entity: "multiline",
      fact: "Line 1\nLine 2\r\nLine 3",
    });
    const formatted = formatFactsAsPromptSection("bot-1");
    expect(formatted).not.toContain("\nLine 2");
    expect(formatted).toContain("Line 1 Line 2 Line 3");
  });
});
