//! Result export: render a completed research session to PDF, HTML, JSON or DOCX.
//!
//! HTML and JSON are produced natively. PDF and DOCX are rendered through
//! `pandoc` (shell), falling back to `wkhtmltopdf` for PDF when pandoc is not
//! available. If neither binary is installed the export fails with a
//! descriptive error instead of panicking.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::contact::Contact;
use crate::session::SessionOutput;

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Pdf,
    Html,
    Json,
    Docx,
}

impl ExportFormat {
    /// Parse a format from a config string (`"pdf" | "html" | "json" | "docx"`),
    /// case-insensitive. Returns `None` for unknown values.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "html" => Some(Self::Html),
            "json" => Some(Self::Json),
            "docx" => Some(Self::Docx),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Json => "json",
            Self::Docx => "docx",
        }
    }

    pub fn extension(&self) -> &'static str {
        self.as_str()
    }

    /// All known format names.
    pub fn all() -> [ExportFormat; 4] {
        [Self::Pdf, Self::Html, Self::Json, Self::Docx]
    }
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ExportFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown export format '{s}' (expected one of: pdf, html, json, docx)"
            )
        })
    }
}

/// Contact export formats (see [`Exporter::export_contacts`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContactExportFormat {
    Csv,
    VCard,
    Json,
    Excel,
}

impl ContactExportFormat {
    /// Parse a format from a config string
    /// (`"csv" | "vcard" | "vcf" | "json" | "excel" | "xlsx"`),
    /// case-insensitive. Returns `None` for unknown values.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "vcard" | "vcf" => Some(Self::VCard),
            "json" => Some(Self::Json),
            "excel" | "xlsx" => Some(Self::Excel),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::VCard => "vcard",
            Self::Json => "json",
            Self::Excel => "excel",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::VCard => "vcf",
            Self::Json => "json",
            Self::Excel => "xlsx",
        }
    }

    /// All known contact format names.
    pub fn all() -> [ContactExportFormat; 4] {
        [Self::Csv, Self::VCard, Self::Json, Self::Excel]
    }
}

impl std::fmt::Display for ContactExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ContactExportFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown contact export format '{s}' (expected one of: csv, vcard, json, excel)"
            )
        })
    }
}

/// Exports finished research sessions into files inside `output_dir`.
#[derive(Debug, Clone)]
pub struct Exporter {
    output_dir: PathBuf,
}

impl Exporter {
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    /// Export the session into the requested format. Returns the path of the
    /// file that was written.
    pub async fn export(
        &self,
        session: &SessionOutput,
        format: ExportFormat,
    ) -> anyhow::Result<PathBuf> {
        std::fs::create_dir_all(&self.output_dir)?;
        match format {
            ExportFormat::Pdf => self.export_pdf(session).await,
            ExportFormat::Html => self.export_html(session).await,
            ExportFormat::Json => self.export_json(session).await,
            ExportFormat::Docx => self.export_docx(session).await,
        }
    }

    /// Target path for a given format.
    pub fn target_path(&self, format: ExportFormat) -> PathBuf {
        self.output_dir.join(format!("report.{}", format.extension()))
    }

    /// Target path for a contact export file.
    pub fn contacts_target_path(&self, format: ContactExportFormat) -> PathBuf {
        self.output_dir
            .join(format!("contacts.{}", format.extension()))
    }

    /// Export a set of contacts into the requested format. Returns the path
    /// of the file that was written.
    pub async fn export_contacts(
        &self,
        contacts: &[Contact],
        format: ContactExportFormat,
    ) -> anyhow::Result<PathBuf> {
        std::fs::create_dir_all(&self.output_dir)?;
        match format {
            ContactExportFormat::Csv => self.export_contacts_csv(contacts).await,
            ContactExportFormat::VCard => self.export_contacts_vcard(contacts).await,
            ContactExportFormat::Json => self.export_contacts_json(contacts).await,
            ContactExportFormat::Excel => self.export_contacts_excel(contacts).await,
        }
    }

    // ── Contact CSV ─────────────────────────────────────────────────────

    async fn export_contacts_csv(&self, contacts: &[Contact]) -> anyhow::Result<PathBuf> {
        let path = self.contacts_target_path(ContactExportFormat::Csv);
        tokio::fs::write(&path, contacts_to_csv(contacts)).await?;
        tracing::info!(path = %path.display(), count = contacts.len(), "exported contacts as CSV");
        Ok(path)
    }

