import { z } from "zod";

const MAX_ALLOWED_ORIGINS = 20;
const secretSchema = z.string().min(32);
const cloudflareTokenSchema = z.string().min(20).max(2_048).regex(/^\S+$/);
const cloudflareResourceIdSchema = z.string().regex(/^[0-9a-f]{32}$/i);
const emailSchema = z.email().max(254);
const originsSchema = z.string();

export interface ControlPlaneConfig {
  authBaseURL: string;
  allowedOrigins: ReadonlySet<string>;
  cloudflare: {
    accountId: string;
    apiToken: string;
    companionHostSuffix: string;
    zoneId: string;
  };
  emailFrom: string;
}

function exactHTTPSOrigin(value: string, label: string): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`${label} must be a valid HTTPS origin`);
  }
  if (
    url.protocol !== "https:"
    || url.username
    || url.password
    || url.pathname !== "/"
    || url.search
    || url.hash
  ) {
    throw new Error(`${label} must be an exact HTTPS origin`);
  }
  return url.origin;
}

function hostnameSuffix(value: string): string {
  // 34-byte opaque label plus the separating dot must remain within the
  // 253-byte DNS hostname limit.
  if (value !== value.toLowerCase() || value.length > 218 || value.endsWith(".")) {
    throw new Error("COMPANION_HOST_SUFFIX must be a lowercase DNS suffix");
  }
  const labels = value.split(".");
  if (
    labels.length < 2
    || labels.some((label) => !/^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(label))
  ) {
    throw new Error("COMPANION_HOST_SUFFIX must be a valid DNS suffix");
  }
  return value;
}

export function readConfig(env: Env): ControlPlaneConfig {
  if (!secretSchema.safeParse(env.BETTER_AUTH_SECRET).success) {
    throw new Error("BETTER_AUTH_SECRET must contain at least 32 characters");
  }

  const emailFrom = emailSchema.safeParse(env.EMAIL_FROM);
  if (!emailFrom.success) throw new Error("EMAIL_FROM must be a valid email address");

  const authBaseURL = exactHTTPSOrigin(env.BETTER_AUTH_URL, "BETTER_AUTH_URL");
  const origins = originsSchema.safeParse(env.ALLOWED_ORIGINS);
  if (!origins.success) throw new Error("ALLOWED_ORIGINS must be a comma-separated string");
  const values = origins.data.split(",")
    .map((value) => value.trim())
    .filter(Boolean);
  if (values.length > MAX_ALLOWED_ORIGINS) {
    throw new Error("ALLOWED_ORIGINS contains too many entries");
  }
  const allowedOrigins = new Set(values.map((value) => exactHTTPSOrigin(value, "ALLOWED_ORIGINS")));
  allowedOrigins.add(authBaseURL);

  if (!cloudflareResourceIdSchema.safeParse(env.CLOUDFLARE_ACCOUNT_ID).success) {
    throw new Error("CLOUDFLARE_ACCOUNT_ID must be a 32-character Cloudflare ID");
  }
  if (!cloudflareResourceIdSchema.safeParse(env.CLOUDFLARE_ZONE_ID).success) {
    throw new Error("CLOUDFLARE_ZONE_ID must be a 32-character Cloudflare ID");
  }
  if (!cloudflareTokenSchema.safeParse(env.CLOUDFLARE_API_TOKEN).success) {
    throw new Error("CLOUDFLARE_API_TOKEN is missing or invalid");
  }
  const hostSuffix = z.string().min(1).max(218).safeParse(env.COMPANION_HOST_SUFFIX);
  if (!hostSuffix.success) {
    throw new Error("COMPANION_HOST_SUFFIX must be a lowercase DNS suffix");
  }

  return {
    authBaseURL,
    allowedOrigins,
    cloudflare: {
      accountId: env.CLOUDFLARE_ACCOUNT_ID,
      apiToken: env.CLOUDFLARE_API_TOKEN,
      companionHostSuffix: hostnameSuffix(hostSuffix.data),
      zoneId: env.CLOUDFLARE_ZONE_ID,
    },
    emailFrom: emailFrom.data,
  };
}
