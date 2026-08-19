//! PDF text extraction built on `lopdf`.
//!
//! Walks every page's content stream, follows text-showing operators
//! (`Tj`, `TJ`, `'`, `"`) and decodes glyph bytes using the page's font
//! encodings. Supports:
//! - simple fonts with standard encodings (Standard/WinAnsi/MacRoman/...)
//!   via `lopdf::Document::decode_text`
//! - CID (Type0) fonts with a `ToUnicode` CMap (the common case for
//!   PDFs produced by modern tools)
//! - Identity-encoded fonts without a CMap (best-effort CID→Unicode)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, ObjectId};
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};

/// Maximum characters returned by `pdf_extract` (default).
const DEFAULT_MAX_CHARS: usize = 50_000;

pub struct PdfTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PdfExtractParams {
    /// Path to the PDF file (resolved against the working directory)
    path: String,
    /// Maximum characters to return (default: 50000)
    #[serde(default = "default_max_chars")]
    max_chars: usize,
}

fn default_max_chars() -> usize {
    DEFAULT_MAX_CHARS
}

/// Per-font decoding state derived from the page's /Font resources.
#[derive(Debug, Default)]
struct FontInfo {
    /// Named encoding for simple fonts (e.g. "WinAnsiEncoding").
    encoding: Option<String>,
    /// Parsed /ToUnicode CMap (CID fonts).
    cmap: Option<CMap>,
}

/// A parsed ToUnicode CMap: code → Unicode string.
#[derive(Debug)]
struct CMap {
    map: HashMap<u32, String>,
    /// Bytes per character code (1 or 2, detected from the first entry).
    code_len: usize,
}

#[async_trait]
impl Tool for PdfTool {
    fn name(&self) -> &str {
        "pdf_extract"
    }
    fn description(&self) -> &str {
        "Extract text content from a PDF file.

## Capability

Parses the PDF locally (no network) and returns the concatenated text of all pages, in page order. Handles standard font encodings and ToUnicode CMaps (the common cases for both scanned-text and generated PDFs). Output is truncated at `max_chars` (default 50,000); the page count is included in metadata.

## When to Use

- Reading papers, reports, invoices, or any `.pdf` file during research.
- Pre-processing a PDF before summarizing or quoting from it.

## When NOT to Use

- Scanned image-only PDFs (no embedded text layer): extraction returns little or nothing; use `analyze_image` on rendered pages instead.
- Encrypted PDFs with a non-empty password: not supported.

## Failure Modes

- File not found or unreadable: check the path.
- Corrupt/non-PDF files: reported as a parse error.
- Password-protected PDFs: reported as an encryption error."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(PdfExtractParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: PdfExtractParams = serde_json::from_value(args)?;

        let full = resolve_pdf_path(&params.path, &ctx.working_dir);
        if !full.is_file() {
            return Ok(ToolOutput::err(format!(
                "PDF file not found: {}",
                full.display()
            )));
        }

        // lopdf is synchronous and CPU-bound; keep it off the async runtime.
        let display = full.display().to_string();
        let load_result = tokio::task::spawn_blocking(move || extract_pdf_text(&full)).await;

        match load_result {
            Ok(Ok((text, page_count))) => {
                let max_chars = params.max_chars.max(1000);
                let truncated = truncate_chars(&text, max_chars);
                let content = if truncated.trim().is_empty() {
                    format!(
                        "No extractable text found in {} ({} page(s)). The PDF may be scanned images — try analyze_image on rendered pages.",
                        display,
                        page_count
                    )
                } else {
                    format!("Source: {}\nPages: {}\n\n{}", display, page_count, truncated)
                };
                Ok(ToolOutput::ok_with_meta(
                    content,
                    serde_json::json!({ "pages": page_count, "chars": text.chars().count() }),
                ))
            }
            Ok(Err(e)) => Ok(ToolOutput::err(format!(
                "failed to extract text from {}: {e}",
                display
            ))),
            Err(e) => Ok(ToolOutput::err(format!("PDF extraction task failed: {e}"))),
        }
    }
}

fn resolve_pdf_path(path: &str, working_dir: &Path) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        working_dir.join(p)
    }
}