    // ── Contact vCard ───────────────────────────────────────────────────

    async fn export_contacts_vcard(&self, contacts: &[Contact]) -> anyhow::Result<PathBuf> {
        let path = self.contacts_target_path(ContactExportFormat::VCard);
        tokio::fs::write(&path, contacts_to_vcard(contacts)).await?;
        tracing::info!(path = %path.display(), count = contacts.len(), "exported contacts as vCard");
        Ok(path)
    }

    // ── Contact JSON ────────────────────────────────────────────────────

    async fn export_contacts_json(&self, contacts: &[Contact]) -> anyhow::Result<PathBuf> {
        let path = self.contacts_target_path(ContactExportFormat::Json);
        tokio::fs::write(&path, serde_json::to_string_pretty(contacts)?).await?;
        tracing::info!(path = %path.display(), count = contacts.len(), "exported contacts as JSON");
        Ok(path)
    }

    // ── Contact Excel ───────────────────────────────────────────────────

    async fn export_contacts_excel(&self, contacts: &[Contact]) -> anyhow::Result<PathBuf> {
        let buffer = contacts_to_xlsx(contacts)?;
        let path = self.contacts_target_path(ContactExportFormat::Excel);
        tokio::fs::write(&path, buffer).await?;
        tracing::info!(path = %path.display(), count = contacts.len(), "exported contacts as Excel");
        Ok(path)
    }

    // ── HTML ────────────────────────────────────────────────────────────

    async fn export_html(&self, session: &SessionOutput) -> anyhow::Result<PathBuf> {
        let markdown = build_report_markdown(session);
        let html = render_html_document(session, &markdown);
        let path = self.target_path(ExportFormat::Html);
        tokio::fs::write(&path, html).await?;
        tracing::info!(path = %path.display(), "exported session as HTML");
        Ok(path)
    }

    // ── JSON ────────────────────────────────────────────────────────────

    async fn export_json(&self, session: &SessionOutput) -> anyhow::Result<PathBuf> {
        let findings = collect_findings(&session.output_dir);

        let findings_json: Vec<serde_json::Value> = findings
            .iter()
            .map(|(file, content)| {
                serde_json::json!({
                    "file": file,
                    "content": content,
                    "sources": extract_urls(content),
                })
            })
            .collect();

        let payload = serde_json::json!({
            "session_id": session.session_id.0,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "output_dir": session.output_dir.display().to_string(),
            "total_tokens": session.total_tokens,
            "total_agents": session.total_agents,
            "synthesis": {
                "markdown": session.synthesis,
                "sources": extract_urls(&session.synthesis),
            },
            "findings": findings_json,
        });

        let path = self.target_path(ExportFormat::Json);
        tokio::fs::write(&path, serde_json::to_string_pretty(&payload)?).await?;
        tracing::info!(path = %path.display(), "exported session as JSON");
        Ok(path)
    }

    // ── PDF ─────────────────────────────────────────────────────────────

    async fn export_pdf(&self, session: &SessionOutput) -> anyhow::Result<PathBuf> {
        let markdown_path = self.write_report_markdown(session).await?;
        let target = self.target_path(ExportFormat::Pdf);

        // Preferred: pandoc (uses its default PDF engine).
        if pandoc_available().await {
            let args: Vec<String> = vec![
                markdown_path.display().to_string(),
                "-o".to_string(),
                target.display().to_string(),
                "--standalone".to_string(),
            ];
            match run_pandoc(&args).await {
                Ok(()) => {
                    tracing::info!(path = %target.display(), "exported session as PDF via pandoc");
                    return Ok(target);
                }
                Err(e) => {
                    tracing::warn!("pandoc PDF rendering failed ({e}); trying wkhtmltopdf");
                }
            }
        }

        // Fallback: wkhtmltopdf on the HTML rendering.
        if tool_available("wkhtmltopdf").await {
            let html_path = self.export_html(session).await?;
            let status = tokio::process::Command::new("wkhtmltopdf")
                .arg(html_path.display().to_string())
                .arg(target.display().to_string())
                .output()
                .await?;
            if status.status.success() {
                tracing::info!(path = %target.display(), "exported session as PDF via wkhtmltopdf");
                return Ok(target);
            }
            anyhow::bail!(
                "wkhtmltopdf failed: {}",
                String::from_utf8_lossy(&status.stderr).trim()
            );
        }

        anyhow::bail!(
            "PDF export requires 'pandoc' (with a PDF engine such as pdflatex/weasyprint) \
             or 'wkhtmltopdf' to be installed; neither was found in PATH"
        )
    }

