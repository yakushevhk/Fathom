//! CRM synchronisation: push collected contacts into amoCRM, Bitrix24 or
//! HubSpot.
//!
//! `CrmSync::push_contact` creates the contact in the configured CRM and
//! returns the remote contact id. Provider API errors are surfaced as
//! descriptive `anyhow` errors (HTTP status + response excerpt) instead of
//! panicking.

use serde::{Deserialize, Serialize};

use crate::config::CrmConfig;
use crate::contact::Contact;

/// Default HubSpot API endpoint for contact creation.
pub const HUBSPOT_CONTACTS_URL: &str = "https://api.hubapi.com/crm/v3/objects/contacts";

/// A supported CRM backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrmProvider {
    /// amoCRM (API v4). `domain` is the account subdomain (`mycompany` for
    /// `mycompany.amocrm.ru`).
    AmoCrm { domain: String, api_key: String },
    /// Bitrix24 (REST API). `domain` is the account subdomain or full host.
    Bitrix24 { domain: String, api_key: String },
    /// HubSpot (CRM v3 API).
    HubSpot { api_key: String },
}

impl CrmProvider {
    /// Short provider name used in config files and log lines.
    pub fn name(&self) -> &'static str {
        match self {
            Self::AmoCrm { .. } => "amocrm",
            Self::Bitrix24 { .. } => "bitrix24",
            Self::HubSpot { .. } => "hubspot",
        }
    }

    /// Parse a provider from config values. Returns `None` when the provider
    /// name is unknown or required values are missing.
    pub fn parse(provider: &str, domain: &str, api_key: &str) -> Option<CrmProvider> {
        let domain = domain.trim().to_string();
        let api_key = api_key.trim().to_string();
        match provider.trim().to_lowercase().as_str() {
            "amocrm" | "amo" => {
                if domain.is_empty() || api_key.is_empty() {
                    return None;
                }
                Some(CrmProvider::AmoCrm { domain, api_key })
            }
            "bitrix24" | "bitrix" => {
                if domain.is_empty() || api_key.is_empty() {
                    return None;
                }
                Some(CrmProvider::Bitrix24 { domain, api_key })
            }
            "hubspot" => {
                if api_key.is_empty() {
                    return None;
                }
                Some(CrmProvider::HubSpot { api_key })
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for CrmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Pushes contacts into a configured CRM.
#[derive(Debug, Clone)]
pub struct CrmSync {
    provider: CrmProvider,
    http: reqwest::Client,
    /// Test hook: when set, overrides the provider endpoint URL.
    endpoint_override: Option<String>,
}

impl CrmSync {
    pub fn new(provider: CrmProvider) -> Self {
        Self {
            provider,
            // Bounded timeout so a hung CRM endpoint cannot stall the
            // save_contacts tool or a `contacts push-crm` run indefinitely.
            http: crate::http_client(),
            endpoint_override: None,
        }
    }

    /// Build a synchroniser from the `[crm]` config section. Returns `None`
    /// when no (valid) provider is configured.
    pub fn from_config(config: &CrmConfig) -> Option<Self> {
        CrmProvider::parse(&config.provider, &config.domain, &config.api_key).map(Self::new)
    }

    /// Override the provider endpoint (used by tests with a local mock).
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint_override = Some(endpoint.into());
        self
    }

    pub fn provider(&self) -> &CrmProvider {
        &self.provider
    }

    /// Create the contact in the CRM and return the remote contact id.
    pub async fn push_contact(&self, contact: &Contact) -> anyhow::Result<String> {
        match &self.provider {
            CrmProvider::AmoCrm { .. } => self.push_amocrm(contact).await,
            CrmProvider::Bitrix24 { .. } => self.push_bitrix24(contact).await,
            CrmProvider::HubSpot { .. } => self.push_hubspot(contact).await,
        }
    }

    fn endpoint_for(&self, default: String) -> String {
        self.endpoint_override.clone().unwrap_or(default)
    }

    // ── amoCRM ──────────────────────────────────────────────────────────

    async fn push_amocrm(&self, contact: &Contact) -> anyhow::Result<String> {
        let (domain, api_key) = match &self.provider {
            CrmProvider::AmoCrm { domain, api_key } => (domain, api_key),
            _ => unreachable!("push_amocrm called with wrong provider"),
        };
        let url = self.endpoint_for(amocrm_contacts_url(domain));
        let payload = amocrm_payload(contact);

        let response = self
            .http
            .post(&url)
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        if !status.is_success() {
            return Err(api_error("amocrm", status.as_u16(), &body));
        }

        // amoCRM v4 responds with {"_embedded": {"contacts": [{"id": N}]}}.
        let id = body
            .pointer("/_embedded/contacts/0/id")
            .or_else(|| body.get("id"))
            .filter(|v| v.is_number() || v.is_string());
        match id {
            Some(v) => Ok(id_to_string(v)),
            None => Err(anyhow::anyhow!(
                "amocrm API returned HTTP {status} but no contact id in response"
            )),
        }
    }

    // ── Bitrix24 ────────────────────────────────────────────────────────

    async fn push_bitrix24(&self, contact: &Contact) -> anyhow::Result<String> {
        let (domain, api_key) = match &self.provider {
            CrmProvider::Bitrix24 { domain, api_key } => (domain, api_key),
            _ => unreachable!("push_bitrix24 called with wrong provider"),
        };
        let url = self.endpoint_for(bitrix24_contact_add_url(domain));
        let payload = serde_json::json!({ "fields": bitrix24_fields(contact) });

        let response = self
            .http
            .post(&url)
            // Bitrix24 REST authenticates via the `auth` query parameter.
            .query(&[("auth", api_key.as_str())])
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        if !status.is_success() || body.get("error").and_then(|v| v.as_str()).is_some() {
            return Err(api_error("bitrix24", status.as_u16(), &body));
        }

        match body.get("result") {
            Some(v) if v.is_number() || v.is_string() => Ok(id_to_string(v)),
            _ => Err(anyhow::anyhow!(
                "bitrix24 API returned HTTP {status} but no contact id in response"
            )),
        }
    }

    // ── HubSpot ─────────────────────────────────────────────────────────

    async fn push_hubspot(&self, contact: &Contact) -> anyhow::Result<String> {
        let api_key = match &self.provider {
            CrmProvider::HubSpot { api_key } => api_key,
            _ => unreachable!("push_hubspot called with wrong provider"),
        };
        let url = self.endpoint_for(HUBSPOT_CONTACTS_URL.to_string());
        let payload = serde_json::json!({ "properties": hubspot_properties(contact) });

        let response = self
            .http
            .post(&url)
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        if !status.is_success() {
            return Err(api_error("hubspot", status.as_u16(), &body));
        }

        match body.get("id") {
            Some(v) if v.is_number() || v.is_string() => Ok(id_to_string(v)),
            _ => Err(anyhow::anyhow!(
                "hubspot API returned HTTP {status} but no contact id in response"
            )),
        }
    }
}

// ── URL and payload builders (pure, unit-testable) ────────────────────────────

/// amoCRM v4 contact-creation endpoint for an account subdomain.
pub fn amocrm_contacts_url(domain: &str) -> String {
    format!(
        "https://{}.amocrm.ru/api/v4/contacts",
        amocrm_subdomain(domain)
    )
}

fn amocrm_subdomain(domain: &str) -> String {
    let d = strip_scheme(domain.trim());
    let d = d.trim_end_matches('/');
    d.strip_suffix(".amocrm.ru").unwrap_or(d).to_string()
}

/// Bitrix24 REST `crm.contact.add` endpoint. Accepts either a bare subdomain
/// (`mycompany`) or a full host (`mycompany.bitrix24.com`, custom domains).
pub fn bitrix24_contact_add_url(domain: &str) -> String {
    format!("https://{}/rest/crm.contact.add.json", bitrix_host(domain))
}

fn bitrix_host(domain: &str) -> String {
    let d = strip_scheme(domain.trim());
    let d = d.trim_end_matches('/');
    if d.contains('.') {
        d.to_string()
    } else {
        format!("{d}.bitrix24.ru")
    }
}

fn strip_scheme(s: &str) -> &str {
    s.strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s)
}

/// amoCRM v4 payload: an array with a single contact. Email and phone are
/// passed through the built-in custom field codes.
pub fn amocrm_payload(contact: &Contact) -> serde_json::Value {
    let mut custom_fields: Vec<serde_json::Value> = Vec::new();
    if let Some(phone) = contact.phone.as_deref().filter(|s| !s.trim().is_empty()) {
        custom_fields.push(serde_json::json!({
            "field_code": "PHONE",
            "values": [{ "value": phone }],
        }));
    }
    if let Some(email) = contact.email.as_deref().filter(|s| !s.trim().is_empty()) {
        custom_fields.push(serde_json::json!({
            "field_code": "EMAIL",
            "values": [{ "value": email }],
        }));
    }

    let name = contact
        .name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| contact.email.clone())
        .or_else(|| contact.phone.clone())
        .unwrap_or_else(|| "Unknown contact".to_string());

    let mut obj = serde_json::json!({ "name": name });
    if !custom_fields.is_empty() {
        obj["custom_fields_values"] = serde_json::Value::Array(custom_fields);
    }
    serde_json::json!([obj])
}

