//! Email verification tool: syntax validation, MX/domain checks, disposable
//! and role-based detection, and an optional SMTP connectivity probe.
//!
//! MX lookups use DNS-over-HTTPS (Google, with Cloudflare fallback) over the
//! shared HTTP client so no native DNS resolver dependency is needed.

use std::time::Duration;

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::cache::MxCache;
use crate::receipt::{ReceiptKind, ReceiptLedger, Verdict};
use crate::registry::{Tool, ToolContext};

/// Overall timeout for one SMTP probe (connect + banner + dialogue).
const SMTP_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// Per-step read timeout while talking to the SMTP server.
const SMTP_READ_TIMEOUT: Duration = Duration::from_secs(5);
/// Sender address used for the SMTP probe (never delivered — we bail before DATA).
const PROBE_FROM: &str = "verify@example.com";

// ─── Result types ───

/// Outcome of the optional SMTP connectivity probe against the best MX host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpResult {
    /// TCP connection to port 25 succeeded and a banner was received.
    pub connected: bool,
    /// The greeting banner, e.g. `220 mx.example.com ESMTP`.
    pub banner: Option<String>,
    /// Whether `RCPT TO` was accepted (`Some(true)`), rejected (`Some(false)`),
    /// or could not be determined (`None` — e.g. connection dropped).
    pub accepted: Option<bool>,
    /// Human-readable detail (error or server response).
    pub detail: Option<String>,
}

/// Full result of an email verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailVerification {
    pub email: String,
    pub is_valid_syntax: bool,
    pub domain_exists: bool,
    pub mx_records: Vec<String>,
    pub smtp_check: Option<SmtpResult>,
    pub is_disposable: bool,
    /// Role-based local part (info@, support@, admin@, …).
    pub is_role_based: bool,
    /// Deliverability confidence in `[0.0, 1.0]`.
    pub confidence: f32,
}

// ─── Verifier ───

pub struct EmailVerifier;

impl EmailVerifier {
    /// Verify an email address.
    ///
    /// 1. Syntax validation (practical RFC 5322 subset)
    /// 2. Domain DNS lookup (MX records, falling back to A records)
    /// 3. Disposable-domain check
    /// 4. Role-based local-part check
    /// 5. Optional SMTP connection test (`smtp_check = true`; may be slow)
    pub async fn verify(
        &self,
        client: &reqwest::Client,
        email: &str,
        smtp_check: bool,
        mx_cache: Option<&MxCache>,
    ) -> EmailVerification {
        let email = email.trim().to_string();
        let is_valid_syntax = check_syntax(&email);

        let domain = email.rsplit_once('@').map(|(_, d)| d.to_lowercase()).unwrap_or_default();
        let local_part = email.split_once('@').map(|(l, _)| l.to_lowercase()).unwrap_or_default();

        let is_disposable = is_disposable_domain(&domain);
        let is_role_based = is_role_based_local(&local_part);

        // DNS lookups only make sense for syntactically valid addresses.
        let mut mx_records = Vec::new();
        let mut domain_exists = false;
        if is_valid_syntax && !domain.is_empty() {
            mx_records = check_mx(client, &domain, mx_cache).await;
            if mx_records.is_empty() {
                // RFC 5321 §5.1: fall back to A/AAAA records when no MX exists.
                domain_exists = has_a_record(client, &domain).await;
            } else {
                domain_exists = true;
            }
        }

        let smtp = if smtp_check && is_valid_syntax && !mx_records.is_empty() {
            Some(smtp_probe(&mx_records[0], &email).await)
        } else {
            None
        };

        let confidence = compute_confidence(
            is_valid_syntax,
            domain_exists,
            is_disposable,
            smtp.as_ref(),
        );

        EmailVerification {
            email,
            is_valid_syntax,
            domain_exists,
            mx_records,
            smtp_check: smtp,
            is_disposable,
            is_role_based,
            confidence,
        }
    }
}

// ─── Verification receipts ───

/// Write durable typed receipts for an email verification into the ledger, if
/// one is attached to the context. Each check kind is recorded independently so
/// a domain-MX green never hides an SMTP red (and vice-versa).
async fn record_verification_receipts(ctx: &ToolContext, result: &EmailVerification) {
    let Some(ledger) = &ctx.receipt_ledger else {
        return;
    };
    record_email_receipts(ledger, result).await;
}

/// The reusable receipt-recording routine (unit-testable without a ToolContext).
pub async fn record_email_receipts(ledger: &ReceiptLedger, result: &EmailVerification) {
    let src = Some("verify_email".to_string());
    let email = result.email.clone();

    ledger
        .record(
            ReceiptKind::of(ReceiptKind::EMAIL_SYNTAX),
            &email,
            if result.is_valid_syntax { Verdict::Pass } else { Verdict::Fail },
            None,
            src.clone(),
        )
        .await
        .ok();

    if result.is_valid_syntax && !email.is_empty() {
        ledger
            .record(
                ReceiptKind::of(ReceiptKind::EMAIL_DOMAIN_MX),
                email.rsplit_once('@').map(|(_, d)| d).unwrap_or(&email),
                if result.domain_exists { Verdict::Pass } else { Verdict::Fail },
                if result.mx_records.is_empty() {
                    None
                } else {
                    Some(result.mx_records.join(", "))
                },
                src.clone(),
            )
            .await
            .ok();
    }

    if let Some(smtp) = &result.smtp_check {
        let verdict = match smtp.accepted {
            Some(true) => Verdict::Pass,
            Some(false) => Verdict::Fail,
            None => Verdict::Inconclusive,
        };
        ledger
            .record(
                ReceiptKind::of(ReceiptKind::EMAIL_SMTP),
                &email,
                verdict,
                smtp.detail.clone(),
                src.clone(),
            )
            .await
            .ok();
    }

    // Disposable is a "this fact is bad" signal — record as pass when the
    // address is ordinary, fail when it is disposable.
    ledger
        .record(
            ReceiptKind::of(ReceiptKind::EMAIL_DISPOSABLE),
            &email,
            if result.is_disposable { Verdict::Fail } else { Verdict::Pass },
            None,
            src.clone(),
        )
        .await
        .ok();
}

