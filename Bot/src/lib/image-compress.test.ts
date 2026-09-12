import { describe, it, expect } from "vitest";
import { compressImageForUpload } from "./image-compress";

describe("compressImageForUpload", () => {
  it("skips non-image files", async () => {
    const textFile = new File(["hello world"], "test.txt", { type: "text/plain" });
    const result = await compressImageForUpload(textFile);
    expect(result).toBe(textFile);
  });

  it("skips gif and svg files", async () => {
    const gifFile = new File([new Uint8Array(400 * 1024)], "anim.gif", { type: "image/gif" });
    const svgFile = new File([new Uint8Array(400 * 1024)], "vector.svg", { type: "image/svg+xml" });
    expect(await compressImageForUpload(gifFile)).toBe(gifFile);
    expect(await compressImageForUpload(svgFile)).toBe(svgFile);
  });

  it("skips images smaller than 300KB", async () => {
    const smallFile = new File([new Uint8Array(100 * 1024)], "small.png", { type: "image/png" });
    const result = await compressImageForUpload(smallFile);
    expect(result).toBe(smallFile);
  });
});
