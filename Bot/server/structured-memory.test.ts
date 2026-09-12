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
});