/// Extract text from every page of the PDF at `path`.
/// Returns `(text, page_count)`.
pub fn extract_pdf_text(path: &Path) -> anyhow::Result<(String, usize)> {
    let mut doc = Document::load(path)
        .map_err(|e| anyhow::anyhow!("could not parse PDF: {e}"))?;

    if doc.is_encrypted() {
        doc.decrypt("").map_err(|_| {
            anyhow::anyhow!("PDF is password-protected and could not be opened with an empty password")
        })?;
    }

    let pages = doc.get_pages();
    let page_count = pages.len();
    let mut out = String::new();

    for (_number, page_id) in pages {
        let fonts = build_font_map(&doc, page_id);
        match doc.get_page_content(page_id) {
            Ok(data) => {
                if let Ok(content) = Content::decode(&data) {
                    extract_page_text(&content, &fonts, &mut out);
                }
            }
            Err(_) => {
                // Pages without content streams contribute no text.
            }
        }
        out.push('\n');
    }

    // Collapse runs of blank lines introduced by positioning operators.
    let cleaned = collapse_blank_lines(&out);
    Ok((cleaned, page_count))
}

/// Build a map from in-content font name → decoding info for one page.
fn build_font_map(doc: &Document, page_id: ObjectId) -> HashMap<Vec<u8>, FontInfo> {
    let mut map: HashMap<Vec<u8>, FontInfo> = HashMap::new();

    for (name, font_dict) in doc.get_page_fonts(page_id) {
        let mut info = FontInfo::default();

        // /Encoding: name (simple fonts) or dictionary with /BaseEncoding.
        match font_dict.get(b"Encoding") {
            Ok(Object::Name(n)) => {
                info.encoding = Some(String::from_utf8_lossy(n).to_string());
            }
            Ok(Object::Dictionary(d)) => {
                if let Ok(Object::Name(base)) = d.get(b"BaseEncoding") {
                    info.encoding = Some(String::from_utf8_lossy(base).to_string());
                }
            }
            Ok(Object::Reference(id)) => {
                if let Ok(Object::Name(n)) = doc.get_object(*id) {
                    info.encoding = Some(String::from_utf8_lossy(n).to_string());
                }
            }
            _ => {}
        }

        // /ToUnicode CMap, direct stream or indirect reference.
        let tounicode_obj = match font_dict.get(b"ToUnicode") {
            Ok(Object::Reference(id)) => doc.get_object(*id).ok(),
            Ok(other) => Some(other),
            Err(_) => None,
        };
        if let Some(Object::Stream(stream)) = tounicode_obj {
            if let Ok(data) = stream.decompressed_content() {
                let cmap = parse_tounicode(&data);
                if !cmap.map.is_empty() {
                    info.cmap = Some(cmap);
                }
            }
        }

        map.insert(name, info);
    }

    map
}

/// Walk a page's content operations and append visible text to `out`.
fn extract_page_text(
    content: &Content<Vec<Operation>>,
    fonts: &HashMap<Vec<u8>, FontInfo>,
    out: &mut String,
) {
    let mut current_font: Option<Vec<u8>> = None;

    for op in &content.operations {
        match op.operator.as_str() {
            "Tf" => {
                if let Some(Object::Name(n)) = op.operands.first() {
                    current_font = Some(n.clone());
                }
            }
            "Tj" => {
                if let Some(Object::String(bytes, _)) = op.operands.first() {
                    let info = current_font.as_ref().and_then(|f| fonts.get(f));
                    out.push_str(&decode_glyphs(bytes, info));
                }
            }
            "TJ" => {
                if let Some(Object::Array(items)) = op.operands.first() {
                    let info = current_font.as_ref().and_then(|f| fonts.get(f));
                    for item in items {
                        if let Object::String(bytes, _) = item {
                            out.push_str(&decode_glyphs(bytes, info));
                        }
                    }
                }
            }
            // Move to next line and show text.
            "'" => {
                out.push('\n');
                if let Some(Object::String(bytes, _)) = op.operands.first() {
                    let info = current_font.as_ref().and_then(|f| fonts.get(f));
                    out.push_str(&decode_glyphs(bytes, info));
                }
            }
            // Set word/char spacing, move to next line, show text.
            "\"" => {
                out.push('\n');
                if let Some(Object::String(bytes, _)) = op.operands.last() {
                    let info = current_font.as_ref().and_then(|f| fonts.get(f));
                    out.push_str(&decode_glyphs(bytes, info));
                }
            }
            // Text positioning that usually starts a new visual line.
            "Td" | "TD" | "T*" | "Tm" | "ET" => {
                out.push('\n');
            }
            _ => {}
        }
    }
}