/// Bitrix24 `crm.contact.add` field map for a contact.
pub fn bitrix24_fields(contact: &Contact) -> serde_json::Value {
    let mut fields = serde_json::Map::new();

    if let Some(name) = contact.name.as_deref().filter(|s| !s.trim().is_empty()) {
        fields.insert("NAME".to_string(), serde_json::json!(name));
    }
    if let Some(title) = contact.title.as_deref().filter(|s| !s.trim().is_empty()) {
        fields.insert("POST".to_string(), serde_json::json!(title));
    }
    if let Some(company) = contact.company.as_deref().filter(|s| !s.trim().is_empty()) {
        fields.insert("COMPANY_TITLE".to_string(), serde_json::json!(company));
    }
    if let Some(email) = contact.email.as_deref().filter(|s| !s.trim().is_empty()) {
        fields.insert(
            "EMAIL".to_string(),
            serde_json::json!([{ "VALUE": email, "VALUE_TYPE": "WORK" }]),
        );
    }
    if let Some(phone) = contact.phone.as_deref().filter(|s| !s.trim().is_empty()) {
        fields.insert(
            "PHONE".to_string(),
            serde_json::json!([{ "VALUE": phone }]),
        );
    }
    if !contact.notes.is_empty() {
        let comments = contact.notes.join("\n");
        fields.insert("COMMENTS".to_string(), serde_json::json!(comments));
    }

    serde_json::Value::Object(fields)
}