    // ── DOCX ────────────────────────────────────────────────────────────

    async fn export_docx(&self, session: &SessionOutput) -> anyhow::Result<PathBuf> {
        if !pandoc_available().await {
            anyhow::bail!("DOCX export requires 'pandoc' to be installed; it was not found in PATH");
        }
        let markdown_path = self.write_report_markdown(session).await?;
        let target = self.target_path(ExportFormat::Docx);
        let args: Vec<String> = vec![
            markdown_path.display().to_string(),
            "-o".to_string(),
            target.display().to_string(),
        ];
        run_pandoc(&args).await?;
        tracing::info!(path = %target.display(), "exported session as DOCX via pandoc");
        Ok(target)
    }

    /// Write the combined report markdown used as pandoc input.
    async fn write_report_markdown(&self, session: &SessionOutput) -> anyhow::Result<PathBuf> {
        let path = self.output_dir.join("report.md");
        tokio::fs::write(&path, build_report_markdown(session)).await?;
        Ok(path)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build one standalone markdown document: metadata header, synthesis and an
/// appendix with the individual findings.
pub fn build_report_markdown(session: &SessionOutput) -> String {
    let mut md = String::new();
    md.push_str("# Research Report\n\n");
    md.push_str(&format!("- **Session**: {}\n", session.session_id));
    md.push_str(&format!("- **Generated**: {}\n", chrono::Utc::now().to_rfc3339()));
    md.push_str(&format!("- **Agents**: {}\n", session.total_agents));
    md.push_str(&format!("- **Tokens used**: {}\n\n", session.total_tokens));
    md.push_str("---\n\n");
    md.push_str(session.synthesis.trim());
    md.push('\n');

    let findings = collect_findings(&session.output_dir);
    if !findings.is_empty() {
        md.push_str("\n---\n\n## Appendix: Individual Findings\n");
        for (file, content) in &findings {
            md.push_str(&format!("\n### {}\n\n{}\n", file, content.trim()));
        }
    }
    md
}

/// Read `findings/*.md` from the session output directory, sorted by name.
pub fn collect_findings(output_dir: &Path) -> Vec<(String, String)> {
    let dir = output_dir.join("findings");
    let mut results = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return results,
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort();
    for path in files {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            results.push((name, content));
        }
    }
    results
}

/// Extract http(s) URLs from free text. Used to build source lists in the
/// JSON/HTML exports.
pub fn extract_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut search_from = 0;
    let bytes = text.as_bytes();
    while search_from < bytes.len() {
        let rest = &text[search_from..];
        let hit = ["https://", "http://"]
            .iter()
            .filter_map(|scheme| rest.find(scheme))
            .min();
        let Some(offset) = hit else { break };
        let start = search_from + offset;
        let tail = &text[start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | ')' | ']' | '`'))
            .unwrap_or(tail.len());
        let url = tail[..end].trim_end_matches(['.', ',', ';', ':']);
        if !url.is_empty() && !urls.contains(&url.to_string()) {
            urls.push(url.to_string());
        }
        search_from = start + end.max(1);
    }
    urls
}

// ── Contact export builders ─────────────────────────────────────────────────

/// CSV column headers used by [`contacts_to_csv`] and the Excel export.
pub const CONTACT_EXPORT_COLUMNS: [&str; 10] = [
    "id",
    "name",
    "title",
    "company",
    "email",
    "phone",
    "tags",
    "social_profiles",
    "source",
    "created_at",
];

