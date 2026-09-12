import { env } from "cloudflare:workers";
import { applyD1Migrations, type D1Migration } from "cloudflare:test";
import { afterEach, beforeAll } from "vitest";

declare global {
  namespace Cloudflare {
    interface Env {
      TEST_MIGRATIONS: D1Migration[];
    }
  }
}

beforeAll(async () => {
  await applyD1Migrations(env.DB, env.TEST_MIGRATIONS);
});

afterEach(async () => {
  await env.DB.batch([
    env.DB.prepare("DELETE FROM otp_recipient_rate_limits"),
    env.DB.prepare("DELETE FROM control_action_rate_limits"),
    env.DB.prepare("DELETE FROM installation_action_rate_limits"),
    env.DB.prepare("DELETE FROM installation_endpoints"),
    env.DB.prepare("DELETE FROM installation_credentials"),
    env.DB.prepare("DELETE FROM installations"),
    env.DB.prepare('DELETE FROM "session"'),
    env.DB.prepare('DELETE FROM "account"'),
    env.DB.prepare('DELETE FROM "verification"'),
    env.DB.prepare('DELETE FROM "rateLimit"'),
    env.DB.prepare('DELETE FROM "user"'),
  ]);
});