// ─── Syntax validation ───

/// Validate an email address against a practical RFC 5322 subset:
/// one `@`, non-empty local part (≤ 64 chars) of atext characters without
/// leading/trailing/consecutive dots, and a dotted domain (≤ 253 chars) of
/// alphanumeric/hyphen labels with a non-numeric TLD.
pub fn check_syntax(email: &str) -> bool {
    let email = email.trim();
    if email.is_empty() || email.len() > 254 {
        return false;
    }
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    // Exactly one `@`.
    if local.contains('@') {
        return false;
    }
    valid_local_part(local) && valid_domain(domain)
}

fn valid_local_part(local: &str) -> bool {
    if local.is_empty() || local.len() > 64 {
        return false;
    }
    if local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return false;
    }
    local.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '!' | '#' | '$' | '%' | '&' | '\'' | '*'
            | '+' | '-' | '/' | '=' | '?' | '^' | '_' | '`' | '{' | '|' | '}' | '~')
    })
}

fn valid_domain(domain: &str) -> bool {
    if domain.len() < 4 || domain.len() > 253 {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    for label in &labels {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }
    // TLD must not be all-numeric (rejects raw IPv4 addresses).
    !labels.last().unwrap().chars().all(|c| c.is_ascii_digit())
}

// ─── Disposable / role-based checks ───

/// Known disposable / temporary email providers.
const DISPOSABLE_DOMAINS: &[&str] = &[
    "0-mail.com", "10minutemail.com", "20minutemail.com", "anonymbox.com",
    "bccto.me", "burnermail.io", "deadaddress.com", "discard.email",
    "dispostable.com", "emailigo.com", "emailondeck.com", "fakeinbox.com",
    "getnada.com", "grr.la", "guerrillamail.com", "guerrillamail.net",
    "guerrillamail.org", "guerrillamailblock.com", "harakirimail.com",
    "inboxkitten.com", "junkmail.com", "mailcatch.com", "maildrop.cc",
    "mailinator.com", "mailnesia.com", "mailnull.com", "mailtemp.info",
    "mail-temp.com", "mailzilla.com", "mintemail.com", "moakt.com",
    "mohmal.com", "mytemp.email", "safetymail.info", "sharklasers.com",
    "spam.la", "spambog.com", "spamfree24.org", "spamgourmet.com",
    "spamobox.com", "tempinbox.com", "tempmail.com", "temp-mail.org",
    "tempr.email", "throwawaymail.com", "trashmail.com", "trashmail.net",
    "trashed.net", "yopmail.com", "disposablemail.com", "mailcatch.com",
    "mailnesia.com", "mailsac.com", "vmani.com",
];

/// Check whether `domain` is a known disposable-email provider (exact match,
/// or subdomain of a provider that hands out arbitrary subdomains).
pub fn is_disposable_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_end_matches('.').to_lowercase();
    if domain.is_empty() {
        return false;
    }
    DISPOSABLE_DOMAINS.iter().any(|d| {
        domain == *d || domain.ends_with(&format!(".{d}"))
    })
}

/// Local parts that belong to a department or function rather than a person.
const ROLE_LOCAL_PARTS: &[&str] = &[
    "admin", "administrator", "abuse", "billing", "careers", "compliance",
    "contact", "contactus", "customerservice", "feedback", "finance",
    "hello", "help", "hr", "info", "it", "jobs", "legal", "marketing",
    "media", "noc", "no-reply", "noreply", "office", "operations",
    "postmaster", "press", "privacy", "reception", "root", "sales",
    "security", "service", "studio", "support", "team", "webmaster",
];

/// Check whether the local part is role-based (info@, support@, admin@, …).
pub fn is_role_based_local(local_part: &str) -> bool {
    let local = local_part.trim().to_lowercase();
    ROLE_LOCAL_PARTS.contains(&local.as_str())
}

// ─── DNS over HTTPS ───

/// Look up MX records for `domain` via DNS-over-HTTPS, deduplicating lookups
/// across the session through `cache` (fleet report B16).
///
/// On a cache hit the stored result is returned without touching the network.
/// Only authoritative answers are ever stored — a non-empty MX list or a
/// definitive `Status == 0` ("no records") response — so a cache hit always
/// reproduces the original lookup output exactly. Transient resolver/network
/// failures are not cached, so later lookups can retry them.
pub async fn check_mx(
    client: &reqwest::Client,
    domain: &str,
    cache: Option<&MxCache>,
) -> Vec<String> {
    if let Some(cache) = cache {
        if let Some(cached) = cache.get(domain) {
            return (*cached).clone();
        }
    }

    let (mx, definitive) = lookup_mx_doh(client, domain).await;
    if definitive {
        if let Some(cache) = cache {
            cache.insert(domain, mx.clone());
        }
    }
    mx
}

