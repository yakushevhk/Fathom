// What a tool call is about to do, in words a chip or a permission card can
// show. Redacted before it is cut: a command line is where credentials get
// pasted, and a key sliced in half would slip past the shapes redaction knows.
import { redactSecretsInText } from "./redact.ts";

const QUESTION_LIMIT = 300;
const LIMIT = 200;

function fieldsOf(input: unknown): Record<string, unknown> | undefined {
  if (input === null || typeof input !== "object" || Array.isArray(input)) return undefined;
  return input as Record<string, unknown>;
}

const cut = (text: string, limit: number) => redactSecretsInText(text).trim().slice(0, limit);

/** The shell command a tool call runs, on one redacted line of at most 200
 * characters — what rides beside the tool name on the chip and what the
 * Verify card reads as a step. Only a command: a Read's path or a fetch's
 * URL is not something the bot ran, so those calls carry no summary. */
export function commandSummary(input: unknown): string | undefined {
  const fields = fieldsOf(input);
  if (!fields) return undefined;

  // 1. Direct shell command
  if (typeof fields.command === "string") {
    return cut(fields.command.replace(/\s*[\r\n]+\s*/g, " "), LIMIT);
  }

  // 2. Subagents / task delegation
  if (Array.isArray(fields.tasks)) {
    const count = fields.tasks.length;
    const taskDetails = fields.tasks
      .map((t) => {
        if (!t || typeof t !== "object") return null;
        const entry = t as Record<string, unknown>;
        return (
          (typeof entry.name === "string" ? entry.name : null) ||
          (typeof entry.agent === "string" ? entry.agent : null) ||
          (typeof entry.task === "string" ? entry.task.slice(0, 30) : null)
        );
      })
      .filter(Boolean)
      .join(", ");
    return cut(`${count} subagent${count === 1 ? "" : "s"}${taskDetails ? `: ${taskDetails}` : ""}`, LIMIT);
  }
  if (typeof fields.task === "string") {
    const agent = fields.agent ? ` (${fields.agent})` : "";
    return cut(`Task${agent}: ${fields.task.replace(/\s*[\r\n]+\s*/g, " ")}`, LIMIT);
  }

  // 3. Delegation & Peer Messaging
  if (typeof fields.message === "string" && (fields.bot_id || fields.toBotId)) {
    const target = fields.bot_id || fields.toBotId;
    return cut(`@${target}: ${fields.message.replace(/\s*[\r\n]+\s*/g, " ")}`, LIMIT);
  }

  // 4. Web search & Queries
  if (typeof fields.query === "string") {
    return cut(`"${fields.query.replace(/\s*[\r\n]+\s*/g, " ")}"`, LIMIT);
  }

  return undefined;
}
/** The permission card's subtitle: the question asked, else the command as
 * the bot wrote it (newlines kept — a multi-line command reads on the card
 * the way it will run), else the URL, else the arguments as JSON. Undefined
 * when there is nothing to say (no input, or an empty object). */
export function askInputSummary(input: unknown): string | undefined {
  const fields = fieldsOf(input);
  if (!fields) return undefined;
  if (typeof fields.question === "string") return cut(fields.question, QUESTION_LIMIT);
  if (typeof fields.command === "string") return cut(fields.command, LIMIT);
  if (typeof fields.url === "string") return cut(fields.url, LIMIT);
  const text = JSON.stringify(fields);
  return text === "{}" ? undefined : cut(text, LIMIT);
}
