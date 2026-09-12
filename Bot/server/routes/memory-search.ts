// Modular router for memory facts and search API endpoints.
import type { RouteContext } from "./context.ts";
import { saveFact, listFacts, deleteFact, type MemoryFact } from "../structured-memory.ts";
import { executeWebSearch } from "../web-search.ts";

export async function handleMemoryAndSearchRoutes(ctx: RouteContext): Promise<boolean> {
  const { path, method, url, res, json, readBody } = ctx;

  if (method === "GET" && path.startsWith("/api/bots/") && path.endsWith("/facts")) {
    const parts = path.split("/");
    const botId = parts[3];
    if (!botId) return false;
    const category = url.searchParams.get("category") as MemoryFact["category"] | null;
    const facts = listFacts(botId, category || undefined);
    json(res, 200, { ok: true, facts });
    return true;
  }

  if (method === "POST" && path.startsWith("/api/bots/") && path.endsWith("/facts")) {
    const parts = path.split("/");
    const botId = parts[3];
    if (!botId) return false;
    try {
      const body = JSON.parse(await readBody()) as {
        category?: MemoryFact["category"];
        entity?: string;
        fact?: string;
        confidence?: number;
      };
      if (!body.category || !body.entity || !body.fact) {
        json(res, 400, { error: "category, entity, and fact are required" });
        return true;
      }
      const created = saveFact(botId, {
        category: body.category,
        entity: String(body.entity).trim(),
        fact: String(body.fact).trim(),
        confidence: body.confidence,
      });
      json(res, 201, { ok: true, fact: created });
      return true;
    } catch {
      json(res, 400, { error: "invalid json body" });
      return true;
    }
  }

  if (method === "DELETE" && path.startsWith("/api/bots/") && path.includes("/facts/")) {
    const parts = path.split("/");
    const botId = parts[3];
    const factId = parts[5];
    if (!botId || !factId) return false;
    const deleted = deleteFact(botId, factId);
    json(res, deleted ? 200 : 404, { ok: deleted });
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