/// Render contacts as CSV (RFC 4180 quoting).
pub fn contacts_to_csv(contacts: &[Contact]) -> String {
    let mut out = String::new();
    out.push_str(&CONTACT_EXPORT_COLUMNS.map(csv_field).join(","));
    out.push_str("\r\n");

    for contact in contacts {
        let socials: Vec<String> = contact
            .social_profiles
            .iter()
            .map(|sp| {
                if sp.url.is_empty() {
                    format!("{}:{}", sp.platform, sp.username)
                } else {
                    format!("{}:{}", sp.platform, sp.url)
                }
            })
            .collect();
        let row = [
            contact.id.map(|id| id.to_string()).unwrap_or_default(),
            contact.name.clone().unwrap_or_default(),
            contact.title.clone().unwrap_or_default(),
            contact.company.clone().unwrap_or_default(),
            contact.email.clone().unwrap_or_default(),
            contact.phone.clone().unwrap_or_default(),
            contact.tags.join("; "),
            socials.join("; "),
            contact.source.clone(),
            contact.created_at.to_rfc3339(),
        ];
        out.push_str(&row.map(|v| csv_field(&v)).join(","));
        out.push_str("\r\n");
    }
    out
}

/// Quote a single CSV field when needed.
pub fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Render contacts as a vCard 3.0 document (CRLF line endings per RFC 2426).
pub fn contacts_to_vcard(contacts: &[Contact]) -> String {
    let mut out = String::new();
    for contact in contacts {
        out.push_str("BEGIN:VCARD\r\nVERSION:3.0\r\n");

        let name = contact.name.clone().unwrap_or_default();
        let (given, family) = vcard_name_parts(&name);
        out.push_str(&format!("N:{};{};;;\r\n", vcard_escape(&family), vcard_escape(&given)));
        out.push_str(&format!("FN:{}\r\n", vcard_escape(&name)));

        if let Some(company) = contact.company.as_deref().filter(|s| !s.trim().is_empty()) {
            out.push_str(&format!("ORG:{}\r\n", vcard_escape(company)));
        }
        if let Some(title) = contact.title.as_deref().filter(|s| !s.trim().is_empty()) {
            out.push_str(&format!("TITLE:{}\r\n", vcard_escape(title)));
        }
        if let Some(email) = contact.email.as_deref().filter(|s| !s.trim().is_empty()) {
            out.push_str(&format!("EMAIL;TYPE=INTERNET:{}\r\n", vcard_escape(email)));
        }
        if let Some(phone) = contact.phone.as_deref().filter(|s| !s.trim().is_empty()) {
            out.push_str(&format!("TEL:{}\r\n", vcard_escape(phone)));
        }
        for sp in &contact.social_profiles {
            if !sp.url.is_empty() {
                out.push_str(&format!(
                    "X-SOCIALPROFILE;TYPE={}:{}\r\n",
                    vcard_escape(&sp.platform),
                    vcard_escape(&sp.url)
                ));
            }
        }
        if !contact.notes.is_empty() {
            out.push_str(&format!("NOTE:{}\r\n", vcard_escape(&contact.notes.join("\n"))));
        }
        if !contact.tags.is_empty() {
            // CATEGORIES is a comma-separated list: escape each tag but keep
            // the commas as separators.
            let categories: Vec<String> =
                contact.tags.iter().map(|t| vcard_escape(t)).collect();
            out.push_str(&format!("CATEGORIES:{}\r\n", categories.join(",")));
        }

        out.push_str("END:VCARD\r\n");
    }
    out
}

/// Split a full name into (given, family) for the vCard `N` property.
fn vcard_name_parts(name: &str) -> (String, String) {
    let mut parts = name.split_whitespace();
    let given = parts.next().unwrap_or_default().to_string();
    let family = parts.collect::<Vec<_>>().join(" ");
    (given, family)
}

/// Escape a vCard TEXT value (RFC 2426).
pub fn vcard_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace('\n', "\\n")
}

