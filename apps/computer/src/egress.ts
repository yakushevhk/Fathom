export class EgressPolicyError extends Error {
  readonly status = 400;
  constructor() { super("Navigation target is not allowed"); this.name = "EgressPolicyError"; }
}

const METADATA_HOSTS = new Set([
  "metadata.google.internal", "metadata.google.com", "metadata.azure.com",
  "instance-data.ec2.internal", "metadata", "metadata.aws.internal",
]);
const METADATA_IPS = new Set(["169.254.169.254", "169.254.169.253", "169.254.170.2", "100.100.100.200"]);

export interface EgressOptions { allowPrivateHosts?: boolean; allowedHosts?: string[] }

function wildcard(pattern: string, host: string): boolean {
  const p = pattern.trim().toLowerCase().replace(/^\.+|\.+$/g, "");
  const h = host.toLowerCase().replace(/\.$/, "");
  if (!p) return false;
  if (p === "*") return true;
  if (p.startsWith("*.")) return h === p.slice(2) || h.endsWith(`.${p.slice(2)}`);
  if (p.includes("*")) {
    const escaped = p.split("*").map(part => part.replace(/[\\^$+?.()|[\]{}]/g, "\\$&")).join(".*");
    return new RegExp(`^${escaped}$`, "i").test(h);
  }
  return p === h;
}

function ipv4(host: string): number[] | null {
  const parts = host.split(".");
  if (parts.length !== 4 || parts.some(part => !/^\d+$/.test(part))) return null;
  const values = parts.map(Number);
  return values.every(v => v >= 0 && v <= 255) ? values : null;
}

function mappedIpv4(host: string): string | null {
  const lower = host.toLowerCase();
  // Check standard hex notation: ::ffff:7f00:0001
  const hexMatch = lower.match(/^::ffff:([0-9a-f]{1,4}):([0-9a-f]{1,4})$/);
  if (hexMatch) {
    const high = Number.parseInt(hexMatch[1], 16);
    const low = Number.parseInt(hexMatch[2], 16);
    return `${high >> 8}.${high & 255}.${low >> 8}.${low & 255}`;
  }
  // Check dotted-quad notation: ::ffff:127.0.0.1 or ::ffff:169.254.169.254
  const dotMatch = lower.match(/^::ffff:(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})$/);
  if (dotMatch) {
    return dotMatch[1];
  }
  return null;
}

function privateIp(host: string): boolean {
  const v = ipv4(host);
  if (v) return v[0] === 10 || v[0] === 127 || v[0] === 0 || (v[0] === 172 && v[1] >= 16 && v[1] <= 31) || (v[0] === 192 && v[1] === 168) || (v[0] === 169 && v[1] === 254);
  const h = host.toLowerCase();
  if (h === "::" || h === "::1" || h.startsWith("fe80:") || h.startsWith("fc") || h.startsWith("fd")) return true;
  const mapped = mappedIpv4(h);
  return mapped !== null && privateIp(mapped);
}

function hardDeniedHost(host: string): boolean {
  const v = ipv4(host);
  if (v && v[0] >= 224) return true;
  const h = host.toLowerCase();
  if (h === "localhost" || h === "localhost.localdomain" || h.startsWith("ff")) return true;
  if (METADATA_HOSTS.has(h) || [...METADATA_HOSTS].some(name => h.endsWith(`.${name}`))) return true;
  if (METADATA_IPS.has(h)) return true;
  const mapped = mappedIpv4(h);
  return mapped !== null && hardDeniedHost(mapped);
}

export function validateEgressUrl(value: string, options: EgressOptions = {}): URL {
  let parsed: URL;
  try { parsed = new URL(value); } catch { throw new EgressPolicyError(); }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:" || parsed.username || parsed.password) throw new EgressPolicyError();
  const host = parsed.hostname.replace(/^\[|\]$/g, "").replace(/\.$/, "").toLowerCase();
  if (!host || hardDeniedHost(host)) throw new EgressPolicyError();
  const allowPrivate = options.allowPrivateHosts ?? process.env.COMPUTER_ALLOW_PRIVATE_HOSTS === "true";
  const allowlist = options.allowedHosts ?? (process.env.COMPUTER_ALLOWED_HOSTS || "").split(",").map(x => x.trim()).filter(Boolean);
  if (allowlist.length && !allowlist.some(rule => wildcard(rule, host))) throw new EgressPolicyError();
  if (privateIp(host) && !allowPrivate) throw new EgressPolicyError();
  return parsed;
}

export function currentEgressOptions(): EgressOptions {
  return { allowPrivateHosts: process.env.COMPUTER_ALLOW_PRIVATE_HOSTS === "true", allowedHosts: (process.env.COMPUTER_ALLOWED_HOSTS || "").split(",").map(x => x.trim()).filter(Boolean) };
}