/// Decode raw glyph bytes using the font's CMap / encoding.
fn decode_glyphs(bytes: &[u8], info: Option<&FontInfo>) -> String {
    let Some(info) = info else {
        // Unknown font: best-effort UTF-8.
        return String::from_utf8_lossy(bytes).to_string();
    };

    if let Some(cmap) = &info.cmap {
        return decode_with_cmap(bytes, cmap);
    }

    match info.encoding.as_deref() {
        Some("Identity-H") | Some("Identity-V") => {
            // 2-byte CIDs; assume CID == Unicode (best effort without a CMap).
            bytes
                .chunks(2)
                .map(|c| {
                    if c.len() == 2 {
                        let code = u16::from_be_bytes([c[0], c[1]]);
                        char::from_u32(code as u32).unwrap_or(' ')
                    } else {
                        ' '
                    }
                })
                .collect()
        }
        _ => Document::decode_text(info.encoding.as_deref(), bytes),
    }
}

/// Decode bytes through a ToUnicode CMap (big-endian fixed-width codes).
fn decode_with_cmap(bytes: &[u8], cmap: &CMap) -> String {
    let mut s = String::new();
    let n = cmap.code_len.max(1);
    let mut i = 0;
    while i + n <= bytes.len() {
        let mut code: u32 = 0;
        for byte in &bytes[i..i + n] {
            code = (code << 8) | u32::from(*byte);
        }
        if let Some(mapped) = cmap.map.get(&code) {
            s.push_str(mapped);
        } else if let Some(ch) = char::from_u32(code) {
            // Fallback: many simple PDFs use identity CID→Unicode.
            if !ch.is_control() {
                s.push(ch);
            }
        }
        i += n;
    }
    s
}

/// Parse a ToUnicode CMap stream. Supports `beginbfchar`/`endbfchar` and
/// `beginbfrange`/`endbfrange` sections.
fn parse_tounicode(data: &[u8]) -> CMap {
    let text = String::from_utf8_lossy(data);
    let mut map: HashMap<u32, String> = HashMap::new();
    let mut code_len: usize = 2;
    let mut code_len_detected = false;

    #[derive(PartialEq)]
    enum Mode {
        None,
        BfChar,
        BfRange,
    }
    let mut mode = Mode::None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        // Section markers may carry a count prefix ("2 beginbfchar"),
        // so match on the last whitespace-separated token of the line.
        let marker = line.split_whitespace().last().unwrap_or("");
        match marker {
            "beginbfchar" => {
                mode = Mode::BfChar;
                continue;
            }
            "endbfchar" => {
                mode = Mode::None;
                continue;
            }
            "beginbfrange" => {
                mode = Mode::BfRange;
                continue;
            }
            "endbfrange" => {
                mode = Mode::None;
                continue;
            }
            _ => {}
        }

        let tokens = hex_tokens(line);
        if tokens.is_empty() {
            continue;
        }
        if !code_len_detected {
            if let Some(first) = tokens.first() {
                if !first.is_empty() {
                    code_len = first.len();
                    code_len_detected = true;
                }
            }
        }

        match mode {
            Mode::BfChar => {
                if tokens.len() >= 2 {
                    let code = be_code(&tokens[0]);
                    map.insert(code, utf16be_to_string(&tokens[1]));
                }
            }
            Mode::BfRange => {
                if tokens.len() == 3 {
                    // <lo> <hi> <dst-start>: contiguous range.
                    let lo = be_code(&tokens[0]);
                    let hi = be_code(&tokens[1]);
                    let mut dst = tokens[2].clone();
                    for code in lo..=hi {
                        map.insert(code, utf16be_to_string(&dst));
                        increment_be_u16(&mut dst);
                    }
                } else if tokens.len() > 3 {
                    // <lo> <hi> [<d1> <d2> ...]: explicit mapping list.
                    // (Square brackets are not hex tokens, so the array items
                    // start at index 2.)
                    let lo = be_code(&tokens[0]);
                    for (i, dst) in tokens[2..].iter().enumerate() {
                        map.insert(lo + i as u32, utf16be_to_string(dst));
                    }
                }
            }
            Mode::None => {}
        }
    }

    CMap { map, code_len }
}

