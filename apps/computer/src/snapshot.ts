import type { Page } from "playwright";

export interface SnapshotElement {
  ref: string;
  role: string | null;
  name: string;
  tag: string;
  value?: string;
}

export interface PageSnapshot {
  tab_id: string;
  url: string;
  title: string;
  aria: string;
  elements: SnapshotElement[];
}

interface RefEntry extends SnapshotElement {
  selector: string;
  fingerprint: string;
  tab_id: string;
}

interface TabSnapshotState {
  refs: Map<string, RefEntry>;
  paths: Map<string, string>;
  current?: PageSnapshot;
}

/** Maintains opaque refs for each tab, invalidating refs across tabs and stale snapshots. */
export class SnapshotManager {
  private readonly tabs = new Map<string, TabSnapshotState>();
  private sequence = 0;
  private current?: PageSnapshot;

  private state(tabId: string): TabSnapshotState {
    let state = this.tabs.get(tabId);
    if (!state) {
      state = { refs: new Map(), paths: new Map() };
      this.tabs.set(tabId, state);
    }
    return state;
  }

  async capture(page: Page, tabId: string): Promise<PageSnapshot> {
    if (!tabId) throw new Error("tab id is required");
    const state = this.state(tabId);
    const rawAria = await page.locator("body").ariaSnapshot({ mode: "default" });
    const aria = rawAria.slice(0, 100_000);
    const elements = await page.locator("body *").evaluateAll((nodes) => nodes.map((node) => {
      const element = node as HTMLElement;
      const tag = element.tagName.toLowerCase();
      let role = element.getAttribute("role");
      if (!role) {
        if (tag === "button") role = "button";
        else if (tag === "a" && element.hasAttribute("href")) role = "link";
        else if (tag === "textarea") role = "textbox";
        else if (tag === "select") role = "combobox";
        else if (tag === "input") {
          const type = (element.getAttribute("type") || "text").toLowerCase();
          role = ["button", "submit", "reset"].includes(type) ? "button" : type === "checkbox" ? "checkbox" : type === "radio" ? "radio" : "textbox";
        }
      }
      const labelledBy = element.getAttribute("aria-labelledby");
      const name = element.getAttribute("aria-label")?.trim()
        || (labelledBy ? labelledBy.split(/\\s+/).map((id) => document.getElementById(id)?.textContent || "").join(" ").trim() : "")
        || ((element as HTMLInputElement).placeholder || element.innerText || element.textContent || "").replace(/\\s+/g, " ").trim().slice(0, 200);
      // Never export live form values: snapshots are returned to agents and
      // may be persisted in transcripts/audit. Ref-based actions still work
      // without exposing the current value.
      let selector = element.id ? `#${CSS.escape(element.id)}` : "";
      if (!selector) {
        const parts: string[] = [];
        let current: HTMLElement | null = element;
        while (current && current !== document.body) {
          let part = current.tagName.toLowerCase();
          const parent: HTMLElement | null = current.parentElement;
          if (parent) {
            const children = Array.from(parent.children) as HTMLElement[];
            const siblings = children.filter((child: HTMLElement) => child.tagName === current!.tagName);
            if (siblings.length > 1) part += `:nth-of-type(${siblings.indexOf(current) + 1})`;
          }
          parts.unshift(part);
          current = parent;
        }
        selector = `body > ${parts.join(" > ")}`;
      }
      return { role, name, tag, selector };
    }));

    const nextRefs = new Map<string, RefEntry>();
    const output: SnapshotElement[] = [];
    for (const item of elements.slice(0, 2_000)) {
      // Keep named semantic elements and explicit interactive controls; omit generic containers.
      const actionable = item.role !== null || ["button", "a", "input", "textarea", "select", "option"].includes(item.tag);
      if ((!item.role && !item.name) || !actionable) continue;
      const fingerprint = `${item.selector}|${item.role ?? ""}|${item.name}`;
      let ref = state.paths.get(fingerprint);
      if (!ref || nextRefs.has(ref)) {
        ref = `t_${tabId}_e_${(++this.sequence).toString(36)}_${randomToken()}`;
      }
      const entry: RefEntry = { ...item, ref, fingerprint, tab_id: tabId };
      nextRefs.set(ref, entry);
      state.paths.set(fingerprint, ref);
      output.push({ ref, role: item.role, name: item.name, tag: item.tag });
    }
    state.refs.clear();
    for (const [ref, entry] of nextRefs) state.refs.set(ref, entry);
    const snapshot: PageSnapshot = { tab_id: tabId, url: page.url(), title: await page.title().catch(() => ""), aria, elements: output };
    state.current = snapshot;
    this.current = snapshot;
    return snapshot;
  }

  async get(page: Page, tabId: string): Promise<PageSnapshot> {
    return this.capture(page, tabId);
  }

  async resolve(page: Page, tabId: string, ref: unknown): Promise<RefEntry> {
    if (typeof ref !== "string" || ref.length === 0) throw new RefError("A valid element ref is required");
    const state = this.tabs.get(tabId);
    const previous = state?.refs.get(ref);
    if (!previous || previous.tab_id !== tabId || !ref.startsWith(`t_${tabId}_`)) throw new RefError(`Unknown or stale element ref: ${ref}`);
    await this.capture(page, tabId);
    const entry = this.tabs.get(tabId)?.refs.get(ref);
    // A ref must survive a fresh accessibility snapshot and retain its original fingerprint.
    if (!entry || entry.fingerprint !== previous.fingerprint) throw new RefError(`Unknown or stale element ref: ${ref}`);
    const locator = page.locator(entry.selector);
    if (await locator.count() !== 1) throw new RefError(`Element ref is no longer unique: ${ref}`);
    return entry;
  }

  locator(page: Page, entry: RefEntry) {
    return page.locator(entry.selector);
  }

  get cached(): PageSnapshot | undefined { return this.current; }
}

export class RefError extends Error {
  readonly status = 400;
}

function randomToken(): string {
  return Math.random().toString(36).slice(2, 8);
}

