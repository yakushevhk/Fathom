import type { Page } from "playwright";
import type { ControlState } from "./control.js";

export const MAX_INPUT_MESSAGE_BYTES = 64 * 1024;
export const INPUT_ACTION_TIMEOUT_MS = 5_000;

type InputMessage =
  | { type: "mouse"; x: number; y: number; button?: "left" | "right" | "middle" }
  | { type: "wheel"; deltaX: number; deltaY: number }
  | { type: "key"; key: string; code?: string }
  | { type: "text"; text: string }
  | { type: "resize"; width: number; height: number }
  | { type: "ping" };

/** Validates and dispatches the small, deliberately non-eval input protocol. */
export class InputDispatcher {
  constructor(private readonly control: ControlState, private readonly page: () => Page) {}

  async dispatch(value: unknown): Promise<"pong" | void> {
    const input = parseInput(value);
    if (input.type === "ping") return "pong";
    this.control.assertHuman();
    const operation = this.apply(input);
    await withTimeout(operation, INPUT_ACTION_TIMEOUT_MS);
  }

  private async apply(input: Exclude<InputMessage, { type: "ping" }>): Promise<void> {
    const page = this.page();
    switch (input.type) {
      case "mouse":
        await page.mouse.click(input.x, input.y, { button: input.button });
        return;
      case "wheel":
        await page.mouse.wheel(input.deltaX, input.deltaY);
        return;
      case "key":
        // Playwright accepts DOM key values (not KeyboardEvent.code); retain
        // code in the validated message for protocol compatibility.
        await page.keyboard.press(input.key);
        return;
      case "text":
        await page.keyboard.insertText(input.text);
        return;
      case "resize":
        await page.setViewportSize({ width: input.width, height: input.height });
        return;
    }
  }
}

function parseInput(value: unknown): InputMessage {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Input message must be an object");
  const message = value as Record<string, unknown>;
  if (typeof message.type !== "string") throw new Error("Input message type is required");
  switch (message.type) {
    case "mouse":
      return { type: "mouse", x: finiteNumber(message.x, "x"), y: finiteNumber(message.y, "y"), button: parseButton(message.button) };
    case "wheel":
      return { type: "wheel", deltaX: finiteNumber(message.deltaX, "deltaX"), deltaY: finiteNumber(message.deltaY, "deltaY") };
    case "key":
      return { type: "key", key: nonEmptyString(message.key, "key"), code: optionalString(message.code, "code") };
    case "text": {
      const text = nonEmptyString(message.text, "text");
      if (Buffer.byteLength(text, "utf8") > MAX_INPUT_MESSAGE_BYTES) throw new Error("text is too large");
      return { type: "text", text };
    }
    case "resize":
      return { type: "resize", width: boundedInteger(message.width, "width", 1, 8_192), height: boundedInteger(message.height, "height", 1, 8_192) };
    case "ping":
      return { type: "ping" };
    default:
      throw new Error("Unsupported input message type");
  }
}

function finiteNumber(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`${field} must be a finite number`);
  return value;
}

function boundedInteger(value: unknown, field: string, min: number, max: number): number {
  if (!Number.isInteger(value) || (value as number) < min || (value as number) > max) throw new Error(`${field} must be an integer between ${min} and ${max}`);
  return value as number;
}

function nonEmptyString(value: unknown, field: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`${field} must be a non-empty string`);
  return value;
}

function optionalString(value: unknown, field: string): string | undefined {
  if (value === undefined) return undefined;
  return nonEmptyString(value, field);
}

function parseButton(value: unknown): "left" | "right" | "middle" | undefined {
  if (value === undefined) return undefined;
  if (value !== "left" && value !== "right" && value !== "middle") throw new Error("button must be left, right, or middle");
  return value;
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<T>((_, reject) => { timer = setTimeout(() => reject(new Error("Input action timed out")), timeoutMs); }),
    ]);
  } finally {
    if (timer) clearTimeout(timer);
  }
}
