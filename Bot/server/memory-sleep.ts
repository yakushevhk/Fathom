// Memory Sleep & Consolidation Engine (Сон ботов).
// Analyzes accumulated facts in SQLite bot_facts and daily logs,
// resolves contradictions, lowers confidence on outdated facts, and merges duplicates.

import { db } from "./message-db.ts";
import { listFacts, saveFact, deleteFact, type MemoryFact } from "./structured-memory.ts";

export interface SleepConsolidationReport {
  botId: string;
  sleptAt: number;
  factsProcessed: number;
  duplicatesRemoved: number;
  conflictsResolved: number;
  confidenceAdjusted: number;
  summary: string;
}

export interface BotSleepState {
  botId: string;
  lastSleptAt: number | null;
  sleepCycleCount: number;
  summary?: string;
}

export function recordBotSleep(botId: string, summary: string): BotSleepState {
  const now = Date.now();
  db().exec(`
    CREATE TABLE IF NOT EXISTS bot_sleep_state (
      bot_id TEXT PRIMARY KEY,
      last_slept_at INTEGER NOT NULL,
      sleep_cycle_count INTEGER NOT NULL DEFAULT 0,
      summary TEXT
    );
  `);

  db().prepare(`
    INSERT INTO bot_sleep_state (bot_id, last_slept_at, sleep_cycle_count, summary)
    VALUES (?, ?, 1, ?)
    ON CONFLICT(bot_id) DO UPDATE SET
      last_slept_at = excluded.last_slept_at,
      sleep_cycle_count = sleep_cycle_count + 1,
      summary = excluded.summary
  `).run(botId, now, summary);

  const row = db().prepare("SELECT bot_id, last_slept_at, sleep_cycle_count, summary FROM bot_sleep_state WHERE bot_id = ?").get(botId) as {
    bot_id: string;
    last_slept_at: number;
    sleep_cycle_count: number;
    summary: string | null;
  };

  return {
    botId: row.bot_id,
    lastSleptAt: row.last_slept_at,
    sleepCycleCount: row.sleep_cycle_count,
    summary: row.summary ?? undefined,
  };
}

export function getBotSleepState(botId: string): BotSleepState {
  db().exec(`
    CREATE TABLE IF NOT EXISTS bot_sleep_state (
      bot_id TEXT PRIMARY KEY,
      last_slept_at INTEGER NOT NULL,
      sleep_cycle_count INTEGER NOT NULL DEFAULT 0,
      summary TEXT
    );
  `);

  const row = db().prepare("SELECT bot_id, last_slept_at, sleep_cycle_count, summary FROM bot_sleep_state WHERE bot_id = ?").get(botId) as {
    bot_id: string;
    last_slept_at: number;
    sleep_cycle_count: number;
    summary: string | null;
  } | undefined;

  if (!row) {
    return {
      botId,
      lastSleptAt: null,
      sleepCycleCount: 0,
    };
  }

  return {
    botId: row.bot_id,
    lastSleptAt: row.last_slept_at,
    sleepCycleCount: row.sleep_cycle_count,
    summary: row.summary ?? undefined,
  };
}

export async function executeBotSleep(botId: string): Promise<SleepConsolidationReport> {
  const database = db();
  const now = Date.now();
  let duplicatesRemoved = 0;
  let conflictsResolved = 0;
  let confidenceAdjusted = 0;
  let initialFactsCount = 0;

  database.exec("BEGIN IMMEDIATE");
  try {
    const facts = listFacts(botId);
    initialFactsCount = facts.length;
    const deletedIds = new Set<string>();

    // Group facts by (category, entity) using exact trimmed casing matching SQLite natural key
    const grouped = new Map<string, MemoryFact[]>();
    for (const f of facts) {
      const key = `${f.category}:${f.entity.trim()}`;
      const list = grouped.get(key) ?? [];
      list.push(f);
      grouped.set(key, list);
    }

    for (const [, items] of grouped) {
      if (items.length <= 1) continue;

      // Sort newest first
      items.sort((a, b) => b.updatedAt - a.updatedAt);
      const newest = items[0];

      // Remove older duplicate facts for the same entity
      for (let i = 1; i < items.length; i++) {
        deleteFact(botId, items[i].id);
        deletedIds.add(items[i].id);
        duplicatesRemoved++;
      }

      // Ensure the newest fact has high confidence
      if (newest.confidence < 1.0) {
        saveFact(botId, {
          id: newest.id,
          category: newest.category,
          entity: newest.entity,
          fact: newest.fact,
          confidence: 1.0,
          updatedAt: newest.updatedAt,
        });
        conflictsResolved++;
      }
    }

    // Decay confidence of surviving facts older than 30 days that haven't been updated
    const THIRTY_DAYS_MS = 30 * 24 * 60 * 60_000;
    for (const f of facts) {
      if (deletedIds.has(f.id)) continue; // Skip already deleted duplicates
      if (now - f.updatedAt > THIRTY_DAYS_MS && f.confidence > 0.6) {
        saveFact(botId, {
          id: f.id,
          category: f.category,
          entity: f.entity,
          fact: f.fact,
          confidence: Math.max(0.5, f.confidence - 0.2),
          updatedAt: f.updatedAt, // Preserve original timestamp so age isn't reset
        });
        confidenceAdjusted++;
      }
    }

    const summary = `Сон завершен: консолидировано ${initialFactsCount} фактов, удалено ${duplicatesRemoved} дубликатов, обновлено ${conflictsResolved + confidenceAdjusted} записей.`;
    recordBotSleep(botId, summary);
    database.exec("COMMIT");

    return {
      botId,
      sleptAt: now,
      factsProcessed: initialFactsCount,
      duplicatesRemoved,
      conflictsResolved,
      confidenceAdjusted,
      summary,
    };
  } catch (error) {
    database.exec("ROLLBACK");
    throw error;
  }
}
