import { createHash } from "node:crypto";
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { Duplex } from "node:stream";
import { BrowserSession } from "./session.js";
import { ControlError } from "./control.js";
import { RefError } from "./snapshot.js";
import { InputDispatcher, MAX_INPUT_MESSAGE_BYTES } from "./input.js";
import { Workspace, WorkspaceError } from "./workspace.js";

const MAX_WORKSPACE_BODY_BYTES = 1_100_000;

export interface ServerOptions {
  host?: string;
  port?: number;
  token?: string;
  session?: BrowserSession;
  workspace?: Workspace;
}

export class ComputerServer {
  readonly session: BrowserSession;
  private readonly host: string;
  private readonly port: number;
  private readonly token?: string;
  private server?: Server;
  private readonly sockets = new Set<WsConnection>();
  private readonly workspace: Workspace;
  private readonly input: InputDispatcher;
  private unsubscribeControl?: () => void;

  constructor(options: ServerOptions = {}) {
    this.host = options.host || process.env.COMPUTER_HOST || "127.0.0.1";
    this.port = options.port ?? Number(process.env.COMPUTER_PORT || 8765);
    this.token = options.token ?? process.env.COMPUTER_TOKEN;
    this.session = options.session || new BrowserSession();
    this.workspace = options.workspace || new Workspace();
    this.input = new InputDispatcher(this.session.control, () => this.session.activePage);
  }

  async listen(): Promise<void> {
    await this.session.start();
    this.server = createServer((request, response) => {
      void this.handle(request, response);
    });
    this.server.on("upgrade", (request, socket, head) => {
      void this.handleUpgrade(request, socket, head);
    });
    this.unsubscribeControl = this.session.control.subscribe((control) => {
      for (const socket of this.sockets) socket.send({ type: "state", control });
    });
    await new Promise<void>((resolve, reject) => {
      this.server!.once("error", reject);
      this.server!.listen(this.port, this.host, () => resolve());
    });
  }

  async close(): Promise<void> {
    const server = this.server;
    this.server = undefined;
    this.unsubscribeControl?.();
    this.unsubscribeControl = undefined;
    for (const socket of this.sockets) socket.close();
    this.sockets.clear();
    if (server) await new Promise<void>((resolve) => server.close(() => resolve()));
    await this.session.close();
  }

  get address(): { host: string; port: number } {
    return { host: this.host, port: this.port };
  }

  private async handle(request: IncomingMessage, response: ServerResponse): Promise<void> {
    try {
      const method = request.method || "GET";
      const url = new URL(request.url || "/", `http://${this.host}`);
      if (url.pathname === "/health" && method === "GET") {
        sendJson(response, 200, { ok: true, service: "computer", control: this.session.control.status() });
        return;
      }
      if (!this.authorized(request)) {
        sendJson(response, 401, { error: "Unauthorized" });
        return;
      }
      if (url.pathname === "/files" && method === "GET") {
        sendJson(response, 200, await this.workspace.list());
        return;
      }
      if (url.pathname === "/files/read" && method === "GET") {
        sendJson(response, 200, await this.workspace.read(asRequiredString(url.searchParams.get("path"), "path")));
        return;
      }
      if (url.pathname === "/files" && method === "DELETE") {
        this.session.control.assertBot();
        sendJson(response, 200, await this.workspace.delete(asRequiredString(url.searchParams.get("path"), "path")));
        return;
      }
      const body = await readJson(request, url.pathname === "/files/write" ? MAX_WORKSPACE_BODY_BYTES : MAX_INPUT_MESSAGE_BYTES);
      if (url.pathname === "/files/write" && method === "PUT") {
        this.session.control.assertBot();
        sendJson(response, 200, await this.workspace.write(asRequiredString(body.path, "path"), asContentString(body.content, "content")));
        return;
      }
      if (url.pathname === "/session" && method === "POST") {
        this.session.control.assertBot();
        await this.session.start(asOptionalString(body.url));
        sendJson(response, 200, { ok: true, snapshot: await this.session.snapshot(), control: this.session.control.status() });
        return;
      }
      if (url.pathname === "/tabs" && method === "GET") {
        sendJson(response, 200, { tabs: await this.session.tabs() });
        return;
      }
      if (url.pathname === "/tabs/open" && method === "POST") {
        this.session.control.assertBot();
        sendJson(response, 200, await this.session.openTab(asRequiredString(body.url, "url")));
        return;
      }
      const tabAction = url.pathname.match(/^\/tabs\/([^/]+)\/(activate|close)$/);
      if (tabAction && method === "POST") {
        this.session.control.assertBot();
        const tabId = decodeURIComponent(tabAction[1]);
        sendJson(response, 200, tabAction[2] === "activate"
          ? await this.session.activateTab(tabId)
          : { tabs: await this.session.closeTab(tabId) });
        return;
      }
      if (url.pathname === "/snapshot" && method === "GET") {
        sendJson(response, 200, await this.session.snapshot(url.searchParams.get("tab_id") || undefined));
        return;
      }
      if (url.pathname === "/navigate" && method === "POST") {
        this.session.control.assertBot();
        sendJson(response, 200, await this.session.navigate(asRequiredString(body.url, "url")));
        return;
      }
      if (url.pathname === "/click" && method === "POST") {
        this.session.control.assertBot();
        sendJson(response, 200, await this.session.click(body.ref));
        return;
      }
      if (url.pathname === "/type" && method === "POST") {
        this.session.control.assertBot();
        sendJson(response, 200, await this.session.type(body.ref, body.text, body.submit === true));
        return;
      }
      if (url.pathname === "/operator/secret" && method === "POST") {
        if (!this.token || request.headers["x-fathom-operator"] !== "true") {
          sendJson(response, 403, { error: "Operator access required" });
          return;
        }
        this.session.control.assertBot();
        const snapshot = await this.session.enterSecret(body.ref, body.secret);
        sendJson(response, 200, { ok: true, url: snapshot.url, title: snapshot.title, elements: snapshot.elements });
        return;
      }
      if (url.pathname === "/key" && method === "POST") {
        this.session.control.assertBot();
        sendJson(response, 200, await this.session.key(body.key, body.ref));
        return;
      }
      if (url.pathname === "/control/take" && method === "POST") {
        sendJson(response, 200, this.session.control.take());
        return;
      }
      if (url.pathname === "/control/release" && method === "POST") {
        sendJson(response, 200, this.session.control.release());
        return;
      }
      if (url.pathname === "/screenshot" && method === "GET") {
        const data = (await this.session.screenshot()).toString("base64");
        sendJson(response, 200, { mimeType: "image/png", data });
        return;
      }
      sendJson(response, 404, { error: "Not found" });
    } catch (error) {
      const status = error instanceof ControlError || error instanceof RefError || error instanceof WorkspaceError ? error.status : 400;
      const message = error instanceof WorkspaceError || error instanceof ControlError || error instanceof RefError
        ? error.message
        : "Request failed";
      sendJson(response, status, { error: message });
    }
  }