/// HubSpot CRM v3 property map for a contact.
pub fn hubspot_properties(contact: &Contact) -> serde_json::Value {
    let mut props = serde_json::Map::new();

    if let Some(email) = contact.email.as_deref().filter(|s| !s.trim().is_empty()) {
        props.insert("email".to_string(), serde_json::json!(email));
    }
    if let Some(phone) = contact.phone.as_deref().filter(|s| !s.trim().is_empty()) {
        props.insert("phone".to_string(), serde_json::json!(phone));
    }
    if let Some(name) = contact.name.as_deref().filter(|s| !s.trim().is_empty()) {
        let (first, last) = split_name(name);
        props.insert("firstname".to_string(), serde_json::json!(first));
        if let Some(last) = last {
            props.insert("lastname".to_string(), serde_json::json!(last));
        }
    }
    if let Some(title) = contact.title.as_deref().filter(|s| !s.trim().is_empty()) {
        props.insert("jobtitle".to_string(), serde_json::json!(title));
    }
    if let Some(company) = contact.company.as_deref().filter(|s| !s.trim().is_empty()) {
        props.insert("company".to_string(), serde_json::json!(company));
    }

    serde_json::Value::Object(props)
}

/// Split a full name into (first name, optional remainder).
fn split_name(name: &str) -> (String, Option<String>) {
    let mut parts = name.split_whitespace();
    let first = parts.next().unwrap_or_default().to_string();
    let rest: Vec<&str> = parts.collect();
    (first, if rest.is_empty() { None } else { Some(rest.join(" ")) })
}

fn id_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Build a descriptive error from a failed CRM API response.
fn api_error(provider: &str, status: u16, body: &serde_json::Value) -> anyhow::Error {
    let detail = extract_api_error_message(body).unwrap_or_else(|| {
        let text = body.to_string();
        if text.len() > 300 {
            let mut end = 300;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &text[..end])
        } else if text == "null" {
            "no response body".to_string()
        } else {
            text
        }
    });
    anyhow::anyhow!("{provider} API error (HTTP {status}): {detail}")
}

