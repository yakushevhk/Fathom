import { readFileSync, existsSync } from "node:fs";

/** Extract readable ASCII text from PDF binary streams without native dependencies */
export function extractTextFromPdf(filePath: string): string {
  if (!existsSync(filePath)) return "";
  try {
    const buffer = readFileSync(filePath);
    const content = buffer.toString("binary");
    const textPieces: string[] = [];

    // Match text blocks within BT ... ET blocks
    const btEtRegex = /BT[\s\S]*?ET/g;
    let match: RegExpExecArray | null;

    while ((match = btEtRegex.exec(content)) !== null) {
      const block = match[0];
      // Match text shown with Tj or TJ operators
      const tjRegex = /\(([\s\S]*?)\)\s*Tj/g;
      let tjMatch: RegExpExecArray | null;
      while ((tjMatch = tjRegex.exec(block)) !== null) {
        const decoded = tjMatch[1]
          .replace(/\\n/g, "\n")
          .replace(/\\r/g, "\r")
          .replace(/\\t/g, "\t")
          .replace(/\\b/g, "\b")
          .replace(/\\f/g, "\f")
          .replace(/\\\(/g, "(")
          .replace(/\\\)/g, ")")
          .replace(/\\\\/g, "\\");
        textPieces.push(decoded);
      }
      
      // Handle TJ array syntax: [(string) -10 (string)] TJ
      const arrayTjRegex = /\[([\s\S]*?)\]\s*TJ/g;
      let arrayMatch: RegExpExecArray | null;
      while ((arrayMatch = arrayTjRegex.exec(block)) !== null) {
        const inner = arrayMatch[1];
        const stringRegex = /\(([\s\S]*?)\)/g;
        let strMatch: RegExpExecArray | null;
        while ((strMatch = stringRegex.exec(inner)) !== null) {
          textPieces.push(strMatch[1]);
        }
      }
    }

    const raw = textPieces.join(" ").replace(/[^\x20-\x7E\n\r\t]/g, " ").trim();
    if (raw.length > 50) {
      return raw.slice(0, 32_000);
    }
    // Fallback: extract printable strings
    const printable = content.replace(/[^\x20-\x7E\n]/g, " ");
    const words = printable.split(/\s+/).filter(w => w.length > 3 && /^[a-zA-Z0-9.,;:!?'"()-]+$/.test(w));
    return words.slice(0, 4_000).join(" ");
  } catch (err) {
    console.error("[PDF Extract Error]", err);
    return "";
  }
}
