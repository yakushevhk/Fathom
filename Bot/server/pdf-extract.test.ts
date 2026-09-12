import { describe, it, expect } from "vitest";
import { writeFileSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateSync } from "node:zlib";
import { extractTextFromPdf } from "./pdf-extract.ts";

describe("pdf-extract", () => {
  it("returns empty string if file does not exist", () => {
    expect(extractTextFromPdf("/tmp/non-existent-pdf-file.pdf")).toBe("");
  });

  it("handles empty file safely", () => {
    const tempPath = join(tmpdir(), `empty-${Date.now()}.pdf`);
    writeFileSync(tempPath, Buffer.alloc(0));
    try {
      expect(extractTextFromPdf(tempPath)).toBe("");
    } finally {
      unlinkSync(tempPath);
    }
  });

  it("handles malformed/garbage file safely without throwing", () => {
    const tempPath = join(tmpdir(), `garbage-${Date.now()}.pdf`);
    writeFileSync(tempPath, Buffer.from("Not a real pdf at all, just binary garbage \x00\x01\x02\xff"));
    try {
      expect(extractTextFromPdf(tempPath)).toBe("");
    } finally {
      unlinkSync(tempPath);
    }
  });

  it("extracts text from uncompressed PDF BT...ET block with Tj and TJ operators", () => {
    const tempPath = join(tmpdir(), `uncompressed-${Date.now()}.pdf`);
    const content = `
%PDF-1.4
1 0 obj
<< /Length 120 >>
stream
BT
/F1 12 Tf
(Hello World) Tj
[(This) -10 (is) 5 (a) -20 (test)] TJ
ET
endstream
endobj
trailer
<< /Root 1 0 R >>
%%EOF
`;
    writeFileSync(tempPath, content);
    try {
      const extracted = extractTextFromPdf(tempPath);
      expect(extracted).toContain("Hello World");
      expect(extracted).toContain("This is a test");
    } finally {
      unlinkSync(tempPath);
    }
  });

  it("handles escaped parenthesis and octal characters correctly", () => {
    const tempPath = join(tmpdir(), `escaped-${Date.now()}.pdf`);
    const content = `
%PDF-1.4
stream
BT
(Text with \\(parentheses\\) and octal \\101\\102\\103) Tj
ET
endstream
`;
    writeFileSync(tempPath, content);
    try {
      const extracted = extractTextFromPdf(tempPath);
      expect(extracted).toContain("Text with (parentheses) and octal ABC");
    } finally {
      unlinkSync(tempPath);
    }
  });

  it("extracts text from hex strings <...> Tj and TJ array", () => {
    const tempPath = join(tmpdir(), `hex-${Date.now()}.pdf`);
    // 48656c6c6f20486578 = "Hello Hex"
    const content = `
%PDF-1.4
stream
BT
<48656c6c6f20486578> Tj
ET
endstream
`;
    writeFileSync(tempPath, content);
    try {
      const extracted = extractTextFromPdf(tempPath);
      expect(extracted).toContain("Hello Hex");
    } finally {
      unlinkSync(tempPath);
    }
  });

  it("extracts text from compressed FlateDecode streams", () => {
    const tempPath = join(tmpdir(), `flate-${Date.now()}.pdf`);
    const streamContent = `
BT
/F1 12 Tf
(Compressed stream text verified) Tj
ET
`;
    const compressed = deflateSync(Buffer.from(streamContent));
    const header = Buffer.from("%PDF-1.4\nstream\n");
    const footer = Buffer.from("\nendstream\n%%EOF");
    const fullBuffer = Buffer.concat([header, compressed, footer]);

    writeFileSync(tempPath, fullBuffer);
    try {
      const extracted = extractTextFromPdf(tempPath);
      expect(extracted).toContain("Compressed stream text verified");
    } finally {
      unlinkSync(tempPath);
    }
  });

  it("falls back to printable words when no BT..ET blocks exist", () => {
    const tempPath = join(tmpdir(), `fallback-${Date.now()}.pdf`);
    const content = `%PDF-1.4\nSome printable structured documents keywords metadata here\n%%EOF`;
    writeFileSync(tempPath, content);
    try {
      const extracted = extractTextFromPdf(tempPath);
      expect(extracted).toContain("Some printable structured documents keywords metadata here");
    } finally {
      unlinkSync(tempPath);
    }
  });
});