/// Render contacts as an XLSX workbook and return the file bytes.
pub fn contacts_to_xlsx(contacts: &[Contact]) -> anyhow::Result<Vec<u8>> {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let worksheet = workbook.add_worksheet().set_name("Contacts")?;

    for (col, header) in CONTACT_EXPORT_COLUMNS.iter().enumerate() {
        worksheet.write_string(0, col as u16, *header)?;
    }

    for (row_idx, contact) in contacts.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        if let Some(id) = contact.id {
            worksheet.write_number(row, 0, id as f64)?;
        }
        worksheet.write_string(row, 1, contact.name.as_deref().unwrap_or_default())?;
        worksheet.write_string(row, 2, contact.title.as_deref().unwrap_or_default())?;
        worksheet.write_string(row, 3, contact.company.as_deref().unwrap_or_default())?;
        worksheet.write_string(row, 4, contact.email.as_deref().unwrap_or_default())?;
        worksheet.write_string(row, 5, contact.phone.as_deref().unwrap_or_default())?;
        worksheet.write_string(row, 6, &contact.tags.join("; "))?;
        let socials: Vec<String> = contact
            .social_profiles
            .iter()
            .map(|sp| format!("{}:{}", sp.platform, if sp.url.is_empty() { &sp.username } else { &sp.url }))
            .collect();
        worksheet.write_string(row, 7, &socials.join("; "))?;
        worksheet.write_string(row, 8, &contact.source)?;
        worksheet.write_string(row, 9, &contact.created_at.to_rfc3339())?;
    }

    Ok(workbook.save_to_buffer()?)
}