/// Raw DNS-over-HTTPS MX lookup (no caching).
///
/// Tries Google's resolver first, then Cloudflare's. Returns MX exchanges
/// sorted by priority (lowest first) plus a `definitive` flag: `true` when a
/// resolver produced a trustworthy answer (records found, or an authoritative
/// "no records" response), `false` when no resolver produced a usable answer
/// (network/parse failures — the returned list is then empty).
async fn lookup_mx_doh(client: &reqwest::Client, domain: &str) -> (Vec<String>, bool) {
    for (base, header) in [
        ("https://dns.google/resolve", None),
        ("https://cloudflare-dns.com/dns-query", Some("application/dns-json")),
    ] {
        let mut req = client
            .get(format!("{base}?name={}&type=MX", urlencoding::encode(domain)))
            .header("User-Agent", "ParallelResearch/0.1")
            .timeout(Duration::from_secs(5));
        if let Some(accept) = header {
            req = req.header("accept", accept);
        }
        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(value) = resp.json::<serde_json::Value>().await {
                    let mx = parse_doh_mx(&value);
                    // Both a definitive answer (records found) and an authoritative
                    // "no records" response mean this resolver worked.
                    if !mx.is_empty() || value.get("Status").and_then(|s| s.as_u64()) == Some(0) {
                        return (mx, true);
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!("DoH MX lookup for {domain} failed: HTTP {}", resp.status());
            }
            Err(e) => {
                tracing::warn!("DoH MX lookup for {domain} error: {e}");
            }
        }
    }
    (Vec::new(), false)
}

/// Check whether `domain` has an A record (fallback when no MX exists).
pub async fn has_a_record(client: &reqwest::Client, domain: &str) -> bool {
    let req = client
        .get(format!(
            "https://dns.google/resolve?name={}&type=A",
            urlencoding::encode(domain)
        ))
        .header("User-Agent", "ParallelResearch/0.1")
        .timeout(Duration::from_secs(5));
    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(value) => parse_doh_a(&value),
                Err(_) => false,
            }
        }
        _ => {
            // Cloudflare fallback.
            let fallback = client
                .get(format!(
                    "https://cloudflare-dns.com/dns-query?name={}&type=A",
                    urlencoding::encode(domain)
                ))
                .header("accept", "application/dns-json")
                .header("User-Agent", "ParallelResearch/0.1")
                .timeout(Duration::from_secs(5))
                .send()
                .await;
            match fallback {
                Ok(resp) if resp.status().is_success() => resp
                    .json::<serde_json::Value>()
                    .await
                    .map(|v| parse_doh_a(&v))
                    .unwrap_or(false),
                _ => false,
            }
        }
    }
}

/// Parse a DNS-over-HTTPS JSON response into sorted MX exchanges.
///
/// Expects the common `dns-json` shape: `{ "Answer": [{ "type": 15,
/// "data": "10 mail.example.com." }, …] }`.
pub fn parse_doh_mx(value: &serde_json::Value) -> Vec<String> {
    let mut records: Vec<(u64, String)> = value
        .get("Answer")
        .and_then(|a| a.as_array())
        .map(|answers| {
            answers
                .iter()
                .filter(|a| a.get("type").and_then(|t| t.as_u64()) == Some(15))
                .filter_map(|a| {
                    let data = a.get("data")?.as_str()?;
                    let mut parts = data.splitn(2, char::is_whitespace);
                    let priority = parts.next()?.parse::<u64>().ok()?;
                    let exchange = parts.next()?.trim_end_matches('.').to_lowercase();
                    if exchange.is_empty() || exchange == "null" {
                        // RFC 7505 null MX — the domain explicitly accepts no mail.
                        return None;
                    }
                    Some((priority, exchange))
                })
                .collect()
        })
        .unwrap_or_default();

    records.sort_by_key(|(priority, _)| *priority);
    records.into_iter().map(|(_, exchange)| exchange).collect()
}

/// Parse a DNS-over-HTTPS JSON response and report whether any A record is present.
pub fn parse_doh_a(value: &serde_json::Value) -> bool {
    value
        .get("Answer")
        .and_then(|a| a.as_array())
        .map(|answers| {
            answers
                .iter()
                .any(|a| a.get("type").and_then(|t| t.as_u64()) == Some(1))
        })
        .unwrap_or(false)
}

// ─── SMTP probe ───

/// Probe the first MX host: connect on port 25, read the banner, run
/// `HELO` / `MAIL FROM` / `RCPT TO` and report whether the recipient was
/// accepted. Never sends message content (no `DATA`).
pub async fn smtp_probe(mx_host: &str, recipient: &str) -> SmtpResult {
    match tokio::time::timeout(SMTP_PROBE_TIMEOUT, smtp_dialogue(mx_host, recipient)).await {
        Ok(result) => result,
        Err(_) => SmtpResult {
            connected: false,
            banner: None,
            accepted: None,
            detail: Some(format!("SMTP probe timed out after {}s", SMTP_PROBE_TIMEOUT.as_secs())),
        },
    }
}

