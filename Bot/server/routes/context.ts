// Route handler context interface passed to modular route handlers.
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Store } from "../store.ts";

export interface RouteContext {
  req: IncomingMessage;
  res: ServerResponse;
  url: URL;
  method: string;
  path: string;
  store: Store;
  readBody: () => Promise<string>;
  json: (res: ServerResponse, status: number, payload: unknown) => void;
}

export type RouteHandler = (ctx: RouteContext) => Promise<boolean> | boolean;
