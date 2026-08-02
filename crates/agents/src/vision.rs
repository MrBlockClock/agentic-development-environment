//! Vision / multimodal helpers: model capability + OpenAI-compatible image parts.

use ade_core::error::AdeError;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAX_VISION_IMAGES: usize = 4;
const MAX_VISION_BYTES: u64 = 5 * 1024 * 1024;

/// Dedicated image-token bands for SpendGuard (not base64 char÷4 inflation).
/// Roughly mirrors OpenAI low/high detail tile weights.
pub fn estimate_image_tokens_for_bytes(bytes: u64) -> u32 {
    const BASE: u32 = 85;
    if bytes == 0 {
        return BASE;
    }
    if bytes < 50_000 {
        BASE + 170
    } else if bytes < 500_000 {
        BASE + 765
    } else {
        BASE + 1_105
    }
}

/// Sum dedicated vision tokens for workspace image paths (stat each file).
pub fn estimate_vision_tokens(
    image_paths: &[String],
    workspace_root: &Path,
) -> Result<u32, AdeError> {
    if image_paths.is_empty() {
        return Ok(0);
    }
    let mut total = 0u32;
    for raw in image_paths.iter().take(MAX_VISION_IMAGES) {
        let path = resolve_image_path(workspace_root, raw)?;
        let meta = std::fs::metadata(&path)
            .map_err(|error| AdeError::Config(format!("stat {}: {error}", path.display())))?;
        total = total.saturating_add(estimate_image_tokens_for_bytes(meta.len()));
    }
    Ok(total)
}

/// Conservative allowlist: VL-named models + common multimodal families.
/// Free text-only Zen presets (deepseek-*-free, big-pickle, …) return false.
///
/// When `profile_vision` is `Some`, it wins (model-profile `vision` flag).
pub fn model_supports_vision(model: &str) -> bool {
    model_supports_vision_ex(model, None)
}

pub fn model_supports_vision_ex(model: &str, profile_vision: Option<bool>) -> bool {
    if let Some(forced) = profile_vision {
        return forced;
    }
    let m = model.trim().to_ascii_lowercase();
    if m.is_empty() {
        return false;
    }
    if matches!(
        m.as_str(),
        "big-pickle" | "auto" | "compound" | "deepseek-v4-flash" | "deepseek-v4-flash-free"
    ) {
        return false;
    }
    if m.ends_with("-free") && !(m.contains("vl") || m.contains("vision")) {
        return false;
    }
    if m.contains("coder") && !(m.contains("vl") || m.contains("vision")) {
        return false;
    }
    if m.contains("nano") && !(m.contains("vl") || m.contains("vision")) {
        return false;
    }
    m.contains("vl")
        || m.contains("vision")
        || m.contains("moondream")
        || m.contains("gpt-4o")
        || m.contains("gpt-4.1")
        || m.contains("gpt-5")
        || m.starts_with("o4")
        || m.contains("claude-")
        || m.contains("gemini")
}

fn mime_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn resolve_image_path(workspace_root: &Path, raw: &str) -> Result<PathBuf, AdeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AdeError::Config("empty image path".into()));
    }
    let candidate = PathBuf::from(trimmed);
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        workspace_root.join(&candidate)
    };
    let canonical = absolute
        .canonicalize()
        .map_err(|error| AdeError::Config(format!("image path {}: {error}", absolute.display())))?;
    if !canonical.is_file() {
        return Err(AdeError::Config(format!(
            "image path is not a file: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn read_as_data_url(path: &Path) -> Result<String, AdeError> {
    let meta = std::fs::metadata(path)
        .map_err(|error| AdeError::Config(format!("stat {}: {error}", path.display())))?;
    if meta.len() > MAX_VISION_BYTES {
        return Err(AdeError::Config(format!(
            "image too large for vision ({} bytes; max {MAX_VISION_BYTES}): {}",
            meta.len(),
            path.display()
        )));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| AdeError::Config(format!("read {}: {error}", path.display())))?;
    let mime = mime_for_path(path);
    let b64 = base64_encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Build OpenAI-compatible user `content` (string or parts array).
/// When `image_paths` is non-empty and the model lacks vision → `vision_required` error.
pub fn user_message_content(
    prompt: &str,
    image_paths: &[String],
    model: &str,
    workspace_root: &Path,
) -> Result<Value, AdeError> {
    user_message_content_ex(prompt, image_paths, model, workspace_root, None)
}

pub fn user_message_content_ex(
    prompt: &str,
    image_paths: &[String],
    model: &str,
    workspace_root: &Path,
    profile_vision: Option<bool>,
) -> Result<Value, AdeError> {
    if image_paths.is_empty() {
        return Ok(Value::String(prompt.to_string()));
    }
    if !model_supports_vision_ex(model, profile_vision) {
        return Err(AdeError::Config(format!(
            "vision_required: model `{model}` does not support images. Switch to a vision-capable model (e.g. Claude, GPT-4.1, or a *-vl* FreeLLM model)."
        )));
    }
    if image_paths.len() > MAX_VISION_IMAGES {
        return Err(AdeError::Config(format!(
            "too many images for one turn ({} > {MAX_VISION_IMAGES})",
            image_paths.len()
        )));
    }
    let mut parts = vec![json!({ "type": "text", "text": prompt })];
    for raw in image_paths {
        let path = resolve_image_path(workspace_root, raw)?;
        let url = read_as_data_url(&path)?;
        parts.push(json!({
            "type": "image_url",
            "image_url": { "url": url }
        }));
    }
    Ok(Value::Array(parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_only_models_reject_vision() {
        assert!(!model_supports_vision("deepseek-v4-flash-free"));
        assert!(!model_supports_vision("big-pickle"));
        assert!(!model_supports_vision("qwen3-coder-480b"));
        assert!(!model_supports_vision("gpt-5.4-nano"));
    }

    #[test]
    fn vision_models_accept() {
        assert!(model_supports_vision("claude-haiku-4-5"));
        assert!(model_supports_vision("gpt-4.1-mini"));
        assert!(model_supports_vision("qwen2.5-vl-72b"));
        assert!(model_supports_vision("command-a-vision"));
    }

    #[test]
    fn profile_vision_flag_overrides_heuristic() {
        assert!(!model_supports_vision_ex("claude-haiku-4-5", Some(false)));
        assert!(model_supports_vision_ex("big-pickle", Some(true)));
    }

    #[test]
    fn image_token_bands_are_stable() {
        assert!(estimate_image_tokens_for_bytes(10_000) < estimate_image_tokens_for_bytes(200_000));
        assert!(
            estimate_image_tokens_for_bytes(200_000) < estimate_image_tokens_for_bytes(2_000_000)
        );
    }

    #[test]
    fn empty_images_stay_string() {
        let content = user_message_content("hi", &[], "big-pickle", Path::new(".")).unwrap();
        assert_eq!(content, Value::String("hi".into()));
    }

    #[test]
    fn images_on_text_model_error() {
        let err = user_message_content(
            "what is this?",
            &["shot.png".into()],
            "deepseek-v4-flash-free",
            Path::new("."),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("vision_required"));
    }

    #[test]
    fn builds_data_url_parts() {
        let root = std::env::temp_dir().join(format!("ade-vision-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("dot.png");
        // 1x1 PNG
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0xD4, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        std::fs::write(&path, png).unwrap();
        let content =
            user_message_content("describe", &["dot.png".into()], "claude-haiku-4-5", &root)
                .unwrap();
        let arr = content.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[1]["type"], "image_url");
        let url = arr[1]["image_url"]["url"].as_str().unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        let _ = std::fs::remove_dir_all(root);
    }
}
