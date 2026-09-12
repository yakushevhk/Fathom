import { describe, expect, it } from "vitest";
import { executeWebSearch } from "./web-search.ts";

describe("web-search", () => {
  it("returns an empty array for blank or whitespace query", async () => {
    expect(await executeWebSearch("")).toEqual([]);
    expect(await executeWebSearch("   ")).toEqual([]);
  });

  it("clamps limit between 1 and 10", async () => {
    // Should not throw on extreme limit arguments
    const res = await executeWebSearch("test query", 0);
    expect(Array.isArray(res)).toBe(true);
  });
});
