export type ControlOwner = "bot" | "human";

export class ControlError extends Error {
  readonly status = 409;
}

/** In-memory control lease. Bot actions are rejected while a human holds the lease. */
export type ControlListener = (status: { owner: ControlOwner; humanSince?: number }) => void;

export class ControlState {
  private owner: ControlOwner = "bot";
  private humanSince?: number;
  private readonly listeners = new Set<ControlListener>();

  subscribe(listener: ControlListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    const status = this.status();
    for (const listener of this.listeners) {
      try {
        listener(status);
      } catch {
        // A disconnected observer must not prevent the lease transition.
      }
    }
  }

  take(): { owner: ControlOwner; humanSince: number } {
    if (this.owner === "human") {
      return { owner: "human", humanSince: this.humanSince ?? Date.now() };
    }
    this.owner = "human";
    this.humanSince = Date.now();
    this.notify();
    return { owner: "human", humanSince: this.humanSince };
  }

  release(): { owner: ControlOwner } {
    this.owner = "bot";
    this.humanSince = undefined;
    this.notify();
    return { owner: "bot" };
  }

  assertHuman(): void {
    if (this.owner !== "human") throw new ControlError("Human control lease is required");
  }

  assertBot(): void {
    if (this.owner === "human") {
      throw new ControlError("Computer is currently controlled by a human");
    }
  }

  status(): { owner: ControlOwner; humanSince?: number } {
    return this.owner === "human" && this.humanSince !== undefined
      ? { owner: this.owner, humanSince: this.humanSince }
      : { owner: this.owner };
  }
}
