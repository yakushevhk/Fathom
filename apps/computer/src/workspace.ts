import { createHash } from "node:crypto";
import { lstat, mkdir, readdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";

export const WORKSPACE_MAX_FILE_BYTES = 1024 * 1024;
export const WORKSPACE_MAX_ENTRIES = 1000;
export const WORKSPACE_MAX_DEPTH = 8;

/** A deliberately generic error for workspace requests. */
export class WorkspaceError extends Error {
  constructor(readonly status = 400) {
    super("Workspace request failed");
    this.name = "WorkspaceError";
  }
}

export interface WorkspaceEntry {
  path: string;
  type: "file" | "directory";
  size?: number;
}

/** Filesystem access confined to COMPUTER_WORKSPACE. */
export class Workspace {
  readonly root: string;

  constructor(root = process.env.COMPUTER_WORKSPACE || ".computer-workspace") {
    this.root = resolve(root);
  }

  async list(): Promise<{ entries: WorkspaceEntry[]; truncated: boolean }> {
    await this.ensureRoot();
    const entries: WorkspaceEntry[] = [];
    let truncated = false;
    const visit = async (directory: string, depth: number): Promise<void> => {
      if (depth > WORKSPACE_MAX_DEPTH || entries.length >= WORKSPACE_MAX_ENTRIES) {
        truncated = true;
        return;
      }
      let children;
      try {
        children = await readdir(directory, { withFileTypes: true });
      } catch {
        throw new WorkspaceError();
      }
      children.sort((a, b) => a.name.localeCompare(b.name));
      for (const child of children) {
        if (entries.length >= WORKSPACE_MAX_ENTRIES) {
          truncated = true;
          return;
        }
        const childPath = resolve(directory, child.name);
        const relativePath = this.confinedRelative(childPath);
        let info;
        try {
          info = await lstat(childPath);
        } catch {
          throw new WorkspaceError();
        }
        if (info.isSymbolicLink()) throw new WorkspaceError();
        if (info.isDirectory()) {
          entries.push({ path: relativePath, type: "directory" });
          await visit(childPath, depth + 1);
        } else if (info.isFile()) {
          entries.push({ path: relativePath, type: "file", size: info.size });
        }
      }
    };
    await visit(this.root, 0);
    return { entries, truncated };
  }

  async read(path: string): Promise<{ path: string; content: string; size: number; truncated: boolean }> {
    const target = await this.safePath(path, false);
    let info;
    try {
      info = await lstat(target);
      if (!info.isFile()) throw new Error();
    } catch {
      throw new WorkspaceError(404);
    }
    if (info.size > WORKSPACE_MAX_FILE_BYTES) throw new WorkspaceError(413);
    try {
      const bytes = await readFile(target);
      if (bytes.length > WORKSPACE_MAX_FILE_BYTES) throw new Error();
      return { path: this.confinedRelative(target), content: bytes.toString("utf8"), size: bytes.length, truncated: false };
    } catch {
      throw new WorkspaceError();
    }
  }

  async write(path: string, content: string): Promise<{ path: string; size: number }> {
    if (typeof content !== "string") throw new WorkspaceError();
    const bytes = Buffer.from(content, "utf8");
    if (bytes.length > WORKSPACE_MAX_FILE_BYTES) throw new WorkspaceError(413);
    const target = await this.safePath(path, true);
    const parent = dirname(target);
    try {
      let current = this.root;
      const parts = path.split(/[\\/]+/);
      for (const part of parts.slice(0, -1)) {
        current = resolve(current, part);
        try {
          const info = await lstat(current);
          if (!info.isDirectory() || info.isSymbolicLink()) throw new Error();
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
          await mkdir(current);
          const info = await lstat(current);
          if (!info.isDirectory() || info.isSymbolicLink()) throw new Error();
        }
      }
      const parentInfo = await lstat(parent);
      if (!parentInfo.isDirectory() || parentInfo.isSymbolicLink()) throw new Error();
      const temp = resolve(parent, `.fathom-${process.pid}-${Date.now()}-${createHash("sha256").update(target).digest("hex").slice(0, 12)}.tmp`);
      try {
        await writeFile(temp, bytes, { flag: "wx", mode: 0o600 });
        await rename(temp, target);
      } finally {
        await rm(temp, { force: true }).catch(() => undefined);
      }
    } catch {
      throw new WorkspaceError();
    }
    return { path: this.confinedRelative(target), size: bytes.length };
  }

  async delete(path: string): Promise<{ path: string }> {
    const target = await this.safePath(path, false);
    try {
      const info = await lstat(target);
      if (!info.isFile() && !info.isDirectory()) throw new Error();
      if (info.isSymbolicLink()) throw new Error();
      await rm(target, { recursive: info.isDirectory(), force: false });
    } catch {
      throw new WorkspaceError(404);
    }
    return { path: this.confinedRelative(target) };
  }

  private async safePath(input: string, allowMissing: boolean): Promise<string> {
    if (typeof input !== "string" || input.length === 0 || input.includes("\0") || isAbsolute(input)) throw new WorkspaceError();
    const parts = input.split(/[\\/]+/);
    if (parts.some((part) => part.length === 0 || part === "." || part === "..")) throw new WorkspaceError();
    const target = resolve(this.root, ...parts);
    this.confinedRelative(target);
    await this.ensureRoot();
    let current = this.root;
    for (const part of parts) {
      current = resolve(current, part);
      try {
        const info = await lstat(current);
        if (info.isSymbolicLink()) throw new WorkspaceError();
        if (!info.isDirectory() && current !== target) throw new WorkspaceError();
      } catch (error) {
        if (error instanceof WorkspaceError) throw error;
        if (!allowMissing) throw new WorkspaceError(404);
        // Missing components are created by the atomic write path below.
        break;
      }
    }
    return target;
  }

  private confinedRelative(target: string): string {
    const value = relative(this.root, target);
    if (!value || value === ".." || value.startsWith(`..${sep}`) || isAbsolute(value)) throw new WorkspaceError();
    return value.split(sep).join("/");
  }

  private async ensureRoot(): Promise<void> {
    try {
      await mkdir(this.root, { recursive: true });
      const info = await lstat(this.root);
      if (!info.isDirectory() || info.isSymbolicLink()) throw new Error();
    } catch {
      throw new WorkspaceError();
    }
  }
}
