//! Opt-in Office extract (`.docx` / `.xlsx`) → inbox markdown for the composer.

use ade_core::error::AdeError;
use calamine::{Data, Reader, Xlsx};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::ZipArchive;

/// Cap extract body so inbox markdown stays harness-friendly.
pub const MAX_OFFICE_EXTRACT_CHARS: usize = 48_000;
/// Reject oversized Office packages before full parse.
pub const MAX_OFFICE_BYTES: u64 = 40 * 1024 * 1024;
/// Cap uncompressed `word/document.xml` size (docx zip-bomb guard).
pub const MAX_DOCX_XML_BYTES: u64 = 16 * 1024 * 1024;
/// Max worksheet rows per sheet (xlsx).
pub const MAX_XLSX_ROWS: usize = 200;
/// Max worksheets (xlsx).
pub const MAX_XLSX_SHEETS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeKind {
    Docx,
    Xlsx,
}

impl OfficeKind {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "docx" => Some(Self::Docx),
            "xlsx" => Some(Self::Xlsx),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OfficeExtractResult {
    pub kind: OfficeKind,
    pub text: String,
    pub truncated: bool,
    /// Human-readable scope (e.g. "3 paragraphs" / "2 sheets · 40 rows").
    pub scope: String,
}

/// Extract text from a `.docx` or `.xlsx` on disk.
pub fn extract_office(path: &Path) -> Result<OfficeExtractResult, AdeError> {
    let kind = OfficeKind::from_path(path).ok_or_else(|| {
        AdeError::Config(format!(
            "not an Office extract target (.docx/.xlsx): {}",
            path.display()
        ))
    })?;
    let meta = std::fs::metadata(path)
        .map_err(|error| AdeError::Config(format!("stat office {}: {error}", path.display())))?;
    let len = meta.len();
    if len > MAX_OFFICE_BYTES {
        return Err(AdeError::Config(format!(
            "office file too large ({len} bytes; max {MAX_OFFICE_BYTES}): {}",
            path.display()
        )));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| AdeError::Config(format!("read office {}: {error}", path.display())))?;
    if bytes.len() < 4 || &bytes[0..2] != b"PK" {
        return Err(AdeError::Config(format!(
            "not a ZIP Office package (missing PK header): {}",
            path.display()
        )));
    }
    match kind {
        OfficeKind::Docx => extract_docx_bytes(&bytes, path),
        OfficeKind::Xlsx => extract_xlsx_path(path),
    }
}

fn extract_docx_bytes(bytes: &[u8], path: &Path) -> Result<OfficeExtractResult, AdeError> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| AdeError::Config(format!("docx zip {}: {error}", path.display())))?;
    let mut entry = archive.by_name("word/document.xml").map_err(|error| {
        AdeError::Config(format!(
            "docx missing word/document.xml {}: {error}",
            path.display()
        ))
    })?;
    let uncompressed = entry.size();
    if uncompressed > MAX_DOCX_XML_BYTES {
        return Err(AdeError::Config(format!(
            "docx document.xml too large ({uncompressed} bytes; max {MAX_DOCX_XML_BYTES}): {}",
            path.display()
        )));
    }
    let mut xml = String::new();
    entry.read_to_string(&mut xml).map_err(|error| {
        AdeError::Config(format!(
            "docx read document.xml {}: {error}",
            path.display()
        ))
    })?;
    let paragraphs = docx_paragraphs(&xml);
    let para_count = paragraphs.len();
    let mut body = paragraphs.join("\n\n");
    if body.trim().is_empty() {
        body = "(No extractable text in document.xml — empty or unsupported runs.)".into();
    }
    let (text, truncated) = truncate_body(body);
    Ok(OfficeExtractResult {
        kind: OfficeKind::Docx,
        text,
        truncated,
        scope: format!(
            "{para_count} paragraph{}",
            if para_count == 1 { "" } else { "s" }
        ),
    })
}

