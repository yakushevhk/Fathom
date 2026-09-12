#!/usr/bin/env node
/**
 * setup-router-agents.mjs
 * 
 * Provisions router.y7.hk configuration and models for all 6 coding agents:
 * - opencode (~/.config/opencode/opencode.json, ~/.local/share/opencode/auth.json)
 * - grok (~/.grok/config.toml, ~/.grok/auth.json)
 * - claude (~/.claude/settings.json)
 * - qwen (~/.qwen/settings.json)
 * - hermes (~/.hermes/config.yaml, ~/.hermes/.env)
 * - pi (~/.pi/agent/models.json, ~/.pi/agent/settings.json, ~/.pi/agent/auth.json)
 * - openmausbot (~/.openmausbot/config.json)
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const ROUTER_URL = (process.env.ROUTER_BASE_URL || process.env.OPENAI_BASE_URL || "https://router.y7.hk/v1").replace(/\/+$/, "");
const ROUTER_KEY = process.env.ROUTER_API_KEY || process.env.OPENAI_API_KEY || "sk-haus";
const DATA_DIR = process.env.HOME || "/data";

const ONLY_MODEL = "antigravity/gemini-3.8-flash-high";

function ensureJson(path, updater) {
  mkdirSync(join(path, ".."), { recursive: true });
  let data = {};
  if (existsSync(path)) {
    try {
      data = JSON.parse(readFileSync(path, "utf8"));
    } catch {}
  }
  const next = updater(data) || data;
  writeFileSync(path, JSON.stringify(next, null, 2) + "\n");
}

function ensureFile(path, content) {
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, content);
}

export async function setupRouterAgents() {
  console.log(`[setup-router-agents] Configuring agents exclusively for: ${ONLY_MODEL}`);

  // 1. Claude (~/.claude/settings.json)
  const claudeSettingsPath = join(DATA_DIR, ".claude", "settings.json");
  ensureJson(claudeSettingsPath, (prev) => {
    prev.env = prev.env || {};
    prev.env.ANTHROPIC_API_KEY = ROUTER_KEY;
    prev.env.ANTHROPIC_BASE_URL = ROUTER_URL;
    prev.customModels = [ONLY_MODEL];
    return prev;
  });
  // 2. OpenCode (~/.config/opencode/opencode.json & ~/.local/share/opencode/auth.json)
  const opencodeConfigPath = join(DATA_DIR, ".config", "opencode", "opencode.json");
  ensureJson(opencodeConfigPath, (prev) => {
    prev.$schema = "https://opencode.ai/config.json";
    prev.provider = {
      router: {
        npm: "@ai-sdk/openai-compatible",
        name: "Helium Router",
        options: {
          baseURL: ROUTER_URL,
          apiKey: ROUTER_KEY,
        },
        models: {
          [ONLY_MODEL]: { name: "Gemini 3.8 Flash High" },
        },
      },
    };
    return prev;
  });
  const opencodeAuthPath = join(DATA_DIR, ".local", "share", "opencode", "auth.json");
  ensureJson(opencodeAuthPath, (prev) => {
    prev.router = { key: ROUTER_KEY };
    return prev;
  });

  // 3. Grok (~/.grok/config.toml & ~/.grok/auth.json)
  const grokAuthPath = join(DATA_DIR, ".grok", "auth.json");
  ensureJson(grokAuthPath, (prev) => {
    prev.access_token = ROUTER_KEY;
    return prev;
  });

  const grokConfigPath = join(DATA_DIR, ".grok", "config.toml");
  const grokSlug = ONLY_MODEL.replace(/[^a-zA-Z0-9._/-]/g, "_");
  const grokToml = `[models]\ndefault = "${grokSlug}"\n\n[model."${grokSlug}"]\nmodel = "${ONLY_MODEL}"\nbase_url = "${ROUTER_URL}"\nname = "${ONLY_MODEL}"\napi_backend = "chat_completions"\napi_key = "${ROUTER_KEY}"\n`;
  ensureFile(grokConfigPath, grokToml);

  // 4. Qwen (~/.qwen/settings.json)
  const qwenSettingsPath = join(DATA_DIR, ".qwen", "settings.json");
  ensureJson(qwenSettingsPath, (prev) => {
    prev.env = prev.env || {};
    prev.env.OPENMAUSBOT_ROUTER_KEY = ROUTER_KEY;
    prev.modelProviders = {
      openai: [
        {
          id: ONLY_MODEL,
          name: "Gemini 3.8 Flash High",
          baseUrl: ROUTER_URL,
          envKey: "OPENMAUSBOT_ROUTER_KEY",
        },
      ],
    };
    return prev;
  });

  // 5. Hermes (~/.hermes/config.yaml & ~/.hermes/.env)
  const hermesEnvPath = join(DATA_DIR, ".hermes", ".env");
  const hermesEnvContent = `OPENROUTER_API_KEY=${ROUTER_KEY}\nOPENROUTER_BASE_URL=${ROUTER_URL}\n`;
  ensureFile(hermesEnvPath, hermesEnvContent);

  const hermesConfigPath = join(DATA_DIR, ".hermes", "config.yaml");
  const hermesYaml = `model:\n  default: "router/${ONLY_MODEL}"\n\nproviders:\n  router:\n    base_url: "${ROUTER_URL}"\n    api_key: "${ROUTER_KEY}"\n  "router/${ONLY_MODEL}":\n    base_url: "${ROUTER_URL}"\n    api_key: "${ROUTER_KEY}"\n`;
  ensureFile(hermesConfigPath, hermesYaml);

  // 6. Pi (~/.pi/agent/models.json & ~/.pi/agent/settings.json & ~/.pi/agent/auth.json)
  const piAuthPath = join(DATA_DIR, ".pi", "agent", "auth.json");
  ensureJson(piAuthPath, (prev) => prev);

  const piModelsPath = join(DATA_DIR, ".pi", "agent", "models.json");
  ensureJson(piModelsPath, (prev) => {
    prev.providers = {
      router: {
        baseUrl: ROUTER_URL,
        api: "openai-completions",
        apiKey: ROUTER_KEY,
        compat: {
          supportsDeveloperRole: false,
          supportsReasoningEffort: true,
        },
        models: [
          {
            id: ONLY_MODEL,
            name: "Gemini 3.8 Flash High",
            reasoning: true,
            input: ["text"],
            contextWindow: 131072,
            maxTokens: 16384,
          },
        ],
      },
    };
    return prev;
  });

  const piSettingsPath = join(DATA_DIR, ".pi", "agent", "settings.json");
  ensureJson(piSettingsPath, (prev) => {
    prev.defaultProvider = "router";
    prev.defaultModel = ONLY_MODEL;
    return prev;
  });

  // 7. Parallel / OpenMausBot config
  const parallelConfigDir = process.env.OMB_DATA_DIR || join(DATA_DIR, ".parallel");
  const targetDirs = [parallelConfigDir, join(DATA_DIR, ".openmausbot")];
  for (const cfgDir of targetDirs) {
    const cfgPath = join(cfgDir, "config.json");
    ensureJson(cfgPath, (prev) => {
      prev.openaiCompat = prev.openaiCompat || {};
      prev.openaiCompat.url = ROUTER_URL;
      prev.openaiCompat.key = ROUTER_KEY;
      prev.openaiCompat.model = ONLY_MODEL;
      prev.defaultModelSelection = {
        instanceId: "openaiCompat",
        model: ONLY_MODEL,
      };
      return prev;
    });

    // Update existing bots.json to set modelSelection to gemini-3.8-flash-high
    const botsPath = join(cfgDir, "bots.json");
    if (existsSync(botsPath)) {
      ensureJson(botsPath, (prev) => {
        if (Array.isArray(prev)) {
          for (const bot of prev) {
            bot.modelSelection = { instanceId: "openaiCompat", model: ONLY_MODEL };
          }
        }
        return prev;
      });
    }
  }

  console.log(`[setup-router-agents] Successfully configured all 6 agents + Parallel with only ${ONLY_MODEL}.`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  setupRouterAgents().catch((err) => {
    console.error("[setup-router-agents] Error:", err);
    process.exit(1);
  });
}
