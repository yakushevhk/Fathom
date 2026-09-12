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

const FALLBACK_MODELS = [
  "x/grok-4.5",
  "x/grok-4.6",
  "alibaba/qwen3.8-max",
  "alibaba/qwen3.7-plus",
  "alibaba/qwen3.7-max",
  "alibaba/qwen3.6-flash",
  "alibaba/deepseek-v4-pro",
  "alibaba/glm-5.2",
  "ds/deepseek-v4-flash",
  "ds/deepseek-v4-pro",
  "xiaomi/mimo-v2.5-pro",
  "xiaomi/mimo-v2.5",
  "antigravity/gemini-3.8-flash-high",
  "antigravity/gemini-3.8-flash-medium",
  "antigravity/gemini-3.8-flash-low",
  "antigravity/gemini-3.7-flash-high",
  "antigravity/gemini-3.7-flash-medium",
  "antigravity/gemini-3.7-flash-low",
  "antigravity/gemini-3.1-pro-high",
  "antigravity/gemini-pro-agent",
  "antigravity/claude-sonnet-4-6",
  "antigravity/claude-opus-4-6-thinking",
  "antigravity/gpt-oss-120b-medium",
  "kimi/k3",
  "sfkey/kimi-k3",
  "vectide/kimi-k3",
  "vectide2/kimi-k3",
  "vectide3/kimi-k3",
  "vectide3/deepseek-v4-flash-0731",
  "vectide3/deepseek-v4-pro-0813",
  "vectide3/glm-5",
  "vectide3/glm-5.1",
  "vectide3/glm-5.2",
  "vectide3/glm-5.3",
  "vectide3/MiniMax-M2.1-highspeed",
  "vectide3/MiniMax-M2.5",
  "vectide3/MiniMax-M2.5-highspeed",
  "vectide3/MiniMax-M2.7",
  "vectide3/MiniMax-M2.7-highspeed",
  "vectide3/MiniMax-M3",
  "claude-opus-4-8",
  "claude-opus-5",
  "gpt-5.6-sol",
  "or/ox",
  "wave/ghost",
  "wave/max",
  "wave/medium",
  "wave/fast",
  "tencent/hy4"
];

