// Back-compat re-export: the Grok Build ACP driver moved to acp/grok.ts,
// riding the shared acp/core.ts runtime. Kept here so existing imports and
// dist-server references keep resolving.
export { GrokAgentDriver } from "./acp/grok.ts";
