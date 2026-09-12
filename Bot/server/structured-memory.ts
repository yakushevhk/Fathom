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

export function ensureStructuredMemoryTable(): void {
  db().exec(`
    CREATE TABLE IF NOT EXISTS bot_facts (
      id TEXT PRIMARY KEY,
      bot_id TEXT NOT NULL,
      category TEXT NOT NULL,
      entity TEXT NOT NULL,
      fact TEXT NOT NULL,
      confidence REAL DEFAULT 1.0,
      updated_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_bot_facts_lookup ON bot_facts (bot_id, entity);
    CREATE INDEX IF NOT EXISTS idx_bot_facts_category ON bot_facts (bot_id, category);
  `);
}

export function saveFact(
  botId: string,
  factData: { id?: string; category: MemoryFact["category"]; entity: string; fact: string; confidence?: number },
): MemoryFact {
  ensureStructuredMemoryTable();
  const id = factData.id || `fact-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
  const updatedAt = Date.now();
  const confidence = factData.confidence ?? 1.0;

  db().prepare(`
    INSERT INTO bot_facts (id, bot_id, category, entity, fact, confidence, updated_at)
    VALUES (?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(id) DO UPDATE SET
      category = excluded.category,
      entity = excluded.entity,
      fact = excluded.fact,
      confidence = excluded.confidence,
      updated_at = excluded.updated_at
  `).run(id, botId, factData.category, factData.entity, factData.fact, confidence, updatedAt);

  return {
    id,
    botId,
    category: factData.category,
    entity: factData.entity,
    fact: factData.fact,
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
  const lines = facts.map((f) => `- [${f.category}] ${f.entity}: ${f.fact}`);
  return `\n\nStructured Facts:\n${lines.join("\n")}`;
}
