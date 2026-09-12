// "You have not set this up" and "I could not read your key" produce the same
// empty screen today. They are opposite situations: the first is the truth,
// the second is ignorance the UI must be told about so it can keep showing
// what it already knew.
import { describe, expect, it } from "vitest";

import { connectorAvailability } from "./composio.ts";
import type { AppConfig } from "./config.ts";

const cfg = (over: Partial<AppConfig> = {}): AppConfig => ({ ...over }) as AppConfig;

describe("connectorAvailability", () => {
  it("is configured when a project key is present", () => {
    expect(connectorAvailability(cfg({ composio: { apiKey: "ak_live" } }), undefined)).toBe("configured");
  });

  it("is unconfigured when there is no key and the store read fine", () => {
    expect(connectorAvailability(cfg(), undefined)).toBe("unconfigured");
    expect(connectorAvailability(cfg({ composio: { apiKey: "" } }), "ok")).toBe("unconfigured");
  });

  it("is unreadable when the desktop shell could not open the credential store", () => {
    expect(connectorAvailability(cfg(), "unavailable")).toBe("unreadable");
  });

  it("prefers a working key over a store that failed earlier in the launch", () => {
    // the key arrived some other way (env, self-hosted config): what the user
    // can actually do matters more than how the shell felt about it
    expect(connectorAvailability(cfg({ composio: { apiKey: "ak_live" } }), "unavailable")).toBe("configured");
  });
});