/// Extract a human-readable message from a provider error body. amoCRM uses
/// `title`/`detail`, Bitrix24 `error_description`, HubSpot `message`.
fn extract_api_error_message(body: &serde_json::Value) -> Option<String> {
    for key in ["detail", "message", "error_description", "title", "error"] {
        if let Some(s) = body.get(key).and_then(|v| v.as_str()) {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn sample_contact() -> Contact {
        let mut c = Contact::new().with_source("test");
        c.name = Some("Jane Doe".into());
        c.email = Some("jane@example.com".into());
        c.phone = Some("+1 555 0100".into());
        c.title = Some("CTO".into());
        c.company = Some("Acme".into());
        c.notes.push("vip".into());
        c
    }

    // ── URL / payload builders ──────────────────────────────────────────

    #[test]
    fn test_url_builders() {
        assert_eq!(
            amocrm_contacts_url("mycompany"),
            "https://mycompany.amocrm.ru/api/v4/contacts"
        );
        // Accepts a full host or scheme too.
        assert_eq!(
            amocrm_contacts_url("https://mycompany.amocrm.ru/"),
            "https://mycompany.amocrm.ru/api/v4/contacts"
        );
        assert_eq!(
            bitrix24_contact_add_url("mycompany"),
            "https://mycompany.bitrix24.ru/rest/crm.contact.add.json"
        );
        // Full hosts (e.g. bitrix24.com or self-hosted) pass through.
        assert_eq!(
            bitrix24_contact_add_url("mycompany.bitrix24.com"),
            "https://mycompany.bitrix24.com/rest/crm.contact.add.json"
        );
    }

    #[test]
    fn test_amocrm_payload() {
        let payload = amocrm_payload(&sample_contact());
        assert_eq!(payload[0]["name"], "Jane Doe");
        let fields = payload[0]["custom_fields_values"].as_array().unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0]["field_code"], "PHONE");
        assert_eq!(fields[0]["values"][0]["value"], "+1 555 0100");
        assert_eq!(fields[1]["field_code"], "EMAIL");
        assert_eq!(fields[1]["values"][0]["value"], "jane@example.com");
    }

    #[test]
    fn test_amocrm_payload_name_fallback() {
        let mut c = Contact::new();
        c.email = Some("x@y.z".into());
        assert_eq!(amocrm_payload(&c)[0]["name"], "x@y.z");

        let empty = Contact::new();
        assert_eq!(amocrm_payload(&empty)[0]["name"], "Unknown contact");
    }

    #[test]
    fn test_bitrix24_fields() {
        let fields = bitrix24_fields(&sample_contact());
        assert_eq!(fields["NAME"], "Jane Doe");
        assert_eq!(fields["POST"], "CTO");
        assert_eq!(fields["COMPANY_TITLE"], "Acme");
        assert_eq!(fields["EMAIL"][0]["VALUE"], "jane@example.com");
        assert_eq!(fields["PHONE"][0]["VALUE"], "+1 555 0100");
        assert_eq!(fields["COMMENTS"], "vip");
    }

    #[test]
    fn test_hubspot_properties() {
        let props = hubspot_properties(&sample_contact());
        assert_eq!(props["email"], "jane@example.com");
        assert_eq!(props["phone"], "+1 555 0100");
        assert_eq!(props["firstname"], "Jane");
        assert_eq!(props["lastname"], "Doe");
        assert_eq!(props["jobtitle"], "CTO");
        assert_eq!(props["company"], "Acme");
    }

    #[test]
    fn test_hubspot_single_word_name() {
        let mut c = Contact::new();
        c.name = Some("Plato".into());
        let props = hubspot_properties(&c);
        assert_eq!(props["firstname"], "Plato");
        assert!(props.get("lastname").is_none());
    }

    // ── Config parsing ──────────────────────────────────────────────────

    #[test]
    fn test_provider_parse() {
        assert_eq!(
            CrmProvider::parse("amocrm", "myco", "key"),
            Some(CrmProvider::AmoCrm {
                domain: "myco".into(),
                api_key: "key".into()
            })
        );
        assert_eq!(
            CrmProvider::parse("Bitrix24", "myco", "key"),
            Some(CrmProvider::Bitrix24 {
                domain: "myco".into(),
                api_key: "key".into()
            })
        );
        assert_eq!(
            CrmProvider::parse("hubspot", "", "key"),
            Some(CrmProvider::HubSpot { api_key: "key".into() })
        );
        // Missing requirements.
        assert_eq!(CrmProvider::parse("amocrm", "", "key"), None);
        assert_eq!(CrmProvider::parse("hubspot", "", " "), None);
        assert_eq!(CrmProvider::parse("salesforce", "d", "k"), None);
        assert_eq!(CrmProvider::parse("", "d", "k"), None);
    }

    #[test]
    fn test_from_config() {
        let cfg = CrmConfig {
            provider: "hubspot".into(),
            domain: String::new(),
            api_key: "secret".into(),
        };
        let sync = CrmSync::from_config(&cfg).expect("valid config");
        assert_eq!(sync.provider().name(), "hubspot");

        let empty = CrmConfig::default();
        assert!(CrmSync::from_config(&empty).is_none());
    }

    // ── End-to-end pushes against a local mock server ───────────────────

    /// One-shot HTTP server that replies with `status_code` and
    /// `response_body`, and captures the raw request.
    async fn mock_server(
        status_code: u16,
        response_body: &'static str,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let mut header_end = None;
            let mut expected = None;
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                if header_end.is_none() {
                    if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                        header_end = Some(pos);
                        let headers = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                        expected = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok());
                    }
                }
                if let (Some(pos), Some(len)) = (header_end, expected) {
                    if buf.len() >= pos + 4 + len {
                        break;
                    }
                }
            }
            let reason = if status_code == 200 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {status_code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&buf).to_string()
        });
        (format!("http://{addr}"), handle)
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    #[tokio::test]
    async fn test_push_amocrm_success() {
        let (endpoint, server) = mock_server(
            200,
            r#"{"_embedded":{"contacts":[{"id":42,"name":"Jane Doe"}]}}"#,
        )
        .await;
        let sync = CrmSync::new(CrmProvider::AmoCrm {
            domain: "myco".into(),
            api_key: "token".into(),
        })
        .with_endpoint(format!("{endpoint}/api/v4/contacts"));

        let id = sync.push_contact(&sample_contact()).await.unwrap();
        assert_eq!(id, "42");

        // hyper writes header names in lower case on the wire.
        let request = server.await.unwrap().to_lowercase();
        assert!(request.contains("authorization: bearer token"));
        assert!(request.contains("jane doe"));
        assert!(request.contains("jane@example.com"));
    }

    #[tokio::test]
    async fn test_push_bitrix24_success() {
        let (endpoint, server) = mock_server(200, r#"{"result":777}"#).await;
        let sync = CrmSync::new(CrmProvider::Bitrix24 {
            domain: "myco".into(),
            api_key: "b24token".into(),
        })
        .with_endpoint(format!("{endpoint}/rest/crm.contact.add.json"));

        let id = sync.push_contact(&sample_contact()).await.unwrap();
        assert_eq!(id, "777");

        let request = server.await.unwrap();
        assert!(request.contains("auth=b24token"));
        assert!(request.contains("NAME"));
    }

    #[tokio::test]
    async fn test_push_hubspot_success() {
        let (endpoint, server) = mock_server(200, r#"{"id":"515","properties":{}}"#).await;
        let sync = CrmSync::new(CrmProvider::HubSpot {
            api_key: "hs-key".into(),
        })
        .with_endpoint(format!("{endpoint}/crm/v3/objects/contacts"));

        let id = sync.push_contact(&sample_contact()).await.unwrap();
        assert_eq!(id, "515");

        let request = server.await.unwrap().to_lowercase();
        assert!(request.contains("authorization: bearer hs-key"));
        assert!(request.contains("firstname"));
    }

    #[tokio::test]
    async fn test_push_hubspot_api_error_is_descriptive() {
        let (endpoint, server) = mock_server(
            401,
            r#"{"status":"error","message":"Invalid auth token","errors":[]}"#,
        )
        .await;
        let sync = CrmSync::new(CrmProvider::HubSpot {
            api_key: "bad".into(),
        })
        .with_endpoint(format!("{endpoint}/crm/v3/objects/contacts"));

        let err = sync.push_contact(&sample_contact()).await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("hubspot"));
        assert!(msg.contains("401"));
        assert!(msg.contains("Invalid auth token"));
        let _ = server.await;
    }

    #[test]
    fn test_api_error_message_extraction() {
        let amo: serde_json::Value =
            serde_json::from_str(r#"{"title":"Bad request","detail":"Invalid field"}"#).unwrap();
        assert_eq!(
            extract_api_error_message(&amo).as_deref(),
            Some("Invalid field")
        );

        let b24: serde_json::Value = serde_json::from_str(
            r#"{"error":"AUTH","error_description":"Access denied"}"#,
        )
        .unwrap();
        assert_eq!(
            extract_api_error_message(&b24).as_deref(),
            Some("Access denied")
        );

        let plain = serde_json::json!({"weird": true});
        assert_eq!(extract_api_error_message(&plain), None);
    }

    #[tokio::test]
    async fn test_push_connection_refused_is_error() {
        // Bind and immediately drop → connection refused.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let sync = CrmSync::new(CrmProvider::HubSpot { api_key: "k".into() })
            .with_endpoint(format!("http://{addr}/x"));
        let err = sync.push_contact(&sample_contact()).await.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("error"));
    }
}