/// Extract `<hex>` string payloads from a CMap line.
fn hex_tokens(line: &str) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut cur: Option<Vec<u8>> = None;
    for ch in line.chars() {
        match ch {
            '<' => cur = Some(Vec::new()),
            '>' => {
                if let Some(bytes) = cur.take() {
                    out.push(bytes);
                }
            }
            _ => {
                if let Some(ref mut bytes) = cur {
                    if ch.is_ascii_hexdigit() {
                        bytes.push(ch as u8);
                    }
                }
            }
        }
    }
    out.into_iter().map(hex_digits_to_bytes).collect()
}

/// Convert ASCII hex digits to bytes (odd length gets a trailing 0 nibble).
fn hex_digits_to_bytes(digits: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(digits.len().div_ceil(2));
    let mut i = 0;
    while i < digits.len() {
        let hi = hex_val(digits[i]);
        let lo = if i + 1 < digits.len() {
            i += 2;
            hex_val(digits[i - 1])
        } else {
            i += 1;
            0
        };
        bytes.push((hi << 4) | lo);
    }
    bytes
}

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

/// Interpret bytes as a big-endian integer code.
fn be_code(bytes: &[u8]) -> u32 {
    let mut code: u32 = 0;
    for b in bytes {
        code = (code << 8) | u32::from(*b);
    }
    code
}

/// Decode a UTF-16BE byte string (CMap dst values) into a Rust String.
fn utf16be_to_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// Increment the last 16-bit unit of a big-endian byte vector (for bfrange).
fn increment_be_u16(bytes: &mut [u8]) {
    if bytes.len() >= 2 {
        let n = bytes.len();
        let val = u16::from_be_bytes([bytes[n - 2], bytes[n - 1]]);
        let next = val.wrapping_add(1);
        bytes[n - 2] = (next >> 8) as u8;
        bytes[n - 1] = (next & 0xff) as u8;
    }
}

