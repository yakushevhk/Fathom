// Structured Key-Entity-Fact Memory Store backed by SQLite.
// Complements unstructured MEMORY.md with clean categorized fact tracking.

import { db } from "./message-db.ts";

export interface MemoryFact {
  id: string;
  botId: string;
  category: "preference" | "decision" | "entity" | "system" | "project";
  entity: string;
  fact: string;
  confidence: number;
  updatedAt: number;
}

export const MAX_PROMPT_FACTS = 40;
export const MAX_PROMPT_FACTS_CHARS = 4000;

export function ensureStructuredMemoryTable(): void {
  // Table and unique natural-key index created once in message-db.ts open()
}

export function saveFact(
  botId: string,
  factData: { id?: string; category: MemoryFact["category"]; entity: string; fact: string; confidence?: number; updatedAt?: number },
): MemoryFact {
  const updatedAt = factData.updatedAt ?? Date.now();
  const confidence = factData.confidence ?? 1.0;
  const cleanEntity = factData.entity.trim();
  const cleanFact = factData.fact.replace(/[\r\n]+/g, " ").trim();

  // Check for existing fact by natural key (bot_id, category, entity)
  const existing = db().prepare(`
    SELECT id FROM bot_facts WHERE bot_id = ? AND category = ? AND entity = ?
  `).get(botId, factData.category, cleanEntity) as { id: string } | undefined;

  const id = existing?.id || factData.id || `fact-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;

  db().prepare(`
    INSERT INTO bot_facts (id, bot_id, category, entity, fact, confidence, updated_at)
    VALUES (?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(bot_id, category, entity) DO UPDATE SET
      fact = excluded.fact,
      confidence = excluded.confidence,
      updated_at = excluded.updated_at
  `).run(id, botId, factData.category, cleanEntity, cleanFact, confidence, updatedAt);

  return {
    id,
    botId,
    category: factData.category,
    entity: cleanEntity,
    fact: cleanFact,
    confidence,
    updatedAt,
  };
}

export function listFacts(botId: string, category?: MemoryFact["category"]): MemoryFact[] {
  ensureStructuredMemoryTable();
  if (category) {
    const rows = db().prepare(`
      SELECT id, bot_id, category, entity, fact, confidence, updated_at
      FROM bot_facts
      WHERE bot_id = ? AND category = ?
      ORDER BY updated_at DESC
    `).all(botId, category) as Array<{
      id: string;
      bot_id: string;
      category: MemoryFact["category"];
      entity: string;
      fact: string;
      confidence: number;
      updated_at: number;
    }>;
    return rows.map((r) => ({
      id: r.id,
      botId: r.bot_id,
      category: r.category,
      entity: r.entity,
      fact: r.fact,
      confidence: r.confidence,
      updatedAt: r.updated_at,
    }));
  }

  const rows = db().prepare(`
    SELECT id, bot_id, category, entity, fact, confidence, updated_at
    FROM bot_facts
    WHERE bot_id = ?
    ORDER BY updated_at DESC
  `).all(botId) as Array<{
    id: string;
    bot_id: string;
    category: MemoryFact["category"];
    entity: string;
    fact: string;
    confidence: number;
    updated_at: number;
  }>;

  return rows.map((r) => ({
    id: r.id,
    botId: r.bot_id,
    category: r.category,
    entity: r.entity,
    fact: r.fact,
    confidence: r.confidence,
    updatedAt: r.updated_at,
  }));
}

export function deleteFact(botId: string, factId: string): boolean {
  ensureStructuredMemoryTable();
  const res = db().prepare("DELETE FROM bot_facts WHERE bot_id = ? AND id = ?").run(botId, factId);
  return Number(res.changes) > 0;
}

export function formatFactsAsPromptSection(botId: string): string {
  const facts = listFacts(botId);
  if (!facts.length) return "";
  const lines: string[] = [];
  let charCount = 0;

  for (const f of facts) {
    if (lines.length >= MAX_PROMPT_FACTS) break;
    const safeFact = f.fact.replace(/[\r\n]+/g, " ");
    const line = `- [${f.category}] ${f.entity}: ${safeFact}`;
    if (charCount + line.length > MAX_PROMPT_FACTS_CHARS) break;
    lines.push(line);
    charCount += line.length;
  }

  return lines.length ? `\n\nStructured Facts:\n${lines.join("\n")}` : "";
}
