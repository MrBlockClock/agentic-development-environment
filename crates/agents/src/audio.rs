//! Opt-in audio transcription → inbox markdown (Whisper-class API or local cmd).
//!
//! Desktop surfaces this behind Debug/Advanced. Network path uses OpenAI-compatible
//! `POST {base}/audio/transcriptions` (Groq / OpenAI). Local path: `ADE_WHISPER_CMD`
//! with `{path}` replaced by the absolute file path; stdout is the transcript.

use ade_core::error::AdeError;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Cap transcript body so inbox markdown stays harness-friendly.
pub const MAX_TRANSCRIPT_CHARS: usize = 48_000;
/// Refuse oversized uploads (Whisper APIs are typically ≤25 MiB).
pub const MAX_AUDIO_BYTES: u64 = 25 * 1024 * 1024;

const AUDIO_EXT: &[&str] = &[
    "mp3", "wav", "m4a", "ogg", "flac", "webm", "mpga", "mpeg", "mp4",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioKind {
    Mp3,
    Wav,
    M4a,
    Ogg,
    Flac,
    Webm,
    Other,
}

impl AudioKind {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !AUDIO_EXT.contains(&ext.as_str()) {
            return None;
        }
        Some(match ext.as_str() {
            "mp3" | "mpga" | "mpeg" => Self::Mp3,
            "wav" => Self::Wav,
            "m4a" => Self::M4a,
            "ogg" => Self::Ogg,
            "flac" => Self::Flac,
            "webm" | "mp4" => Self::Webm,
            _ => Self::Other,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::M4a => "m4a",
            Self::Ogg => "ogg",
            Self::Flac => "flac",
            Self::Webm => "webm",
            Self::Other => "audio",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Mp3 => "audio/mpeg",
            Self::Wav => "audio/wav",
            Self::M4a => "audio/mp4",
            Self::Ogg => "audio/ogg",
            Self::Flac => "audio/flac",
            Self::Webm => "audio/webm",
            Self::Other => "application/octet-stream",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TranscribeResult {
    pub kind: AudioKind,
    pub text: String,
    pub truncated: bool,
    pub backend: String,
}

#[derive(Debug, Clone)]
pub struct TranscribeApiOpts {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub provider_label: String,
}

/// True when `ADE_WHISPER_CMD` is set (local whisper-class binary/script).
pub fn local_whisper_cmd_configured() -> bool {
    std::env::var("ADE_WHISPER_CMD")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/// Validate path/extension/size before any network or spawn.
pub fn validate_audio_file(path: &Path) -> Result<AudioKind, AdeError> {
    let kind = AudioKind::from_path(path).ok_or_else(|| {
        AdeError::Config(format!(
            "not an audio transcribe target (mp3/wav/m4a/ogg/flac/webm/mp4): {}",
            path.display()
        ))
    })?;
    if !path.is_file() {
        return Err(AdeError::Config(format!(
            "audio file not found: {}",
            path.display()
        )));
    }
    let meta = std::fs::metadata(path).map_err(|error| {
        AdeError::Config(format!("stat audio {}: {error}", path.display()))
    })?;
    if meta.len() > MAX_AUDIO_BYTES {
        return Err(AdeError::Config(format!(
            "audio too large ({} bytes; max {}): {}",
            meta.len(),
            MAX_AUDIO_BYTES,
            path.display()
        )));
    }
    if meta.len() == 0 {
        return Err(AdeError::Config(format!(
            "audio file is empty: {}",
            path.display()
        )));
    }
    Ok(kind)
}

/// Transcribe via local `ADE_WHISPER_CMD` (sync; no network).
/// Command template may include `{path}`; otherwise the absolute path is appended as one arg.
pub fn transcribe_local(path: &Path) -> Result<TranscribeResult, AdeError> {
    let kind = validate_audio_file(path)?;
    let template = std::env::var("ADE_WHISPER_CMD").map_err(|_| {
        AdeError::Config("ADE_WHISPER_CMD not set — use API transcribe or set a local whisper command".into())
    })?;
    let template = template.trim();
    if template.is_empty() {
        return Err(AdeError::Config("ADE_WHISPER_CMD is empty".into()));
    }
    let abs = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());
    let abs_str = abs.to_string_lossy();
    let (program, args) = if template.contains("{path}") {
        let rendered = template.replace("{path}", &abs_str);
        split_cmd_line(&rendered)?
    } else {
        let (program, mut args) = split_cmd_line(template)?;
        args.push(abs_str.into_owned());
        (program, args)
    };
    let output = Command::new(&program)
        .args(&args)
        .output()
        .map_err(|error| {
            AdeError::Config(format!(
                "spawn ADE_WHISPER_CMD ({program}): {error}"
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AdeError::Config(format!(
            "ADE_WHISPER_CMD failed (status {}): {}",
            output.status,
            stderr.trim()
        )));
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return Err(AdeError::Config(
            "ADE_WHISPER_CMD returned empty transcript".into(),
        ));
    }
    let (text, truncated) = truncate_body(raw);
    Ok(TranscribeResult {
        kind,
        text,
        truncated,
        backend: "local:ADE_WHISPER_CMD".into(),
    })
}

/// Transcribe via OpenAI-compatible `/audio/transcriptions`.
pub async fn transcribe_api(
    path: &Path,
    opts: &TranscribeApiOpts,
) -> Result<TranscribeResult, AdeError> {
    let kind = validate_audio_file(path)?;
    let bytes = std::fs::read(path).map_err(|error| {
        AdeError::Config(format!("read audio {}: {error}", path.display()))
    })?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.bin")
        .to_string();
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(kind.mime())
        .map_err(|error| AdeError::Config(format!("audio multipart: {error}")))?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", opts.model.clone())
        .text("response_format", "json");
    let url = format!(
        "{}/audio/transcriptions",
        opts.base_url.trim_end_matches('/')
    );
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .bearer_auth(opts.api_key.trim())
        .multipart(form)
        .send()
        .await
        .map_err(|error| AdeError::Provider(format!("transcribe request: {error}")))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| String::new());
    if !status.is_success() {
        return Err(AdeError::Provider(format!(
            "transcribe {} → {url} HTTP {status}: {}",
            opts.provider_label,
            body.chars().take(400).collect::<String>()
        )));
    }
    let parsed: WhisperJson = serde_json::from_str(&body).map_err(|error| {
        AdeError::Provider(format!("transcribe JSON parse: {error}; body={}", body.chars().take(200).collect::<String>()))
    })?;
    let raw = parsed.text.trim().to_string();
    if raw.is_empty() {
        return Err(AdeError::Provider(
            "transcribe returned empty text".into(),
        ));
    }
    let (text, truncated) = truncate_body(raw);
    Ok(TranscribeResult {
        kind,
        text,
        truncated,
        backend: format!("api:{}:{}", opts.provider_label, opts.model),
    })
}

#[derive(Debug, Deserialize)]
struct WhisperJson {
    text: String,
}

/// Default Whisper model for a vault provider id.
pub fn default_whisper_model(provider: &str) -> &'static str {
    match provider.trim().to_ascii_lowercase().as_str() {
        "groq" => "whisper-large-v3-turbo",
        "openai" => "whisper-1",
        _ => "whisper-1",
    }
}

/// Default OpenAI-compatible base URL for Whisper.
pub fn default_whisper_base_url(provider: &str) -> Option<&'static str> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "groq" => Some("https://api.groq.com/openai/v1"),
        "openai" => Some("https://api.openai.com/v1"),
        _ => None,
    }
}

/// Preferred vault providers for Auto (Groq first — free Whisper tier).
pub const WHISPER_PROVIDER_PREFERENCE: &[&str] = &["groq", "openai"];

pub fn format_transcript_markdown(
    source_label: &str,
    source_path: &str,
    result: &TranscribeResult,
) -> String {
    format!(
        "# Audio transcript\n\n\
Source: {source_label}\n\
Path: {source_path}\n\
Kind: {}\n\
Backend: {}{}\n\n\
---\n\n\
{}\n",
        result.kind.label(),
        result.backend,
        if result.truncated {
            " · body truncated"
        } else {
            ""
        },
        result.text
    )
}

fn truncate_body(body: String) -> (String, bool) {
    let truncated = body.chars().count() > MAX_TRANSCRIPT_CHARS;
    if !truncated {
        return (body, false);
    }
    let mut cut: String = body.chars().take(MAX_TRANSCRIPT_CHARS).collect();
    cut.push_str("\n\n…[truncated]");
    (cut, true)
}

/// Minimal shell-ish split for `ADE_WHISPER_CMD` (no nested quotes gymnastics).
fn split_cmd_line(line: &str) -> Result<(String, Vec<String>), AdeError> {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    parts.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    if parts.is_empty() {
        return Err(AdeError::Config("ADE_WHISPER_CMD has no program".into()));
    }
    let program = parts.remove(0);
    Ok((program, parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_audio_extension() {
        let root = std::env::temp_dir().join(format!("ade-audio-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("note.txt");
        std::fs::write(&path, b"hello").unwrap();
        let err = validate_audio_file(&path).unwrap_err().to_string();
        assert!(err.contains("not an audio transcribe target"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn accepts_wav_extension() {
        let root = std::env::temp_dir().join(format!("ade-audio-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("clip.wav");
        // Minimal RIFF header + bytes (not a valid decode — validate is extension/size only).
        std::fs::write(&path, b"RIFF....WAVEfmt ").unwrap();
        let kind = validate_audio_file(&path).unwrap();
        assert_eq!(kind, AudioKind::Wav);
        let md = format_transcript_markdown(
            "clip.wav",
            "clip.wav",
            &TranscribeResult {
                kind,
                text: "hello".into(),
                truncated: false,
                backend: "test".into(),
            },
        );
        assert!(md.contains("Audio transcript"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn local_cmd_echo_path_transcript() {
        let root = std::env::temp_dir().join(format!("ade-audio-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("clip.mp3");
        std::fs::write(&path, b"ID3fakeaudio").unwrap();
        let cmd = if cfg!(windows) {
            r#"powershell -NoProfile -Command "Write-Output 'ADE audio gold transcript'""#
        } else {
            r#"sh -c "echo ADE audio gold transcript""#
        };
        std::env::set_var("ADE_WHISPER_CMD", cmd);
        let result = transcribe_local(&path).unwrap();
        std::env::remove_var("ADE_WHISPER_CMD");
        assert!(result.text.contains("ADE audio gold transcript"));
        assert!(result.backend.starts_with("local:"));
        let _ = std::fs::remove_dir_all(root);
    }
}