/// Collapse runs of >1 blank lines and trim trailing whitespace per line.
fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0usize;
    for line in s.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}...\n\n[Content truncated at {max_chars} characters]")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid single-page PDF with one uncompressed content
    /// stream showing `text` in Helvetica (WinAnsiEncoding). Xref offsets are
    /// computed programmatically so the file is structurally valid.
    fn build_minimal_pdf(text: &str) -> Vec<u8> {
        let escaped = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
            format!("<< /Length {} >>\nstream\nBT /F1 24 Tf 72 720 Td ({escaped}) Tj ET\nendstream", {
                let stream = format!("BT /F1 24 Tf 72 720 Td ({escaped}) Tj ET\n");
                stream.len()
            }),
        ];

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, obj).as_bytes());
        }
        let xref_pos = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            pdf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objects.len() + 1, xref_pos)
                .as_bytes(),
        );
        pdf
    }

    #[test]
    fn test_extract_minimal_pdf() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.pdf");
        std::fs::write(&path, build_minimal_pdf("Hello PDF World")).unwrap();

        let (text, pages) = extract_pdf_text(&path).unwrap();
        assert_eq!(pages, 1);
        assert!(
            text.contains("Hello PDF World"),
            "extracted text was: {text:?}"
        );
    }

    #[test]
    fn test_extract_missing_file_errors() {
        let err = extract_pdf_text(Path::new("/nonexistent/nope.pdf"));
        assert!(err.is_err());
    }

    #[test]
    fn test_extract_non_pdf_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("not-a-pdf.pdf");
        std::fs::write(&path, b"this is definitely not a pdf").unwrap();
        assert!(extract_pdf_text(&path).is_err());
    }

    #[test]
    fn test_hex_tokens_and_code() {
        let tokens = hex_tokens("<0041> <0042>");
        assert_eq!(tokens, vec![vec![0x00, 0x41], vec![0x00, 0x42]]);
        assert_eq!(be_code(&tokens[0]), 0x41);

        // Array form line.
        let tokens = hex_tokens("<0020> <0022> [<0041> <0042> <0043>]");
        assert_eq!(tokens.len(), 5);
    }

    #[test]
    fn test_parse_tounicode_bfchar() {
        let cmap_text = r#"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
2 beginbfchar
<0003> <0046>
<0004> <006F006F>
endbfchar
endcmap
"#;
        let cmap = parse_tounicode(cmap_text.as_bytes());
        assert_eq!(cmap.code_len, 2);
        assert_eq!(cmap.map.get(&0x3).unwrap(), "F");
        assert_eq!(cmap.map.get(&0x4).unwrap(), "oo");
    }

    #[test]
    fn test_parse_tounicode_bfrange_contiguous() {
        let cmap_text = r#"
1 beginbfrange
<0041> <0043> <0061>
endbfrange
"#;
        let cmap = parse_tounicode(cmap_text.as_bytes());
        assert_eq!(cmap.map.get(&0x41).unwrap(), "a");
        assert_eq!(cmap.map.get(&0x42).unwrap(), "b");
        assert_eq!(cmap.map.get(&0x43).unwrap(), "c");
    }

    #[test]
    fn test_parse_tounicode_bfrange_array() {
        let cmap_text = r#"
1 beginbfrange
<0010> <0012> [<0058> <0059> <005A>]
endbfrange
"#;
        let cmap = parse_tounicode(cmap_text.as_bytes());
        assert_eq!(cmap.map.get(&0x10).unwrap(), "X");
        assert_eq!(cmap.map.get(&0x11).unwrap(), "Y");
        assert_eq!(cmap.map.get(&0x12).unwrap(), "Z");
    }

    #[test]
    fn test_decode_with_cmap() {
        let mut map = HashMap::new();
        map.insert(0x0041u32, "H".to_string());
        map.insert(0x0042u32, "i".to_string());
        let cmap = CMap {
            map,
            code_len: 2,
        };
        assert_eq!(decode_with_cmap(&[0x00, 0x41, 0x00, 0x42], &cmap), "Hi");
        // Unknown code falls back to CID-as-Unicode ('C' = 0x43).
        assert_eq!(decode_with_cmap(&[0x00, 0x43], &cmap), "C");
    }

    #[test]
    fn test_decode_glyphs_identity_h() {
        let info = FontInfo {
            encoding: Some("Identity-H".to_string()),
            cmap: None,
        };
        assert_eq!(decode_glyphs(&[0x00, 0x48, 0x00, 0x69], Some(&info)), "Hi");
    }

    #[test]
    fn test_decode_glyphs_winansi() {
        let info = FontInfo {
            encoding: Some("WinAnsiEncoding".to_string()),
            cmap: None,
        };
        assert_eq!(decode_glyphs(b"abc", Some(&info)), "abc");
    }

    #[test]
    fn test_collapse_blank_lines() {
        let input = "line1\n\n\n\nline2\n   \n\nline3\n";
        let out = collapse_blank_lines(input);
        assert!(!out.contains("\n\n\n"));
        assert!(out.contains("line1\n\nline2"));
    }

    #[test]
    fn test_truncate_chars() {
        assert_eq!(truncate_chars("abc", 10), "abc");
        let t = truncate_chars("abcdefgh", 3);
        assert!(t.starts_with("abc..."));
    }

    #[test]
    fn test_pdf_extract_params_defaults() {
        let params: PdfExtractParams =
            serde_json::from_value(serde_json::json!({"path": "a.pdf"})).unwrap();
        assert_eq!(params.max_chars, DEFAULT_MAX_CHARS);
    }

    #[test]
    fn test_tool_name_and_schema() {
        let tool = PdfTool;
        assert_eq!(tool.name(), "pdf_extract");
        let schema = tool.schema();
        assert_eq!(schema.name, "pdf_extract");
        assert!(schema.parameters.is_object());
    }

    #[tokio::test]
    async fn test_pdf_tool_execute_end_to_end() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("doc.pdf"), build_minimal_pdf("End to end")).unwrap();

        let tool = PdfTool;
        let ctx = ToolContext::new(tmp.path().to_path_buf(), pr_core::SearchConfig::default());
        let out = tool
            .execute(serde_json::json!({"path": "doc.pdf"}), &ctx)
            .await
            .unwrap();
        assert!(out.success, "pdf_extract failed: {}", out.content);
        assert!(out.content.contains("End to end"));
        assert!(out.metadata.unwrap()["pages"] == 1);
    }

    #[tokio::test]
    async fn test_pdf_tool_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tool = PdfTool;
        let ctx = ToolContext::new(tmp.path().to_path_buf(), pr_core::SearchConfig::default());
        let out = tool
            .execute(serde_json::json!({"path": "missing.pdf"}), &ctx)
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("not found"));
    }
}
