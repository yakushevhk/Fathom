/**
 * High-performance multiplexed WebSocket client for /api/v1/ws with
 * auto-reconnect, ping/pong heartbeats, and sub-16ms event dispatching.
 */

export interface WebSocketMessage {
  type: "pong" | "event";
  payload?: Record<string, unknown>;
}

export type WebSocketListener = (msg: WebSocketMessage) => void;

export class FathomWebSocketClient {
  private ws: WebSocket | null = null;
  private url: string;
  private listeners: Set<WebSocketListener> = new Set();
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private reconnectTimer: number | undefined = undefined;
  private heartbeatTimer: number | undefined = undefined;
  private sessionId: string | null = null;

  constructor(url?: string) {
    if (url) {
      this.url = url;
    } else {
      const rawBase = typeof window !== "undefined"
        ? localStorage.getItem("fathom_base_url") || "http://127.0.0.1:8080"
        : "http://127.0.0.1:8080";
      const apiKey = typeof window !== "undefined"
        ? localStorage.getItem("fathom_api_key") || ""
        : "";
      const wsBase = rawBase.replace(/^http:/, "ws:").replace(/^https:/, "wss:");
      const baseWithSlash = wsBase.endsWith("/") ? wsBase : `${wsBase}/`;
      const fullUrl = new URL("api/v1/ws", baseWithSlash);
      if (apiKey) {
        fullUrl.searchParams.set("api_key", apiKey);
      }
      this.url = fullUrl.toString();
    }
  }

  public connect(sessionId?: string) {
    this.sessionId = sessionId || null;
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.onerror = null;
      this.ws.close();
      this.ws = null;
    }

    try {
      this.ws = new WebSocket(this.url);

      this.ws.onopen = () => {
        this.reconnectAttempts = 0;
        this.startHeartbeat();
        if (this.sessionId) {
          this.subscribe(this.sessionId);
        }
      };

      this.ws.onmessage = (event: MessageEvent) => {
        try {
          const parsed: WebSocketMessage = JSON.parse(event.data);
          this.listeners.forEach((listener) => listener(parsed));
        } catch {
          // ignore non-json messages
        }
      };

      this.ws.onclose = () => {
        this.stopHeartbeat();
        this.scheduleReconnect();
      };

      this.ws.onerror = () => {
        if (this.ws) {
          this.ws.close();
        }
      };
    } catch {
      this.scheduleReconnect();
    }
  }

  public subscribe(sessionId: string) {
    this.sessionId = sessionId;
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: "subscribe", session_id: sessionId }));
    }
  }

  public send(data: Record<string, unknown> | string) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(typeof data === "string" ? data : JSON.stringify(data));
    }
  }

  public addListener(listener: WebSocketListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  public disconnect() {
    this.stopHeartbeat();
    clearTimeout(this.reconnectTimer);
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  private startHeartbeat() {
    this.stopHeartbeat();
    this.heartbeatTimer = window.setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: "ping" }));
      }
    }, 15000);
  }

  private stopHeartbeat() {
    clearInterval(this.heartbeatTimer);
    this.heartbeatTimer = undefined;
  }

  private scheduleReconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      return;
    }
    const backoff = Math.min(1000 * Math.pow(1.5, this.reconnectAttempts), 30000);
    this.reconnectAttempts++;
    this.reconnectTimer = window.setTimeout(() => {
      this.connect(this.sessionId || undefined);
    }, backoff);
  }
}
