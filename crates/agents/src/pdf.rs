//! First-N-pages PDF text extract for composer inbox packaging.

use ade_core::error::AdeError;
use std::path::Path;

/// Default pages extracted into `.ade/inbox/*.extract.md`.
pub const DEFAULT_PDF_EXTRACT_PAGES: usize = 8;
/// Cap extract body so inbox markdown stays harness-friendly.
pub const MAX_PDF_EXTRACT_CHARS: usize = 48_000;
/// Reject oversized PDFs before full parse (OOM / zip-bomb class inputs).
pub const MAX_PDF_BYTES: u64 = 40 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PdfExtractResult {
    pub pages_total: usize,
    pub pages_extracted: usize,
    pub text: String,
    pub truncated: bool,
}

/// Extract text from the first `max_pages` of a PDF (path on disk).
pub fn extract_pdf_text(
    path: &Path,
    max_pages: usize,
) -> Result<PdfExtractResult, AdeError> {
    let max_pages = max_pages.clamp(1, 40);
    let meta = std::fs::metadata(path).map_err(|error| {
        AdeError::Config(format!("stat pdf {}: {error}", path.display()))
    })?;
    let len = meta.len();
    if len > MAX_PDF_BYTES {
        return Err(AdeError::Config(format!(
            "pdf too large ({len} bytes; max {MAX_PDF_BYTES}): {}",
            path.display()
        )));
    }
    let bytes = std::fs::read(path).map_err(|error| {
        AdeError::Config(format!("read pdf {}: {error}", path.display()))
    })?;
    if bytes.len() < 5 || &bytes[0..4] != b"%PDF" {
        return Err(AdeError::Config(format!(
            "not a PDF (missing %PDF header): {}",
            path.display()
        )));
    }
    let pages = pdf_extract::extract_text_from_mem_by_pages(&bytes).map_err(|error| {
        AdeError::Config(format!("pdf extract {}: {error}", path.display()))
    })?;
    let pages_total = pages.len();
    let take = pages_total.min(max_pages);
    let mut body = String::new();
    for (idx, page) in pages.iter().take(take).enumerate() {
        let cleaned = page.split_whitespace().collect::<Vec<_>>().join(" ");
        if cleaned.is_empty() {
            continue;
        }
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str(&format!("## Page {}\n\n{cleaned}", idx + 1));
    }
    if body.trim().is_empty() {
        body = "(No extractable text on the first pages — scanned PDF or empty.)".into();
    }
    let truncated = body.chars().count() > MAX_PDF_EXTRACT_CHARS;
    if truncated {
        body = body.chars().take(MAX_PDF_EXTRACT_CHARS).collect::<String>();
        body.push_str("\n\n…[truncated]");
    }
    Ok(PdfExtractResult {
        pages_total,
        pages_extracted: take,
        text: body,
        truncated,
    })
}

/// Markdown document for `.ade/inbox/*.extract.md`.
pub fn format_extract_markdown(
    source_label: &str,
    source_path: &str,
    result: &PdfExtractResult,
) -> String {
    format!(
        "# PDF extract\n\n\
Source: {source_label}\n\
Path: {source_path}\n\
Pages: {} of {}{}\n\n\
---\n\n\
{}\n",
        result.pages_extracted,
        result.pages_total,
        if result.truncated {
            " · body truncated"
        } else {
            ""
        },
        result.text
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_pdf_bytes() {
        let root = std::env::temp_dir().join(format!("ade-pdf-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("note.txt");
        std::fs::write(&path, b"hello").unwrap();
        let err = extract_pdf_text(&path, 2).unwrap_err().to_string();
        assert!(err.contains("not a PDF"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_oversized_pdf() {
        let root = std::env::temp_dir().join(format!("ade-pdf-big-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("huge.pdf");
        let mut bytes = b"%PDF-1.4".to_vec();
        bytes.resize((MAX_PDF_BYTES as usize) + 8, b'0');
        std::fs::write(&path, &bytes).unwrap();
        let err = extract_pdf_text(&path, 2).unwrap_err().to_string();
        assert!(err.contains("too large"), "{err}");
        let _ = std::fs::remove_dir_all(root);
    }
}
