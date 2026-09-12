import { describe, expect, it } from "vitest";
import type { IncomingMessage, ServerResponse } from "node:http";
import { handleMemoryAndSearchRoutes } from "./memory-search.ts";
import { Store } from "../store.ts";

describe("memory-search routes", () => {
  const mockContext = (method: string, path: string, bodyObj: unknown = {}) => {
    let statusCode = 0;
    let payload: unknown = null;

    const res = {
      setHeader: () => {},
      writeHead: (code: number) => {
        statusCode = code;
      },
      end: (data: string) => {
        try {
          payload = JSON.parse(data);
        } catch {
          payload = data;
        }
      },
    } as unknown as ServerResponse;

    const req = {} as unknown as IncomingMessage;
    const url = new URL(`http://localhost${path}`);
    const store = new Store(() => ({ instanceId: "claude", model: "claude-sonnet-5" }));

    return {
      ctx: {
        req,
        res,
        url,
        method,
        path: url.pathname,
        store,
        readBody: async () => JSON.stringify(bodyObj),
        json: (_res: ServerResponse, status: number, data: unknown) => {
          statusCode = status;
          payload = data;
        },
      },
      getResponse: () => ({ status: statusCode, payload }),
    };
  };

  it("handles GET /api/bots/:id/facts", async () => {
    const store = new Store(() => ({ instanceId: "claude", model: "claude-sonnet-5" }));
    const bot = store.createBot({ name: "Test Bot" });
    const { ctx, getResponse } = mockContext("GET", `/api/bots/${bot.id}/facts`);
    ctx.store = store;
    const handled = await handleMemoryAndSearchRoutes(ctx);
    expect(handled).toBe(true);
    expect(getResponse().status).toBe(200);
    expect(getResponse().payload).toMatchObject({ ok: true, facts: expect.any(Array) });
  });

  it("returns 404 for non-existent bot on GET /api/bots/:id/facts", async () => {
    const { ctx, getResponse } = mockContext("GET", "/api/bots/missing-bot/facts");
    const handled = await handleMemoryAndSearchRoutes(ctx);
    expect(handled).toBe(true);
    expect(getResponse().status).toBe(404);
    expect(getResponse().payload).toEqual({ error: "no such bot" });
  });

  it("handles POST /api/bots/:id/facts validation", async () => {
    const botStore = new Store(() => ({ instanceId: "claude", model: "claude-sonnet-5" }));
    const bot = botStore.createBot({ name: "Test Bot" });
    const { ctx, getResponse } = mockContext("POST", `/api/bots/${bot.id}/facts`, {
      category: "preference",
      entity: "user",
      fact: "Test preference",
    });
    ctx.store = botStore;
    const handled = await handleMemoryAndSearchRoutes(ctx);
    expect(handled).toBe(true);
    expect(getResponse().status).toBe(201);
  });

  it("returns 400 for invalid category on POST", async () => {
    const botStore = new Store(() => ({ instanceId: "claude", model: "claude-sonnet-5" }));
    const bot = botStore.createBot({ name: "Test Bot" });
    const { ctx, getResponse } = mockContext("POST", `/api/bots/${bot.id}/facts`, {
      category: "invalid-category",
      entity: "user",
      fact: "Test preference",
    });
    ctx.store = botStore;
    const handled = await handleMemoryAndSearchRoutes(ctx);
    expect(handled).toBe(true);
    expect(getResponse().status).toBe(400);
    expect(getResponse().payload).toMatchObject({ error: expect.stringContaining("category must be one of") });
  });

  it("returns false for unhandled paths", async () => {
    const { ctx } = mockContext("GET", "/api/unrelated");
    const handled = await handleMemoryAndSearchRoutes(ctx);
    expect(handled).toBe(false);
  });
});