async fn smtp_dialogue(mx_host: &str, recipient: &str) -> SmtpResult {
    let addr = format!("{mx_host}:25");
    let stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            return SmtpResult {
                connected: false,
                banner: None,
                accepted: None,
                detail: Some(format!("connect to {addr} failed: {e}")),
            };
        }
    };

    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // 1. Read the greeting banner (220 …).
    let banner = match read_reply(&mut reader).await {
        Some(line) => line,
        None => {
            return SmtpResult {
                connected: true,
                banner: None,
                accepted: None,
                detail: Some("connected but no SMTP banner received".to_string()),
            };
        }
    };
    if !banner.starts_with("220") {
        return SmtpResult {
            connected: true,
            banner: Some(banner.clone()),
            accepted: None,
            detail: Some(format!("unexpected banner: {banner}")),
        };
    }

    // 2. HELO.
    if !send_and_expect(&mut reader, &mut write_half, "HELO verify.local\r\n", "250").await {
        return SmtpResult {
            connected: true,
            banner: Some(banner),
            accepted: None,
            detail: Some("HELO rejected".to_string()),
        };
    }

    // 3. MAIL FROM.
    if !send_and_expect(&mut reader, &mut write_half, &format!("MAIL FROM:<{PROBE_FROM}>\r\n"), "250").await {
        return SmtpResult {
            connected: true,
            banner: Some(banner),
            accepted: None,
            detail: Some("MAIL FROM rejected".to_string()),
        };
    }

    // 4. RCPT TO — the actual deliverability signal.
    let _ = write_half.write_all(format!("RCPT TO:<{recipient}>\r\n").as_bytes()).await;
    let rcpt = read_reply(&mut reader).await.unwrap_or_default();
    let accepted = if rcpt.starts_with("250") || rcpt.starts_with("251") {
        Some(true)
    } else if rcpt.starts_with('5') {
        Some(false)
    } else {
        None // 4xx or garbage: greylisting / rate limiting — inconclusive.
    };

    // 5. Clean up: RSET + QUIT (best effort).
    let _ = write_half.write_all(b"RSET\r\n").await;
    let _ = read_reply(&mut reader).await;
    let _ = write_half.write_all(b"QUIT\r\n").await;

    SmtpResult {
        connected: true,
        banner: Some(banner),
        accepted,
        detail: if rcpt.is_empty() { None } else { Some(rcpt) },
    }
}

/// Send one SMTP command and check whether the reply starts with `expect`.
async fn send_and_expect(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    command: &str,
    expect: &str,
) -> bool {
    if writer.write_all(command.as_bytes()).await.is_err() {
        return false;
    }
    match read_reply(reader).await {
        Some(line) => line.starts_with(expect),
        None => false,
    }
}

