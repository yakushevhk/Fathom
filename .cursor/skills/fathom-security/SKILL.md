---
name: fathom-security
description: Enterprise security guidelines, on-premise sovereignty, air-gapped guarantees, sandboxing, and compliance with Uzbekistan Law #ZRU-547. Use when auditing security, adding tools, configuring servers, or implementing policy controls.
---

# Enterprise Security, Sovereignty & Compliance

Fathom is built for zero-trust enterprise and government deployments requiring strict isolation and regulatory compliance.

## 1. Compliance: Uzbekistan Law #ZRU-547 (Data Localization)

* **Article 28-1 Requirement**: All personal data of citizens of Uzbekistan must be collected, processed, and stored exclusively on physical servers located within the territory of Uzbekistan.
* **Fathom Invariant**:
  * Single statically linked binary with zero cloud dependencies.
  * Local database storage (SQLite WAL or on-premise PostgreSQL).
  * Local or private-cloud LLM weights (vLLM, Ollama, llama.cpp).
  * Strict zero-exfiltration: no telemetry, third-party analytics, or remote logging.

## 2. Command Execution Sandboxing

* Any execution of arbitrary host commands via `ShellTool` in `crates/tools/src/shell.rs` MUST be routed through `pr_supervisor::HostSandbox`.
* **Linux**: Bubblewrap (`bwrap`) unsharing PID, IPC, UTS, and network namespaces with read-only root binds (`/usr`, `/bin`, `/lib`, `/etc`) and isolated writable workspace binds.
* **macOS**: `sandbox-exec` profiles restricting write access strictly to the session working directory.
* **Emergency override**: `FATHOM_DISABLE_SANDBOX=1` (use only in trusted CI environments).

## 3. Network Egress & SSRF Protection

* All HTTP fetching tools (`web_fetch`, `web_crawl`) enforce `ensure_safe_url()` in `crates/tools/src/guard.rs`.
* **Blocked ranges**: Private subnets (RFC 1918), loopback (`127.0.0.0/8`), link-local/cloud metadata (`169.254.169.254`), broadcast, and multicast addresses.
* Callers must validate both domain resolution and redirects to mitigate DNS-rebinding windows.

## 4. Credentials & Secret Vault

* Passwords, API tokens, and private keys are encrypted using AES-256-GCM.
* Encryption keys are derived via PBKDF2/Argon2.
* Secrets held in memory must be explicitly zeroized on drop (`zeroize` crate) to prevent RAM inspection leaks.

## 5. Timing Attack Protections

* Server API key verification (`crates/server/src/auth.rs`) MUST use constant-time bitwise slice comparison (`diff |= a ^ b`).
* Never use short-circuiting string equality `==` for secrets or tokens.
