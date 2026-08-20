import { chromium, type BrowserContext, type Page } from "playwright";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { ControlState } from "./control.js";
import { SnapshotManager, type PageSnapshot } from "./snapshot.js";
import { currentEgressOptions, validateEgressUrl } from "./egress.js";

export interface SessionOptions {
  workspace?: string;
  headless?: boolean;
  viewport?: { width: number; height: number };
}

export interface TabInfo {
  tab_id: string;
  url: string;
  title: string;
  active: boolean;
}

export class BrowserSession {
  private context?: BrowserContext;
  private page?: Page;
  private readonly pages = new Map<string, Page>();
  private readonly pageIds = new WeakMap<Page, string>();
  private nextTabId = 1;
  private activeTabId?: string;
  readonly snapshots = new SnapshotManager();
  readonly control = new ControlState();
  private readonly options: Required<SessionOptions>;

  constructor(options: SessionOptions = {}) {
    this.options = {
      workspace: options.workspace || process.env.COMPUTER_WORKSPACE || ".computer-workspace",
      headless: options.headless ?? parseBoolean(process.env.COMPUTER_HEADLESS, true),
      viewport: options.viewport || {
        width: Number(process.env.COMPUTER_VIEWPORT_WIDTH || 1280),
        height: Number(process.env.COMPUTER_VIEWPORT_HEIGHT || 800),
      },
    };
  }

  async start(initialUrl?: string): Promise<void> {
    if (this.context && this.page && !this.page.isClosed()) {
      if (initialUrl) await this.navigate(initialUrl);
      return;
    }
    const workspace = resolve(this.options.workspace);
    await mkdir(workspace, { recursive: true });
    this.context = await chromium.launchPersistentContext(workspace, {
      headless: this.options.headless,
      viewport: this.options.viewport,
      args: ["--disable-dev-shm-usage"],
    });
    this.context.on("page", (page) => {
      const tabId = this.registerPage(page);
      this.activeTabId = tabId;
      this.page = page;
    });
    const existing = this.context.pages();
    this.page = existing[0] || await this.context.newPage();
    await this.context.route("**/*", async route => {
      if (!route.request().isNavigationRequest()) return route.continue();
      try {
        validateEgressUrl(route.request().url(), currentEgressOptions());
        return route.continue();
      } catch {
        return route.abort("blockedbyclient");
      }
    });
    for (const page of this.context.pages()) this.registerPage(page);
    if (initialUrl) await this.navigate(initialUrl);
  }

  private registerPage(page: Page): string {
    const existing = this.pageIds.get(page);
    if (existing) return existing;
    const tabId = `tab_${(this.nextTabId++).toString(36)}`;
    this.pageIds.set(page, tabId);
    this.pages.set(tabId, page);
    if (!this.activeTabId) {
      this.activeTabId = tabId;
      this.page = page;
    }
    page.on("crash", () => this.dropPage(tabId, page));
    page.on("close", () => this.dropPage(tabId, page));
    return tabId;
  }

  private dropPage(tabId: string, page: Page): void {
    this.pages.delete(tabId);
    if (this.activeTabId === tabId) {
      const next = this.pages.entries().next().value as [string, Page] | undefined;
      this.activeTabId = next?.[0];
      this.page = next?.[1];
    }
    if (this.page === page && !this.activeTabId) this.page = undefined;
  }

  async close(): Promise<void> {
    const context = this.context;
    this.context = undefined;
    this.page = undefined;
    this.activeTabId = undefined;
    this.pages.clear();
    if (context) await context.close();
  }

  get activePage(): Page {
    if (!this.page || this.page.isClosed()) throw new Error("Computer browser session is not running");
    return this.page;
  }

  get activeTabIdValue(): string {
    if (!this.activeTabId) throw new Error("Computer browser session is not running");
    return this.activeTabId;
  }

  private pageFor(tabId?: string): { id: string; page: Page } {
    const id = tabId || this.activeTabIdValue;
    const page = this.pages.get(id);
    if (!page || page.isClosed()) throw new Error(`Unknown tab: ${id}`);
    return { id, page };
  }