/// Read one complete SMTP reply (multi-line replies end with `NNN ` — a code
/// followed by a space). Returns the last line, e.g. `250 OK`.
async fn read_reply(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Option<String> {
    let mut last = None;
    loop {
        let mut line = String::new();
        let read = tokio::time::timeout(SMTP_READ_TIMEOUT, reader.read_line(&mut line)).await;
        match read {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return last,
            Ok(Ok(_)) => {
                let trimmed = line.trim_end().to_string();
                let is_final = trimmed.len() >= 4
                    && trimmed.as_bytes()[3] == b' '
                    && trimmed[..3].chars().all(|c| c.is_ascii_digit());
                last = Some(trimmed);
                if is_final {
                    return last;
                }
            }
        }
    }
}

// ─── Confidence ───

/// Combine the individual signals into a deliverability confidence score.
pub fn compute_confidence(
    is_valid_syntax: bool,
    domain_exists: bool,
    is_disposable: bool,
    smtp: Option<&SmtpResult>,
) -> f32 {
    if !is_valid_syntax {
        return 0.0;
    }
    let mut c = 0.5_f32;
    if domain_exists {
        c += 0.4;
    }
    match smtp.and_then(|s| s.accepted) {
        Some(true) => c = c.max(0.95),
        Some(false) => c = c.min(0.2),
        // No SMTP probe or inconclusive: without a mailbox check we cannot
        // claim more than 0.9 confidence.
        None => c = c.min(0.9),
    }
    if is_disposable {
        c -= 0.3;
    }
    c.clamp(0.0, 1.0)
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct VerifyEmailParams {
    /// Email address to verify
    email: String,
    /// Run an SMTP connectivity probe against the best MX host (slower, default: false)
    #[serde(default)]
    smtp_check: bool,
}

#[async_trait]
impl Tool for EmailVerifier {
    fn name(&self) -> &str {
        "verify_email"
    }
    fn description(&self) -> &str {
        "Verify an email address: syntax validation, domain MX lookup, disposable and role-based detection, and an optional SMTP check. Returns a deliverability confidence score.

## Capability

Checks the address in stages: RFC 5322 syntax, domain existence via MX (falling back to A) records, known disposable-provider detection (mailinator.com, yopmail.com, …), and role-based local-part detection (info@, support@, admin@, …). With `smtp_check: true` it also connects to the best MX host on port 25 and tests whether the mailbox is accepted (RCPT TO) without sending any mail.

## When to Use

- Validating contact emails found during lead generation or OSINT research.
- Filtering out disposable or department-level addresses from lead lists.
- Scoring how trustworthy a contact email is before outreach.

## When NOT to Use

- Do NOT use for mass verification of large lists — each lookup performs live DNS queries.
- Do NOT enable `smtp_check` by default: port 25 is often blocked or rate-limited, and probing can be slow.

## Output

A per-signal report plus a confidence score: 0.0 = invalid syntax, ~0.9 = valid syntax + live domain, higher/lower when the SMTP probe confirms or rejects the mailbox. Disposable domains are penalized.

## Failure Modes

- DNS-over-HTTPS unreachable → domain reported as not existing; retry later.
- SMTP inconclusive (greylisting, 4xx) → `smtp_check.accepted` is null; rely on the other signals."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(VerifyEmailParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: VerifyEmailParams = serde_json::from_value(args)?;
        let result = self
            .verify(&ctx.http_client, &params.email, params.smtp_check, Some(&ctx.mx_cache))
            .await;

        let verdict = if !result.is_valid_syntax {
            "INVALID (syntax)"
        } else if !result.domain_exists {
            "UNDELIVERABLE (domain has no MX/A records)"
        } else if result.smtp_check.as_ref().and_then(|s| s.accepted) == Some(false) {
            "UNDELIVERABLE (mailbox rejected)"
        } else if result.is_disposable {
            "DISPOSABLE"
        } else if result.smtp_check.as_ref().and_then(|s| s.accepted) == Some(true) {
            "VALID (mailbox accepted)"
        } else {
            "LIKELY VALID"
        };

        let mut out = format!("Email verification: {}\n", result.email);
        out.push_str(&format!("Verdict: {verdict}\n"));
        out.push_str(&format!("Syntax: {}\n", if result.is_valid_syntax { "valid" } else { "INVALID" }));
        out.push_str(&format!(
            "Domain: {}\n",
            if result.domain_exists { "exists" } else { "NOT FOUND" }
        ));
        if !result.mx_records.is_empty() {
            out.push_str(&format!("MX records: {}\n", result.mx_records.join(", ")));
        }
        out.push_str(&format!("Disposable: {}\n", if result.is_disposable { "YES" } else { "no" }));
        out.push_str(&format!(
            "Role-based: {}\n",
            if result.is_role_based { "YES (department address, not a person)" } else { "no" }
        ));
        if let Some(ref smtp) = result.smtp_check {
            let accepted = match smtp.accepted {
                Some(true) => "accepted",
                Some(false) => "REJECTED",
                None => "inconclusive",
            };
            out.push_str(&format!(
                "SMTP: connected={} mailbox={}\n",
                smtp.connected, accepted
            ));
            if let Some(ref d) = smtp.detail {
                out.push_str(&format!("SMTP detail: {d}\n"));
            }
        } else {
            out.push_str("SMTP: not checked\n");
        }
        out.push_str(&format!("Confidence: {:.2}\n", result.confidence));

        let meta = serde_json::to_value(&result).unwrap_or_default();
        record_verification_receipts(ctx, &result).await;
        Ok(ToolOutput::ok_with_meta(out, meta))
    }
}

// ─── Email pattern suggestion (permutations) ───

/// ASCII-sanitize a name token for use in an email local part.
fn sanitize_local(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect()
}

/// Standard corporate email patterns for a person, most common first.
/// Returns `(pattern_id, local_part)` pairs.
pub fn permutation_patterns(first: &str, last: &str) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    if first.is_empty() && last.is_empty() {
        return out;
    }
    if first.is_empty() || last.is_empty() {
        let only = if first.is_empty() { last } else { first };
        out.push(("single", only.to_string()));
        return out;
    }
    let f_initial: String = first.chars().take(1).collect();
    let l_initial: String = last.chars().take(1).collect();
    out.push(("first.last", format!("{first}.{last}")));
    out.push(("firstlast", format!("{first}{last}")));
    out.push(("f.last", format!("{f_initial}.{last}")));
    out.push(("first.l", format!("{first}.{l_initial}")));
    out.push(("last.first", format!("{last}.{first}")));
    out.push(("first_local", format!("{first}_{last}")));
    out.push(("f_local", format!("{f_initial}_{last}")));
    out.push(("flast", format!("{f_initial}{last}")));
    out.push(("first-last", format!("{first}-{last}")));
    out
}

/// Generate candidate email addresses for a person at a domain.
/// Accepts "First Last" (first token = first name, last token = last name,
/// middle names ignored). Duplicate and syntactically invalid permutations
/// are dropped.
pub fn generate_email_permutations(name: &str, domain: &str) -> Vec<(String, String)> {
    let domain = domain.trim().to_lowercase();
    if domain.is_empty() {
        return Vec::new();
    }
    let tokens: Vec<String> = name
        .split_whitespace()
        .map(sanitize_local)
        .filter(|t| !t.is_empty())
        .collect();
    let (first, last) = match tokens.len() {
        0 => return Vec::new(),
        1 => (tokens[0].clone(), String::new()),
        _ => (tokens[0].clone(), tokens[tokens.len() - 1].clone()),
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (id, local) in permutation_patterns(&first, &last) {
        let email = format!("{local}@{domain}");
        if seen.insert(email.clone()) && check_syntax(&email) {
            out.push((id.to_string(), email));
        }
    }
    out
}

/// Structural email pattern detected on a domain from known addresses of
/// OTHER people (e.g. colleagues found on the company site).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainEmailPattern {
    /// Separator between name parts: `.`, `_`, `-`, or `\0` when none.
    pub separator: char,
    /// First name part reduced to an initial (`j.smith` vs `john.smith`).
    pub initial_first: bool,
}

/// Infer the corporate email pattern from known addresses on the same
/// domain. Role addresses (info@, sales@, …) are ignored.
pub fn detect_domain_pattern(
    known_emails: &[String],
    domain: &str,
) -> Option<DomainEmailPattern> {
    let domain = domain.trim().to_lowercase();
    // Counts for separators: '.', '_', '-', none.
    let mut sep_counts = [0usize; 4];
    let mut initial_votes = 0usize;
    let mut total = 0usize;
    for email in known_emails {
        let email = email.trim().to_lowercase();
        let Some((local, dom)) = email.split_once('@') else {
            continue;
        };
        if dom != domain || local.is_empty() || is_role_based_local(local) {
            continue;
        }
        let (sep_idx, parts) = if local.contains('.') {
            (0, local.split('.').collect::<Vec<_>>())
        } else if local.contains('_') {
            (1, local.split('_').collect::<Vec<_>>())
        } else if local.contains('-') {
            (2, local.split('-').collect::<Vec<_>>())
        } else {
            (3, vec![local])
        };
        if parts.iter().any(|p| p.is_empty()) {
            continue;
        }
        sep_counts[sep_idx] += 1;
        total += 1;
        if parts.len() >= 2 && parts[0].len() == 1 {
            initial_votes += 1;
        }
    }
    if total == 0 {
        return None;
    }
    let (best_idx, _) = sep_counts.iter().enumerate().max_by_key(|(_, c)| *c)?;
    let separator = match best_idx {
        0 => '.',
        1 => '_',
        2 => '-',
        _ => '\0',
    };
    Some(DomainEmailPattern {
        separator,
        initial_first: initial_votes * 2 > total,
    })
}

/// The generated-pattern id that best matches a detected domain pattern.
fn pattern_match_id(p: DomainEmailPattern) -> Option<&'static str> {
    match (p.separator, p.initial_first) {
        ('.', false) => Some("first.last"),
        ('.', true) => Some("f.last"),
        ('\0', false) => Some("firstlast"),
        ('\0', true) => Some("flast"),
        ('_', false) => Some("first_local"),
        ('_', true) => Some("f_local"),
        ('-', false) => Some("first-last"),
        _ => None,
    }
}