fn extract_xlsx_path(path: &Path) -> Result<OfficeExtractResult, AdeError> {
    let mut workbook: Xlsx<_> = calamine::open_workbook(path)
        .map_err(|error| AdeError::Config(format!("xlsx open {}: {error}", path.display())))?;
    let names = workbook.sheet_names().to_vec();
    if names.is_empty() {
        return Ok(OfficeExtractResult {
            kind: OfficeKind::Xlsx,
            text: "(Workbook has no sheets.)".into(),
            truncated: false,
            scope: "0 sheets".into(),
        });
    }
    let mut parts: Vec<String> = Vec::new();
    let mut total_rows = 0usize;
    let mut sheets_used = 0usize;
    for name in names.iter().take(MAX_XLSX_SHEETS) {
        let Ok(range) = workbook.worksheet_range(name) else {
            continue;
        };
        sheets_used += 1;
        let mut lines: Vec<String> = Vec::new();
        for (idx, row) in range.rows().enumerate() {
            if idx >= MAX_XLSX_ROWS {
                lines.push("…[rows truncated]".into());
                break;
            }
            let cells: Vec<String> = row.iter().map(cell_to_string).collect();
            if cells.iter().all(|c| c.trim().is_empty()) {
                continue;
            }
            lines.push(cells.join("\t"));
            total_rows += 1;
        }
        let sheet_body = if lines.is_empty() {
            "(empty sheet)".to_string()
        } else {
            lines.join("\n")
        };
        parts.push(format!("## Sheet: {name}\n\n{sheet_body}"));
    }
    let mut body = parts.join("\n\n");
    if body.trim().is_empty() {
        body = "(No extractable cells.)".into();
    }
    let (text, truncated) = truncate_body(body);
    let more = if names.len() > MAX_XLSX_SHEETS {
        format!(" of {} listed", names.len())
    } else {
        String::new()
    };
    Ok(OfficeExtractResult {
        kind: OfficeKind::Xlsx,
        text,
        truncated,
        scope: format!(
            "{sheets_used} sheet{s}{more} · {total_rows} rows",
            s = if sheets_used == 1 { "" } else { "s" }
        ),
    })
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERR:{e:?}"),
    }
}

fn docx_paragraphs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in xml.split("</w:p>") {
        let mut para = String::new();
        let mut rest = chunk;
        while let Some(start) = rest.find("<w:t") {
            rest = &rest[start..];
            let Some(gt) = rest.find('>') else { break };
            let after = &rest[gt + 1..];
            let Some(end) = after.find("</w:t>") else {
                break;
            };
            para.push_str(&decode_xml_entities(&after[..end]));
            rest = &after[end + 6..];
        }
        let trimmed = para.trim();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn decode_xml_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn truncate_body(body: String) -> (String, bool) {
    let truncated = body.chars().count() > MAX_OFFICE_EXTRACT_CHARS;
    if !truncated {
        return (body, false);
    }
    let mut cut: String = body.chars().take(MAX_OFFICE_EXTRACT_CHARS).collect();
    cut.push_str("\n\n…[truncated]");
    (cut, true)
}

/// Markdown document for `.ade/inbox/*.extract.md`.
pub fn format_office_extract_markdown(
    source_label: &str,
    source_path: &str,
    result: &OfficeExtractResult,
) -> String {
    format!(
        "# Office extract ({})\n\n\
Source: {source_label}\n\
Path: {source_path}\n\
Scope: {}{}\n\n\
---\n\n\
{}\n",
        result.kind.label(),
        result.scope,
        if result.truncated {
            " · body truncated"
        } else {
            ""
        },
        result.text
    )
}

/// Build a minimal `.docx` (ZIP + document.xml) for tests and gold probes.
pub fn write_minimal_docx(path: &Path, paragraph: &str) -> Result<(), AdeError> {
    let file = std::fs::File::create(path).map_err(|e| AdeError::Config(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("[Content_Types].xml", opts)
        .map_err(|e| AdeError::Config(e.to_string()))?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .map_err(|e| AdeError::Config(e.to_string()))?;
    zip.start_file("_rels/.rels", opts)
        .map_err(|e| AdeError::Config(e.to_string()))?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .map_err(|e| AdeError::Config(e.to_string()))?;
    let escaped = paragraph
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>{escaped}</w:t></w:r></w:p>
  </w:body>
</w:document>"#
    );
    zip.start_file("word/document.xml", opts)
        .map_err(|e| AdeError::Config(e.to_string()))?;
    zip.write_all(document.as_bytes())
        .map_err(|e| AdeError::Config(e.to_string()))?;
    zip.finish().map_err(|e| AdeError::Config(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_office_extension() {
        let root = std::env::temp_dir().join(format!("ade-office-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("note.txt");
        std::fs::write(&path, b"hello").unwrap();
        let err = extract_office(&path).unwrap_err().to_string();
        assert!(err.contains("not an Office extract target"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_docx_paragraph() {
        let root = std::env::temp_dir().join(format!("ade-office-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("note.docx");
        write_minimal_docx(&path, "ADE Continuity Office dogfood").unwrap();
        let result = extract_office(&path).unwrap();
        assert_eq!(result.kind, OfficeKind::Docx);
        assert!(result.text.contains("ADE Continuity Office dogfood"));
        let md = format_office_extract_markdown("note.docx", "note.docx", &result);
        assert!(md.contains("Office extract (docx)"));
        let _ = std::fs::remove_dir_all(root);
    }
}