  async tabs(): Promise<TabInfo[]> {
    return Promise.all([...this.pages.entries()].map(async ([tab_id, page]) => ({
      tab_id,
      url: page.url(),
      title: await page.title().catch(() => ""),
      active: tab_id === this.activeTabId,
    })));
  }

  async openTab(url: string): Promise<TabInfo & { snapshot: PageSnapshot }> {
    validateEgressUrl(url, currentEgressOptions());
    if (!this.context) throw new Error("Computer browser session is not running");
    const page = await this.context.newPage();
    const tabId = this.registerPage(page);
    try {
      await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30_000 });
      validateEgressUrl(page.url(), currentEgressOptions());
    } catch {
      await page.close().catch(() => undefined);
      throw new Error("Navigation failed");
    }
    await this.activateTab(tabId);
    return { ...(await this.tabs()).find((tab) => tab.tab_id === tabId)!, snapshot: await this.snapshot(tabId) };
  }

  async activateTab(tabId: string): Promise<TabInfo> {
    const { page } = this.pageFor(tabId);
    this.activeTabId = tabId;
    this.page = page;
    await page.bringToFront();
    return (await this.tabs()).find((tab) => tab.tab_id === tabId)!;
  }

  async closeTab(tabId: string): Promise<TabInfo[]> {
    const { page } = this.pageFor(tabId);
    if (this.pages.size <= 1) throw new Error("Cannot close the last tab");
    await page.close();
    return this.tabs();
  }

  async snapshot(tabId?: string): Promise<PageSnapshot> {
    const { id, page } = this.pageFor(tabId);
    return this.snapshots.capture(page, id);
  }

  async navigate(url: string) {
    validateEgressUrl(url, currentEgressOptions());
    const page = this.activePage;
    try {
      await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30_000 });
      validateEgressUrl(page.url(), currentEgressOptions());
    } catch {
      throw new Error("Navigation failed");
    }
    return this.snapshot();
  }

  async click(ref: unknown) {
    const { id, page } = this.pageFor();
    const entry = await this.snapshots.resolve(page, id, ref);
    await this.snapshots.locator(page, entry).click({ timeout: 15_000 });
    return this.snapshot(id);
  }

  async type(ref: unknown, text: unknown, submit = false) {
    if (typeof text !== "string") throw new Error("text must be a string");
    const { id, page } = this.pageFor();
    const entry = await this.snapshots.resolve(page, id, ref);
    const locator = this.snapshots.locator(page, entry);
    await locator.fill(text, { timeout: 15_000 });
    if (submit) await locator.press("Enter");
    return this.snapshot(id);
  }

  async key(key: unknown, ref?: unknown) {
    if (typeof key !== "string" || key.length === 0) throw new Error("key must be a non-empty string");
    const { id, page } = this.pageFor();
    if (ref !== undefined) {
      const entry = await this.snapshots.resolve(page, id, ref);
      await this.snapshots.locator(page, entry).press(key, { timeout: 15_000 });
    } else {
      await page.keyboard.press(key);
    }
    return this.snapshot(id);
  }

  /** Type a secret directly into a current snapshot element. The value is
   * never returned, logged, or included in a snapshot response. */
  async enterSecret(ref: unknown, secret: unknown): Promise<PageSnapshot> {
    if (typeof secret !== "string" || secret.length === 0) throw new Error("secret must be a non-empty string");
    const { id, page } = this.pageFor();
    const entry = await this.snapshots.resolve(page, id, ref);
    await this.snapshots.locator(page, entry).fill(secret, { timeout: 15_000 });
    return this.snapshot(id);
  }

  async screenshot(): Promise<Buffer> {
    return this.activePage.screenshot({ type: "png" });
  }
}

function parseBoolean(value: string | undefined, fallback: boolean): boolean {
  if (value === undefined) return fallback;
  return value === "1" || value.toLowerCase() === "true";
}

function isHttpUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}