  private authorized(request: IncomingMessage): boolean {
    if (!this.token) return true;
    const auth = request.headers.authorization;
    const provided = auth?.startsWith("Bearer ") ? auth.slice(7) : request.headers["x-computer-token"];
    return typeof provided === "string" && provided === this.token;
  }

  private async handleUpgrade(request: IncomingMessage, socket: Duplex, head: Buffer): Promise<void> {
    const url = new URL(request.url || "/", `http://${this.host}`);
    if (url.pathname !== "/control/ws" || request.headers.upgrade?.toLowerCase() !== "websocket" || !this.authorizedWebSocket(request, url)) {
      socket.write("HTTP/1.1 401 Unauthorized\\r\\nConnection: close\\r\\n\\r\\n");
      socket.destroy();
      return;
    }
    const key = request.headers["sec-websocket-key"];
    if (typeof key !== "string") {
      socket.write("HTTP/1.1 400 Bad Request\\r\\nConnection: close\\r\\n\\r\\n");
      socket.destroy();
      return;
    }
    const accept = createHash("sha1").update(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest("base64");
    socket.write(`HTTP/1.1 101 Switching Protocols\\r\\nUpgrade: websocket\\r\\nConnection: Upgrade\\r\\nSec-WebSocket-Accept: ${accept}\\r\\n\\r\\n`);
    let connection!: WsConnection;
    connection = new WsConnection(socket, (value) => this.handleWsMessage(connection, value));
    this.sockets.add(connection);
    connection.onClose = () => this.sockets.delete(connection);
    connection.send({ type: "ready", control: this.session.control.status() });
    if (head.length > 0) connection.push(head);
  }

  private authorizedWebSocket(request: IncomingMessage, _url: URL): boolean {
    if (!this.token) return false;
    return this.authorized(request);
  }

  private async handleWsMessage(connection: WsConnection, value: unknown): Promise<void> {
    try {
      const result = await this.input.dispatch(value);
      if (result === "pong") connection.send({ type: "pong" });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Input failed";
      connection.send({ type: "error", message });
    }
  }
}

class WsConnection {
  private buffer = Buffer.alloc(0);
  private closed = false;
  private pending: Promise<void> = Promise.resolve();
  onClose = (): void => {};

