import { readFileSync, existsSync } from "node:fs";
import { inflateSync } from "node:zlib";

function decodePdfString(str: string): string {
  return str
    .replace(/\\([0-7]{1,3})/g, (_, oct) => String.fromCharCode(parseInt(oct, 8)))
    .replace(/\\n/g, "\n")
    .replace(/\\r/g, "\r")
    .replace(/\\t/g, "\t")
    .replace(/\\b/g, "\b")
    .replace(/\\f/g, "\f")
    .replace(/\\\(/g, "(")
    .replace(/\\\)/g, ")")
    .replace(/\\\\/g, "\\");
}

function decodeHexString(hex: string): string {
  const clean = hex.replace(/\s+/g, "");
  const padded = clean.length % 2 !== 0 ? clean + "0" : clean;
  try {
    return Buffer.from(padded, "hex").toString("latin1");
  } catch {
    return "";
  }
}

function parsePdfStreamContent(streamText: string, textPieces: string[]): void {
  // Match text blocks within BT ... ET blocks
  const btEtRegex = /BT[\s\S]*?ET/g;
  let match: RegExpExecArray | null;

  while ((match = btEtRegex.exec(streamText)) !== null) {
    const block = match[0];

    // 1. Literal string Tj: (text) Tj
    const tjRegex = /\(((?:[^()\\]|\\.)*)\)\s*Tj/g;
    let tjMatch: RegExpExecArray | null;
    while ((tjMatch = tjRegex.exec(block)) !== null) {
      textPieces.push(decodePdfString(tjMatch[1]));
    }

    // 2. Hex string Tj: <48656c6c6f> Tj
    const hexTjRegex = /<([0-9a-fA-F\s]+)>\s*Tj/g;
    let hexTjMatch: RegExpExecArray | null;
    while ((hexTjMatch = hexTjRegex.exec(block)) !== null) {
      const decoded = decodeHexString(hexTjMatch[1]);
      if (decoded) textPieces.push(decoded);
    }

    // 3. TJ array syntax: [(string) -10 (string)] TJ or [<48656c6c6f> 10 <20576f726c64>] TJ
    const arrayTjRegex = /\[([\s\S]*?)\]\s*TJ/g;
    let arrayMatch: RegExpExecArray | null;
    while ((arrayMatch = arrayTjRegex.exec(block)) !== null) {
      const inner = arrayMatch[1];
      const elemRegex = /\(((?:[^()\\]|\\.)*)\)|<([0-9a-fA-F\s]+)>/g;
      let elemMatch: RegExpExecArray | null;
      while ((elemMatch = elemRegex.exec(inner)) !== null) {
        if (elemMatch[1] !== undefined) {
          textPieces.push(decodePdfString(elemMatch[1]));
        } else if (elemMatch[2] !== undefined) {
          const decoded = decodeHexString(elemMatch[2]);
          if (decoded) textPieces.push(decoded);
        }
      }
    }

    // 4. Single quote operator: (string) '
    const quoteRegex = /\(((?:[^()\\]|\\.)*)\)\s*'/g;
    let quoteMatch: RegExpExecArray | null;
    while ((quoteMatch = quoteRegex.exec(block)) !== null) {
      textPieces.push(decodePdfString(quoteMatch[1]));
    }
  }
}

/** Extract readable ASCII text from PDF binary streams without native dependencies */
export function extractTextFromPdf(filePath: string): string {
  if (!existsSync(filePath)) return "";
  try {
    const buffer = readFileSync(filePath);
    if (!buffer || buffer.length === 0) return "";

    const content = buffer.toString("binary");
    // Check if buffer looks like a PDF
    const isPdf = buffer.slice(0, 1024).includes("%PDF");
    if (!isPdf && !content.includes("BT") && !content.includes("stream")) {
      return "";
    }

    const textPieces: string[] = [];

    // Parse uncompressed BT ... ET blocks in the body
    parsePdfStreamContent(content, textPieces);

    // Decompress FlateDecode streams if present: stream ... endstream
    const streamRegex = /stream\r?\n([\s\S]*?)\r?\nendstream/g;
    let streamMatch: RegExpExecArray | null;
    while ((streamMatch = streamRegex.exec(content)) !== null) {
      const startPos = streamMatch.index + streamMatch[0].indexOf("\n") + 1;
      const rawStream = buffer.subarray(startPos, startPos + streamMatch[1].length);
      try {
        const decompressed = inflateSync(rawStream).toString("latin1");
        parsePdfStreamContent(decompressed, textPieces);
      } catch {
        // Stream may not be deflated or is another filter (e.g. DCTDecode, JBIG2), ignore safely
      }
    }

    const raw = textPieces.join(" ").replace(/[^\x20-\x7E\n\r\t]/g, " ").replace(/\s+/g, " ").trim();
    if (raw.length > 0) {
      return raw.slice(0, 32_000);
    }

    // Fallback: extract printable ASCII words across the entire file if it has PDF markers
    if (isPdf) {
      const printable = content.replace(/[^\x20-\x7E\n]/g, " ");
      const words = printable.split(/\s+/).filter(w => w.length > 3 && /^[a-zA-Z0-9.,;:!?'"()-]+$/.test(w));
      return words.slice(0, 4_000).join(" ");
    }
    return "";
  } catch (err) {
    console.error("[PDF Extract Error]", err);
    return "";
  }
}