/// Render the full styled HTML document for a session.
pub fn render_html_document(session: &SessionOutput, markdown: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut body = String::new();
    html::push_html(&mut body, parser);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Research Report — {session_id}</title>
<style>
  :root {{ color-scheme: light dark; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    line-height: 1.6; max-width: 860px; margin: 0 auto; padding: 2rem 1.5rem;
  }}
  h1, h2, h3 {{ line-height: 1.25; }}
  h1 {{ border-bottom: 2px solid #8884; padding-bottom: .3rem; }}
  h2 {{ border-bottom: 1px solid #8883; padding-bottom: .2rem; margin-top: 2rem; }}
  code {{ background: #8882; padding: .1rem .35rem; border-radius: 4px; font-size: .92em; }}
  pre {{ background: #8882; padding: 1rem; border-radius: 8px; overflow-x: auto; }}
  pre code {{ background: transparent; padding: 0; }}
  blockquote {{ border-left: 4px solid #8886; margin-left: 0; padding-left: 1rem; opacity: .85; }}
  table {{ border-collapse: collapse; width: 100%; }}
  th, td {{ border: 1px solid #8885; padding: .4rem .6rem; text-align: left; }}
  a {{ color: #2f6fed; }}
  hr {{ border: none; border-top: 1px solid #8884; margin: 2rem 0; }}
  .meta {{ opacity: .75; font-size: .9rem; }}
</style>
</head>
<body>
{body}
<p class="meta">Generated by Parallel Research Agent · session {session_id} · {generated}</p>
</body>
</html>
"#,
        session_id = html_escape(&session.session_id.0),
        body = body,
        generated = chrono::Utc::now().to_rfc3339(),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Check whether `pandoc` is available in PATH.
pub async fn pandoc_available() -> bool {
    tool_available("pandoc").await
}

async fn tool_available(name: &str) -> bool {
    let probe = if cfg!(windows) { "where" } else { "which" };
    tokio::process::Command::new(probe)
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

async fn run_pandoc(args: &[String]) -> anyhow::Result<()> {
    let output = tokio::process::Command::new("pandoc")
        .args(args)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("failed to launch pandoc: {e}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "pandoc failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

// ── Export-time deduplication ────────────────────────────────────────────────

/// Dedup key priority for export: normalized email first, then normalized
/// phone, then a `name + email-domain` pair (catches the same person listed
/// once with a full and once with a bare name only when names match
/// exactly — anything fuzzier is left for the interactive `contacts dedup`
/// merge). Contacts with no identity at all return `None` and are always
/// kept.
fn export_dedup_key(c: &Contact) -> Option<String> {
    if let Some(e) = c.normalized_email() {
        return Some(format!("email:{e}"));
    }
    if let Some(p) = c.normalized_phone() {
        return Some(format!("phone:{p}"));
    }
    let name = c.name.as_deref().unwrap_or("").trim().to_lowercase();
    if !name.is_empty() {
        let domain = c
            .email
            .as_deref()
            .and_then(|e| e.split('@').nth(1))
            .unwrap_or("")
            .to_lowercase();
        return Some(format!("name:{name}|domain:{domain}"));
    }
    None
}

/// Deduplicate contacts before export. Returns the deduplicated list
/// (first occurrence wins, preserving order) and the number of rows
/// dropped.
pub fn dedup_contacts(contacts: Vec<Contact>) -> (Vec<Contact>, usize) {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(contacts.len());
    let total = contacts.len();
    for c in contacts {
        match export_dedup_key(&c) {
            Some(key) => {
                if seen.insert(key) {
                    out.push(c);
                }
            }
            None => out.push(c),
        }
    }
    let dropped = total - out.len();
    (out, dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::SessionId;

    fn sample_session(dir: PathBuf) -> SessionOutput {
        SessionOutput {
            session_id: SessionId("sess-export-test".to_string()),
            output_dir: dir,
            synthesis: "# Answer\n\nRust is fast. See https://example.com/rust and https://rust-lang.org.".to_string(),
            total_tokens: 500,
            total_agents: 2,
        }
    }

    #[test]
    fn test_format_parse_roundtrip() {
        for fmt in ExportFormat::all() {
            assert_eq!(ExportFormat::parse(fmt.as_str()), Some(fmt));
            assert_eq!(fmt.to_string(), fmt.extension());
        }
        assert_eq!(ExportFormat::parse("HTML"), Some(ExportFormat::Html));
        assert_eq!(ExportFormat::parse(" docx "), Some(ExportFormat::Docx));
        assert_eq!(ExportFormat::parse("xml"), None);
        assert!("xml".parse::<ExportFormat>().is_err());
    }

    #[test]
    fn test_extract_urls() {
        let text = "See https://a.com/x, http://b.org/y. Also (https://c.io/z) done";
        let urls = extract_urls(text);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://a.com/x");
        assert_eq!(urls[1], "http://b.org/y");
        assert_eq!(urls[2], "https://c.io/z");
    }

    #[test]
    fn test_extract_urls_deduplicates() {
        let urls = extract_urls("https://a.com and https://a.com again");
        assert_eq!(urls, vec!["https://a.com"]);
    }

    #[test]
    fn test_collect_findings_sorted_and_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        // Missing findings dir → empty, no panic.
        assert!(collect_findings(tmp.path()).is_empty());

        let findings = tmp.path().join("findings");
        std::fs::create_dir_all(&findings).unwrap();
        std::fs::write(findings.join("finding-2.md"), "second").unwrap();
        std::fs::write(findings.join("finding-1.md"), "first").unwrap();
        std::fs::write(findings.join("notes.txt"), "not markdown").unwrap();

        let collected = collect_findings(tmp.path());
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].0, "finding-1.md");
        assert_eq!(collected[1].1, "second");
    }

    #[test]
    fn test_build_report_markdown_contains_metadata_and_findings() {
        let tmp = tempfile::tempdir().unwrap();
        let findings = tmp.path().join("findings");
        std::fs::create_dir_all(&findings).unwrap();
        std::fs::write(findings.join("finding-1.md"), "finding body").unwrap();

        let md = build_report_markdown(&sample_session(tmp.path().to_path_buf()));
        assert!(md.contains("# Research Report"));
        assert!(md.contains("sess-export-test"));
        assert!(md.contains("**Agents**: 2"));
        assert!(md.contains("Rust is fast"));
        assert!(md.contains("### finding-1.md"));
        assert!(md.contains("finding body"));
    }

    #[test]
    fn test_render_html_document_escapes_and_converts() {
        let html = render_html_document(
            &sample_session(PathBuf::from("/tmp")),
            "# Heading\n\nSome **bold** text with <script>alert(1)</script>.",
        );
        assert!(html.contains("<h1>Heading</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        // Raw HTML from markdown is passed through by pulldown-cmark by default;
        // session id interpolation must be escaped.
        assert!(html.contains("sess-export-test"));
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[tokio::test]
    async fn test_export_html_writes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let session = sample_session(tmp.path().to_path_buf());
        let exporter = Exporter::new(tmp.path().to_path_buf());

        let path = exporter.export(&session, ExportFormat::Html).await.unwrap();
        assert_eq!(path, tmp.path().join("report.html"));
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Rust is fast"));
    }

    #[tokio::test]
    async fn test_export_json_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let findings = tmp.path().join("findings");
        std::fs::create_dir_all(&findings).unwrap();
        std::fs::write(
            findings.join("finding-1.md"),
            "Sub-finding with source https://example.org/ref",
        )
        .unwrap();

        let session = sample_session(tmp.path().to_path_buf());
        let exporter = Exporter::new(tmp.path().to_path_buf());

        let path = exporter.export(&session, ExportFormat::Json).await.unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        assert_eq!(value["session_id"], "sess-export-test");
        assert_eq!(value["total_tokens"], 500);
        assert_eq!(value["total_agents"], 2);
        assert!(value["synthesis"]["markdown"].as_str().unwrap().contains("Rust is fast"));
        assert_eq!(
            value["synthesis"]["sources"][0],
            "https://example.com/rust"
        );
        assert_eq!(value["findings"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["findings"][0]["sources"][0],
            "https://example.org/ref"
        );
    }

    #[tokio::test]
    async fn test_pdf_docx_fail_gracefully_without_pandoc() {
        // Regardless of whether pandoc is installed, the call must not panic:
        // it either succeeds or returns a descriptive error.
        let tmp = tempfile::tempdir().unwrap();
        let session = sample_session(tmp.path().to_path_buf());
        let exporter = Exporter::new(tmp.path().to_path_buf());

        match exporter.export(&session, ExportFormat::Docx).await {
            Ok(path) => assert!(path.exists()),
            Err(e) => assert!(e.to_string().to_lowercase().contains("pandoc")),
        }
    }

    // ── Contact export ──────────────────────────────────────────────────

    fn sample_contacts() -> Vec<Contact> {
        let mut first = Contact::new().with_source("test");
        first.id = Some(1);
        first.name = Some("Jane Doe".into());
        first.title = Some("CTO".into());
        first.company = Some("Acme, Inc.".into());
        first.email = Some("jane@example.com".into());
        first.phone = Some("+1 555 0100".into());
        first.tags = vec!["lead".into(), "vip".into()];
        first.social_profiles.push(crate::contact::SocialProfile::new(
            "linkedin",
            "https://linkedin.com/in/jdoe",
            "jdoe",
        ));
        first.notes.push("Met at RustConf".into());

        let mut second = Contact::new().with_source("test");
        second.id = Some(2);
        second.name = Some("Bob \"The Builder\" Smith".into());
        second.email = Some("bob@example.com".into());

        vec![first, second]
    }

    #[test]
    fn test_contact_format_parse_roundtrip() {
        for fmt in ContactExportFormat::all() {
            assert_eq!(ContactExportFormat::parse(fmt.as_str()), Some(fmt));
        }
        assert_eq!(ContactExportFormat::parse("VCF"), Some(ContactExportFormat::VCard));
        assert_eq!(ContactExportFormat::parse(" xlsx "), Some(ContactExportFormat::Excel));
        assert_eq!(ContactExportFormat::parse("yaml"), None);
        assert!("yaml".parse::<ContactExportFormat>().is_err());
    }

    #[test]
    fn test_dedup_contacts_by_email_case_insensitive() {
        let mut a = Contact::new();
        a.email = Some("Jane@Example.com".into());
        let mut b = Contact::new();
        b.email = Some("jane@example.com".into());
        let mut c = Contact::new();
        c.email = Some("other@example.com".into());
        let (out, dropped) = dedup_contacts(vec![a, b, c]);
        assert_eq!(out.len(), 2);
        assert_eq!(dropped, 1);
        assert_eq!(out[0].email.as_deref(), Some("Jane@Example.com"));
    }

    #[test]
    fn test_dedup_contacts_by_name_and_domain() {
        // Same person, same company domain, no email match (different local
        // parts would be distinct emails; here both emails are absent and
        // only name+domain identity remains).
        let mut a = Contact::new();
        a.name = Some("Maria Ivanova".into());
        a.company = Some("Acme".into());
        let mut b = Contact::new();
        b.name = Some("maria ivanova".into());
        b.company = Some("Acme".into());
        let mut c = Contact::new();
        c.name = Some("Someone Else".into());
        let (out, dropped) = dedup_contacts(vec![a, b, c]);
        assert_eq!(out.len(), 2);
        assert_eq!(dropped, 1);
    }

    #[test]
    fn test_dedup_contacts_keeps_identityless_rows() {
        let a = Contact::new();
        let b = Contact::new();
        let (out, dropped) = dedup_contacts(vec![a, b]);
        assert_eq!(out.len(), 2);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn test_contacts_to_csv_quotes_and_escapes() {
        let csv = contacts_to_csv(&sample_contacts());
        let mut lines = csv.lines();
        assert_eq!(lines.next().unwrap(), "id,name,title,company,email,phone,tags,social_profiles,source,created_at");

        let first = lines.next().unwrap();
        assert!(first.starts_with("1,Jane Doe,CTO,\"Acme, Inc.\",jane@example.com"));
        assert!(first.contains("lead; vip"));
        assert!(first.contains("linkedin:https://linkedin.com/in/jdoe"));

        let second = lines.next().unwrap();
        // Embedded quotes are doubled per RFC 4180.
        assert!(second.contains("\"Bob \"\"The Builder\"\" Smith\""));
    }

    #[test]
    fn test_csv_field_escaping() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn test_contacts_to_vcard_structure() {
        let vcard = contacts_to_vcard(&sample_contacts());
        assert!(vcard.starts_with("BEGIN:VCARD\r\nVERSION:3.0\r\n"));
        assert!(vcard.contains("N:Doe;Jane;;;\r\n"));
        assert!(vcard.contains("FN:Jane Doe\r\n"));
        assert!(vcard.contains("ORG:Acme\\, Inc.\r\n"));
        assert!(vcard.contains("TITLE:CTO\r\n"));
        assert!(vcard.contains("EMAIL;TYPE=INTERNET:jane@example.com\r\n"));
        assert!(vcard.contains("TEL:+1 555 0100\r\n"));
        assert!(vcard.contains("X-SOCIALPROFILE;TYPE=linkedin:https://linkedin.com/in/jdoe\r\n"));
        assert!(vcard.contains("NOTE:Met at RustConf\r\n"));
        assert!(vcard.contains("CATEGORIES:lead,vip\r\n"));
        assert!(vcard.ends_with("END:VCARD\r\n"));
        // Two cards.
        assert_eq!(vcard.matches("BEGIN:VCARD").count(), 2);
    }

    #[test]
    fn test_vcard_escape() {
        assert_eq!(vcard_escape("a;b,c\\d\ne"), "a\\;b\\,c\\\\d\\ne");
    }

    #[test]
    fn test_contacts_to_xlsx_is_valid_zip() {
        let buffer = contacts_to_xlsx(&sample_contacts()).unwrap();
        // XLSX files are ZIP archives ("PK" magic bytes).
        assert!(buffer.len() > 500);
        assert_eq!(&buffer[..2], b"PK");
    }

    #[tokio::test]
    async fn test_export_contacts_all_formats() {
        let tmp = tempfile::tempdir().unwrap();
        let exporter = Exporter::new(tmp.path().to_path_buf());
        let contacts = sample_contacts();

        let csv = exporter
            .export_contacts(&contacts, ContactExportFormat::Csv)
            .await
            .unwrap();
        assert_eq!(csv, tmp.path().join("contacts.csv"));
        assert!(std::fs::read_to_string(&csv).unwrap().contains("jane@example.com"));

        let vcf = exporter
            .export_contacts(&contacts, ContactExportFormat::VCard)
            .await
            .unwrap();
        assert_eq!(vcf, tmp.path().join("contacts.vcf"));
        assert!(std::fs::read_to_string(&vcf).unwrap().contains("BEGIN:VCARD"));

        let json = exporter
            .export_contacts(&contacts, ContactExportFormat::Json)
            .await
            .unwrap();
        assert_eq!(json, tmp.path().join("contacts.json"));
        let parsed: Vec<Contact> =
            serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].email.as_deref(), Some("jane@example.com"));

        let xlsx = exporter
            .export_contacts(&contacts, ContactExportFormat::Excel)
            .await
            .unwrap();
        assert_eq!(xlsx, tmp.path().join("contacts.xlsx"));
        let bytes = std::fs::read(&xlsx).unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }

    #[tokio::test]
    async fn test_export_contacts_empty_list() {
        let tmp = tempfile::tempdir().unwrap();
        let exporter = Exporter::new(tmp.path().to_path_buf());

        let csv = exporter
            .export_contacts(&[], ContactExportFormat::Csv)
            .await
            .unwrap();
        let content = std::fs::read_to_string(&csv).unwrap();
        // Header only.
        assert_eq!(content.lines().count(), 1);

        let json = exporter
            .export_contacts(&[], ContactExportFormat::Json)
            .await
            .unwrap();
        assert_eq!(std::fs::read_to_string(&json).unwrap(), "[]");
    }
}