  constructor(private readonly socket: Duplex, private readonly onMessage: (value: unknown) => Promise<void>) {
    socket.on("data", (chunk: Buffer | string) => this.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)));
    socket.on("close", () => this.finish());
    socket.on("error", () => this.finish());
  }

  push(chunk: Buffer): void {
    if (this.closed) return;
    this.buffer = Buffer.concat([this.buffer, chunk]);
    if (this.buffer.length > MAX_INPUT_MESSAGE_BYTES + 16 * 1024) {
      this.close(1009);
      return;
    }
    try {
      while (this.readFrame()) { /* drain complete frames */ }
    } catch {
      this.close(1002);
    }
  }

  send(value: unknown): void {
    if (this.closed) return;
    const payload = Buffer.from(JSON.stringify(value), "utf8");
    if (payload.length > MAX_INPUT_MESSAGE_BYTES) {
      this.close(1009);
      return;
    }
    try {
      this.socket.write(frame(0x1, payload));
    } catch {
      this.finish();
    }
  }

  close(code = 1000): void {
    if (this.closed) return;
    this.closed = true;
    try {
      this.socket.write(frame(0x8, Buffer.from([code >> 8, code & 0xff])));
      this.socket.end();
    } catch {
      this.socket.destroy();
    }
    this.onClose();
  }

  private readFrame(): boolean {
    if (this.buffer.length < 2) return false;
    const first = this.buffer[0];
    const second = this.buffer[1];
    const opcode = first & 0x0f;
    const masked = (second & 0x80) !== 0;
    let length = second & 0x7f;
    let offset = 2;
    if (length === 126) {
      if (this.buffer.length < 4) return false;
      length = this.buffer.readUInt16BE(2);
      offset = 4;
    } else if (length === 127) {
      if (this.buffer.length < 10) return false;
      const bigLength = this.buffer.readBigUInt64BE(2);
      if (bigLength > BigInt(MAX_INPUT_MESSAGE_BYTES)) throw new Error("frame too large");
      length = Number(bigLength);
      offset = 10;
    }
    if (length > MAX_INPUT_MESSAGE_BYTES) throw new Error("frame too large");
    const maskOffset = masked ? 4 : 0;
    const total = offset + maskOffset + length;
    if (this.buffer.length < total) return false;
    if (!masked) throw new Error("client frames must be masked");
    const mask = this.buffer.subarray(offset, offset + 4);
    const payload = Buffer.alloc(length);
    const start = offset + 4;
    for (let index = 0; index < length; index += 1) payload[index] = this.buffer[start + index] ^ mask[index % 4];
    this.buffer = this.buffer.subarray(total);
    if (opcode === 0x8) { this.close(); return true; }
    if (opcode === 0x9) { this.socket.write(frame(0xA, payload)); return true; }
    if (opcode !== 0x1 || (first & 0x40) !== 0) throw new Error("unsupported websocket frame");
    let value: unknown;
    try { value = JSON.parse(payload.toString("utf8")); } catch { this.send({ type: "error", message: "Invalid JSON message" }); return true; }
    this.pending = this.pending.then(() => this.onMessage(value)).catch(() => undefined);
    return true;
  }

  private finish(): void {
    if (this.closed) return;
    this.closed = true;
    this.onClose();
  }
}

function frame(opcode: number, payload: Buffer): Buffer {
  const length = payload.length;
  if (length < 126) return Buffer.concat([Buffer.from([0x80 | opcode, length]), payload]);
  if (length <= 0xffff) { const header = Buffer.alloc(4); header[0] = 0x80 | opcode; header[1] = 126; header.writeUInt16BE(length, 2); return Buffer.concat([header, payload]); }
  const header = Buffer.alloc(10); header[0] = 0x80 | opcode; header[1] = 127; header.writeBigUInt64BE(BigInt(length), 2); return Buffer.concat([header, payload]);
}


async function readJson(request: IncomingMessage, limit = MAX_INPUT_MESSAGE_BYTES): Promise<Record<string, unknown>> {
  const chunks: Buffer[] = [];
  let total = 0;
  for await (const chunk of request) {
    const bytes = Buffer.from(chunk);
    total += bytes.length;
    if (total > limit) throw new WorkspaceError(413);
    chunks.push(bytes);
  }
  if (chunks.length === 0) return {};
  const parsed: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("JSON body must be an object");
  return parsed as Record<string, unknown>;
}

function asRequiredString(value: unknown, field: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`${field} must be a non-empty string`);
  return value;
}

function asOptionalString(value: unknown): string | undefined {
  return value === undefined ? undefined : asRequiredString(value, "url");
}

function asContentString(value: unknown, field: string): string {
  if (typeof value !== "string") throw new WorkspaceError();
  return value;
}

function sendJson(response: ServerResponse, status: number, value: unknown): void {
  const body = JSON.stringify(value);
  response.writeHead(status, { "content-type": "application/json; charset=utf-8", "content-length": Buffer.byteLength(body), "cache-control": "no-store" });
  response.end(body);
}
