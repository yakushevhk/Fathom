// Modular router for memory facts and search API endpoints.
import type { RouteContext } from "./context.ts";
import { executeBotSleep, getBotSleepState } from "../memory-sleep.ts";
import { saveFact, listFacts, deleteFact, type MemoryFact } from "../structured-memory.ts";
import { executeWebSearch } from "../web-search.ts";

const VALID_CATEGORIES: Record<MemoryFact["category"], true> = {
  preference: true,
  decision: true,
  entity: true,
  system: true,
  project: true,
};
const FACTS_PATH = /^\/api\/bots\/([\w-]+)\/facts$/;
const FACT_ITEM_PATH = /^\/api\/bots\/([\w-]+)\/facts\/([\w-]+)$/;
const BOT_SLEEP_PATH = /^\/api\/bots\/([\w-]+)\/sleep$/;

export async function handleMemoryAndSearchRoutes(ctx: RouteContext): Promise<boolean> {
  const { path, method, url, res, json, readBody, store } = ctx;

  const factsMatch = FACTS_PATH.exec(path);
  if (factsMatch) {
    const botId = factsMatch[1];
    if (!store.bot(botId)) {
      json(res, 404, { error: "no such bot" });
      return true;
    }
    if (method === "GET") {
      const rawCategory = url.searchParams.get("category");
      const category = rawCategory && rawCategory in VALID_CATEGORIES
        ? (rawCategory as MemoryFact["category"])
        : undefined;
      const facts = listFacts(botId, category);
      json(res, 200, { ok: true, facts });
      return true;
    }

    if (method === "POST") {
      try {
        const raw = await readBody();
        const body = JSON.parse(raw) as {
          category?: string;
          entity?: string;
          fact?: string;
          confidence?: number;
        };
        if (!body.category || !(body.category in VALID_CATEGORIES)) {
          json(res, 400, { error: "category must be one of: preference, decision, entity, system, project" });
          return true;
        }
        const entity = String(body.entity ?? "").trim();
        const fact = String(body.fact ?? "").trim();
        if (!entity || !fact) {
          json(res, 400, { error: "entity and fact are required and cannot be blank" });
          return true;
        }
        if (entity.length > 120 || fact.length > 500) {
          json(res, 400, { error: "entity must be <= 120 chars and fact <= 500 chars" });
          return true;
        }
        const created = saveFact(botId, {
          category: body.category as MemoryFact["category"],
          entity,
          fact,
          confidence: typeof body.confidence === "number" ? Math.max(0, Math.min(1, body.confidence)) : 1.0,
        });
        json(res, 201, { ok: true, fact: created });
        return true;
      } catch {
        json(res, 400, { error: "invalid json body" });
        return true;
      }
    }
  }

  const factItemMatch = FACT_ITEM_PATH.exec(path);
  if (factItemMatch && method === "DELETE") {
    const botId = factItemMatch[1];
    const factId = factItemMatch[2];
    if (!store.bot(botId)) {
      json(res, 404, { error: "no such bot" });
      return true;
    }
    const deleted = deleteFact(botId, factId);
    if (!deleted) {
      json(res, 404, { error: "no such fact" });
      return true;
    }
    json(res, 200, { ok: true });
    return true;
  }

  if (method === "GET" && path === "/api/web-search") {
    const q = String(url.searchParams.get("q") ?? "").trim();
    if (!q) {
      json(res, 400, { error: "q parameter is required" });
      return true;
    }
    const limit = Number(url.searchParams.get("limit")) || 5;
    try {
      const results = await executeWebSearch(q, limit);
      json(res, 200, { ok: true, results });
      return true;
    } catch (error) {
      json(res, 500, { error: error instanceof Error ? error.message : String(error) });
      return true;
    }
  }

  return false;
}