fn describe_pattern(p: DomainEmailPattern) -> &'static str {
    match (p.separator, p.initial_first) {
        ('.', false) => "first.last",
        ('.', true) => "f.last (initial + dot)",
        ('\0', false) => "firstlast (no separator)",
        ('\0', true) => "flast (initial, no separator)",
        ('_', false) => "first_last",
        ('_', true) => "f_last (initial + underscore)",
        ('-', false) => "first-last",
        ('-', true) => "initial + hyphen (rare)",
        _ => "unknown",
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SuggestEmailsParams {
    /// Person name, e.g. "Ivan Petrov" (first token = first name, last
    /// token = last name). Use the Latin spelling you intend to use in the
    /// address.
    pub name: String,
    /// Corporate domain, e.g. "example.com".
    pub domain: String,
    /// Known addresses of OTHER people at the same domain (e.g. colleagues
    /// from the company site) — used to infer the corporate email pattern
    /// and rank candidates.
    #[serde(default)]
    pub known_emails: Vec<String>,
    /// Probe each mailbox over SMTP (slow, often rate-limited). Default
    /// false — candidates are scored by syntax + domain MX only.
    #[serde(default)]
    pub smtp_check: bool,
}

pub struct EmailSuggester;

#[async_trait]
impl Tool for EmailSuggester {
    fn name(&self) -> &str {
        "suggest_emails"
    }
    fn description(&self) -> &str {
        "Generate likely corporate email addresses for a person from standard name permutations at a domain, infer the company's email pattern from known colleague addresses, and verify every candidate (syntax + MX, optional SMTP).

## Capability

Builds up to 9 permutations (first.last, firstlast, f.last, …) for the given name at the given domain. If `known_emails` of other people at the same domain are provided, detects the corporate pattern (separator, initial vs full first name) and boosts the matching candidate. All candidates pass the same verification pipeline as verify_email; MX is looked up once per domain (cached).

## When to Use

- You found a person's name and the company domain but no personal email.
- Ranking guessed addresses before outreach to avoid bounces.

## When NOT to Use

- Do NOT use for domains with catch-all MX accepting every mailbox — SMTP probing cannot distinguish real boxes there; report such candidates as low-confidence.
- Do NOT enable `smtp_check` for more than a handful of candidates at once.

## Output

Ranked candidates with per-candidate confidence (0–1) and a pattern-match flag. Confidence reflects deliverability of the pattern, not certainty that the person uses it."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(SuggestEmailsParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: SuggestEmailsParams = serde_json::from_value(args)?;
        let candidates = generate_email_permutations(&params.name, &params.domain);
        if candidates.is_empty() {
            return Ok(ToolOutput::err_code(
                "could not build candidates: empty name or domain",
                "invalid_input",
            ));
        }

        let pattern = detect_domain_pattern(&params.known_emails, &params.domain);
        let boost_id = pattern.and_then(pattern_match_id);

        let verifier = EmailVerifier;
        let mut rows: Vec<serde_json::Value> = Vec::new();
        let mut domain_ok: Option<bool> = None;
        let mut mx_summary: Vec<String> = Vec::new();

        for (id, email) in candidates.iter().take(12) {
            let v = verifier
                .verify(
                    &ctx.http_client,
                    email,
                    params.smtp_check,
                    Some(&ctx.mx_cache),
                )
                .await;
            if domain_ok.is_none() {
                domain_ok = Some(v.domain_exists);
                mx_summary = v.mx_records.clone();
            }
            let pattern_match = boost_id == Some(id.as_str());
            let mut confidence = v.confidence;
            if pattern_match {
                confidence = (confidence + 0.15).min(0.95);
            }
            rows.push(serde_json::json!({
                "email": v.email,
                "pattern": id,
                "pattern_match": pattern_match,
                "confidence": (confidence * 100.0).round() / 100.0,
                "domain_exists": v.domain_exists,
                "smtp_accepted": v.smtp_check.as_ref().and_then(|s| s.accepted),
            }));
        }

        // Rank: pattern match first, then confidence desc.
        rows.sort_by(|a, b| {
            let am = a["pattern_match"].as_bool().unwrap_or(false);
            let bm = b["pattern_match"].as_bool().unwrap_or(false);
            bm.cmp(&am).then_with(|| {
                let ac = a["confidence"].as_f64().unwrap_or(0.0);
                let bc = b["confidence"].as_f64().unwrap_or(0.0);
                bc.partial_cmp(&ac).unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        if domain_ok == Some(false) {
            return Ok(ToolOutput::err_code(
                format!(
                    "domain '{}' has no MX/A records — no candidate can be deliverable",
                    params.domain
                ),
                "domain_not_found",
            ));
        }

        let mut out = format!(
            "Email candidates for {} @ {} ({} generated)\n",
            params.name,
            params.domain,
            rows.len()
        );
        if let Some(p) = pattern {
            out.push_str(&format!(
                "Domain pattern detected from {} known address(es): {}\n",
                params.known_emails.len(),
                describe_pattern(p)
            ));
        } else if !params.known_emails.is_empty() {
            out.push_str("Domain pattern: not detected from known addresses\n");
        }
        if !mx_summary.is_empty() {
            out.push_str(&format!("MX: {}\n", mx_summary.join(", ")));
        }
        out.push_str("\nRank  Email                                  Confidence  Notes\n");
        for (i, row) in rows.iter().enumerate() {
            let email = row["email"].as_str().unwrap_or_default();
            let conf = row["confidence"].as_f64().unwrap_or(0.0);
            let mut notes = row["pattern"].as_str().unwrap_or_default().to_string();
            if row["pattern_match"].as_bool().unwrap_or(false) {
                notes.push_str(" ← pattern match");
            }
            if let Some(accepted) = row["smtp_accepted"].as_bool() {
                notes.push_str(if accepted {
                    ", SMTP accepted"
                } else {
                    ", SMTP REJECTED"
                });
            }
            out.push_str(&format!("{:<6}{email:<40}{conf:<12.2}{notes}\n", i + 1));
        }
        out.push_str(
            "\nConfidence scores deliverability of the address format; save only top-ranked candidates and prefer verifying again before outreach.",
        );

        let meta = serde_json::json!({
            "name": params.name,
            "domain": params.domain,
            "pattern_detected": pattern.map(describe_pattern),
            "candidates": rows,
        });
        Ok(ToolOutput::ok_with_meta(out, meta))
    }
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    // ── Syntax ──

    #[test]
    fn test_syntax_valid() {
        for email in [
            "user@example.com",
            "first.last@example.co.uk",
            "user+tag@sub.domain.example.com",
            "a_b-c@d-e.com",
            "USER@EXAMPLE.COM",
        ] {
            assert!(check_syntax(email), "{email} should be valid");
        }
    }

    #[test]
    fn test_syntax_invalid() {
        for email in [
            "",
            "plainaddress",
            "@no-local.com",
            "user@",
            "user@@example.com",
            "user@com",                 // no dotted domain
            "user@.example.com",       // empty label
            "user@example..com",       // empty label
            "user@-example.com",       // leading hyphen
            "user@example.com-",       // trailing hyphen
            ".user@example.com",       // leading dot in local
            "user.@example.com",       // trailing dot in local
            "us..er@example.com",      // consecutive dots
            "user name@example.com",   // space in local
            "user@example.123",        // numeric TLD
            "user@exam ple.com",       // space in domain
        ] {
            assert!(!check_syntax(email), "{email} should be invalid");
        }
    }

    #[test]
    fn test_syntax_length_limits() {
        let long_local = "a".repeat(65);
        assert!(!check_syntax(&format!("{long_local}@example.com")));
        let ok_local = "a".repeat(64);
        assert!(check_syntax(&format!("{ok_local}@example.com")));
    }

    // ── Disposable ──

    #[test]
    fn test_disposable_domains() {
        assert!(is_disposable_domain("mailinator.com"));
        assert!(is_disposable_domain("YOPMAIL.COM"));
        assert!(is_disposable_domain("anything.guerrillamail.com")); // subdomain
        assert!(!is_disposable_domain("gmail.com"));
        assert!(!is_disposable_domain("example.com"));
        assert!(!is_disposable_domain(""));
        // A domain that merely contains a disposable name is not disposable.
        assert!(!is_disposable_domain("notmailinator.com"));
    }

    // ── Role-based ──

    #[test]
    fn test_role_based_local_parts() {
        for local in ["info", "support", "admin", "sales", "no-reply", "HR"] {
            assert!(is_role_based_local(local), "{local} should be role-based");
        }
        for local in ["john.doe", "jane", "user123"] {
            assert!(!is_role_based_local(local), "{local} should not be role-based");
        }
    }

    // ── DoH parsing ──

    #[test]
    fn test_parse_doh_mx_sorts_by_priority_and_strips_dots() {
        let value = serde_json::json!({
            "Status": 0,
            "Answer": [
                {"name": "example.com.", "type": 15, "data": "20 backup.example.com."},
                {"name": "example.com.", "type": 15, "data": "10 mail.example.com."},
                {"name": "example.com.", "type": 1, "data": "93.184.216.34"}
            ]
        });
        assert_eq!(
            parse_doh_mx(&value),
            vec!["mail.example.com", "backup.example.com"]
        );
    }

    #[test]
    fn test_parse_doh_mx_empty_and_null() {
        assert!(parse_doh_mx(&serde_json::json!({"Status": 3})).is_empty());
        assert!(parse_doh_mx(&serde_json::json!({})).is_empty());
        // RFC 7505 null MX must be dropped.
        let value = serde_json::json!({
            "Answer": [{"type": 15, "data": "0 ."}]
        });
        assert!(parse_doh_mx(&value).is_empty());
    }

    #[test]
    fn test_parse_doh_a() {
        let with_a = serde_json::json!({"Answer": [{"type": 1, "data": "93.184.216.34"}]});
        assert!(parse_doh_a(&with_a));
        let mx_only = serde_json::json!({"Answer": [{"type": 15, "data": "10 mail.x.com."}]});
        assert!(!parse_doh_a(&mx_only));
        assert!(!parse_doh_a(&serde_json::json!({})));
    }

    // ── Confidence ──

    #[test]
    fn test_confidence_invalid_syntax_is_zero() {
        assert_eq!(compute_confidence(false, true, false, None), 0.0);
    }

    #[test]
    fn test_confidence_valid_domain_capped_without_smtp() {
        let c = compute_confidence(true, true, false, None);
        assert!((c - 0.9).abs() < f32::EPSILON, "expected 0.9, got {c}");
    }

    #[test]
    fn test_confidence_smtp_accepted_raises_score() {
        let smtp = SmtpResult { connected: true, banner: None, accepted: Some(true), detail: None };
        let c = compute_confidence(true, true, false, Some(&smtp));
        assert!(c >= 0.95);
    }

    #[test]
    fn test_confidence_smtp_rejected_lowers_score() {
        let smtp = SmtpResult { connected: true, banner: None, accepted: Some(false), detail: None };
        let c = compute_confidence(true, true, false, Some(&smtp));
        assert!(c <= 0.2);
    }

    #[test]
    fn test_confidence_disposable_penalty() {
        let normal = compute_confidence(true, true, false, None);
        let disposable = compute_confidence(true, true, true, None);
        assert!(disposable < normal);
    }

    #[test]
    fn test_confidence_no_domain_low() {
        let c = compute_confidence(true, false, false, None);
        assert!(c <= 0.5);
    }

    // ── Tool plumbing ──

    #[test]
    fn test_tool_metadata() {
        let tool = EmailVerifier;
        assert_eq!(tool.name(), "verify_email");
        let schema = tool.schema();
        assert_eq!(schema.name, "verify_email");
        assert!(schema.parameters.get("properties").is_some());
    }

    #[test]
    fn test_suggest_emails_tool_metadata() {
        let tool = EmailSuggester;
        assert_eq!(tool.name(), "suggest_emails");
        let schema = tool.schema();
        assert_eq!(schema.name, "suggest_emails");
        let props = schema.parameters.get("properties").unwrap();
        assert!(props.get("name").is_some());
        assert!(props.get("domain").is_some());
        assert!(props.get("known_emails").is_some());
    }

    // ── Email permutations ──

    #[test]
    fn test_generate_email_permutations_two_token_name() {
        let out = generate_email_permutations("Ivan Petrov", "Example.COM");
        let emails: Vec<String> = out.iter().map(|(_, e)| e.clone()).collect();
        assert!(emails.contains(&"ivan.petrov@example.com".to_string()));
        assert!(emails.contains(&"ivanpetrov@example.com".to_string()));
        assert!(emails.contains(&"i.petrov@example.com".to_string()));
        assert!(emails.contains(&"petrov.ivan@example.com".to_string()));
        assert!(emails.iter().all(|e| e.ends_with("@example.com")));
        // No duplicates.
        let unique: std::collections::HashSet<_> = emails.iter().collect();
        assert_eq!(unique.len(), emails.len());
    }

    #[test]
    fn test_generate_email_permutations_middle_name_uses_last_token() {
        let out = generate_email_permutations("Ivan Sergeevich Petrov", "x.ru");
        let emails: Vec<String> = out.iter().map(|(_, e)| e.clone()).collect();
        assert!(emails.contains(&"ivan.petrov@x.ru".to_string()));
        assert!(!emails.iter().any(|e| e.contains("sergeevich")));
    }

    #[test]
    fn test_generate_email_permutations_single_token() {
        let out = generate_email_permutations("Madonna", "x.com");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, "madonna@x.com");
    }

    #[test]
    fn test_generate_email_permutations_cyrillic_dropped() {
        // Non-ASCII chars are sanitized away; empty tokens yield nothing.
        let out = generate_email_permutations("Иван Петров", "x.ru");
        assert!(out.is_empty());
    }

    #[test]
    fn test_generate_email_permutations_empty_inputs() {
        assert!(generate_email_permutations("", "x.com").is_empty());
        assert!(generate_email_permutations("Ivan", "").is_empty());
    }

    #[test]
    fn test_detect_domain_pattern_dot_full_names() {
        let known = vec![
            "john.smith@corp.com".to_string(),
            "anna.jones@corp.com".to_string(),
            "info@corp.com".to_string(), // role address ignored
        ];
        let p = detect_domain_pattern(&known, "corp.com").unwrap();
        assert_eq!(p.separator, '.');
        assert!(!p.initial_first);
        assert_eq!(pattern_match_id(p), Some("first.last"));
    }

    #[test]
    fn test_detect_domain_pattern_initial_dot() {
        let known = vec![
            "j.smith@corp.com".to_string(),
            "a.jones@corp.com".to_string(),
        ];
        let p = detect_domain_pattern(&known, "corp.com").unwrap();
        assert_eq!(p.separator, '.');
        assert!(p.initial_first);
        assert_eq!(pattern_match_id(p), Some("f.last"));
    }

    #[test]
    fn test_detect_domain_pattern_no_separator() {
        let known = vec!["johnsmith@corp.com".to_string(), "annajones@corp.com".to_string()];
        let p = detect_domain_pattern(&known, "corp.com").unwrap();
        assert_eq!(p.separator, '\0');
        assert!(!p.initial_first);
        assert_eq!(pattern_match_id(p), Some("firstlast"));
    }

    #[test]
    fn test_detect_domain_pattern_other_domain_ignored() {
        let known = vec!["john.smith@other.com".to_string()];
        assert!(detect_domain_pattern(&known, "corp.com").is_none());
    }

    #[test]
    fn test_detect_domain_pattern_empty() {
        assert!(detect_domain_pattern(&[], "corp.com").is_none());
    }
}