async function fetchModels() {
  try {
    const res = await fetch(`${ROUTER_URL}/models`, {
      headers: { Authorization: `Bearer ${ROUTER_KEY}` },
      signal: AbortSignal.timeout(6000),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const json = await res.json();
    const list = Array.isArray(json) ? json : json?.data;
    if (Array.isArray(list) && list.length > 0) {
      const ids = list.map((m) => m?.id).filter(Boolean);
      if (ids.length > 0) return ids;
    }
  } catch (err) {
    console.warn(`[setup-router-agents] Fetch models failed (${err.message}), using fallback model list.`);
  }
  return FALLBACK_MODELS;
}

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
  console.log(`[setup-router-agents] Configuring agents for router: ${ROUTER_URL}`);
  const models = await fetchModels();
  console.log(`[setup-router-agents] Available models count: ${models.length}`);

  const defaultModel = models.includes("antigravity/gemini-3.8-flash-high")
    ? "antigravity/gemini-3.8-flash-high"
    : models[0];

  // 1. Claude (~/.claude/settings.json)
  const claudeSettingsPath = join(DATA_DIR, ".claude", "settings.json");
  ensureJson(claudeSettingsPath, (prev) => {
    prev.env = prev.env || {};
    prev.env.ANTHROPIC_API_KEY = ROUTER_KEY;
    prev.env.ANTHROPIC_BASE_URL = ROUTER_URL;
    const currentCustom = new Set(prev.customModels || []);
    for (const m of models) currentCustom.add(m);
    prev.customModels = Array.from(currentCustom);
    return prev;
  });

  // 2. OpenCode (~/.config/opencode/opencode.json & ~/.local/share/opencode/auth.json)
  const opencodeConfigPath = join(DATA_DIR, ".config", "opencode", "opencode.json");
  ensureJson(opencodeConfigPath, (prev) => {
    prev.$schema = "https://opencode.ai/config.json";
    prev.provider = prev.provider || {};
    const modelObj = {};
    for (const m of models) {
      modelObj[m] = { name: m };
    }
    prev.provider.router = {
      npm: "@ai-sdk/openai-compatible",
      name: "Helium Router",
      options: {
        baseURL: ROUTER_URL,
        apiKey: ROUTER_KEY,
      },
      models: modelObj,
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
    prev.access_token = prev.access_token || ROUTER_KEY;
    return prev;
  });

  const grokConfigPath = join(DATA_DIR, ".grok", "config.toml");
  let grokToml = `[models]\ndefault = "${defaultModel}"\n\n`;
  for (const m of models) {
    const slug = m.replace(/[^a-zA-Z0-9._/-]/g, "_");
    grokToml += `[model."${slug}"]\n`;
    grokToml += `model = "${m}"\n`;
    grokToml += `base_url = "${ROUTER_URL}"\n`;
    grokToml += `name = "${m}"\n`;
    grokToml += `api_backend = "chat_completions"\n`;
    grokToml += `api_key = "${ROUTER_KEY}"\n\n`;
  }
  ensureFile(grokConfigPath, grokToml);

  // 4. Qwen (~/.qwen/settings.json)
  const qwenSettingsPath = join(DATA_DIR, ".qwen", "settings.json");
  ensureJson(qwenSettingsPath, (prev) => {
    prev.env = prev.env || {};
    prev.env.OPENMAUSBOT_ROUTER_KEY = ROUTER_KEY;
    prev.modelProviders = prev.modelProviders || {};
    const openaiModels = models.map((m) => ({
      id: m,
      name: m,
      baseUrl: ROUTER_URL,
      envKey: "OPENMAUSBOT_ROUTER_KEY",
    }));
    prev.modelProviders.openai = openaiModels;
    return prev;
  });

  // 5. Hermes (~/.hermes/config.yaml & ~/.hermes/.env)
  const hermesEnvPath = join(DATA_DIR, ".hermes", ".env");
  const hermesEnvContent = `OPENROUTER_API_KEY=${ROUTER_KEY}\nOPENROUTER_BASE_URL=${ROUTER_URL}\n`;
  ensureFile(hermesEnvPath, hermesEnvContent);

  const hermesConfigPath = join(DATA_DIR, ".hermes", "config.yaml");
  let hermesYaml = `model:\n  default: "router/${defaultModel}"\n\nproviders:\n  router:\n    base_url: "${ROUTER_URL}"\n    api_key: "${ROUTER_KEY}"\n`;
  for (const m of models) {
    hermesYaml += `  "router/${m}":\n    base_url: "${ROUTER_URL}"\n    api_key: "${ROUTER_KEY}"\n`;
  }
  ensureFile(hermesConfigPath, hermesYaml);
  // 6. Pi (~/.pi/agent/models.json & ~/.pi/agent/settings.json & ~/.pi/agent/auth.json)
  const piAuthPath = join(DATA_DIR, ".pi", "agent", "auth.json");
  ensureJson(piAuthPath, (prev) => prev);

  const piModelsPath = join(DATA_DIR, ".pi", "agent", "models.json");
  ensureJson(piModelsPath, (prev) => {
    prev.providers = prev.providers || {};
    prev.providers.router = {
      baseUrl: ROUTER_URL,
      api: "openai-completions",
      apiKey: ROUTER_KEY,
      compat: {
        supportsDeveloperRole: false,
        supportsReasoningEffort: true,
      },
      models: models.map((m) => ({
        id: m,
        name: m,
        reasoning: true,
        input: ["text"],
        contextWindow: 131072,
        maxTokens: 16384,
      })),
    };
    return prev;
  });

  const piSettingsPath = join(DATA_DIR, ".pi", "agent", "settings.json");
  ensureJson(piSettingsPath, (prev) => {
    prev.defaultProvider = "router";
    prev.defaultModel = defaultModel;
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
      prev.openaiCompat.model = defaultModel;
      prev.defaultModelSelection = prev.defaultModelSelection || {
        instanceId: "openaiCompat",
        model: defaultModel,
      };
      return prev;
    });
  }

  console.log("[setup-router-agents] Successfully configured all 6 agents + Parallel.");
}

if (import.meta.url === `file://${process.argv[1]}`) {
  setupRouterAgents().catch((err) => {
    console.error("[setup-router-agents] Error:", err);
    process.exit(1);
  });
}
