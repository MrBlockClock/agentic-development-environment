//! Built-in read-only web access for agent turns (no MCP required).

use ade_core::error::AdeError;
use serde_json::{json, Value};
use std::time::Duration;

const MAX_BODY_CHARS: usize = 24_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const USER_AGENT: &str = "ADE-Desktop/0.1 (+https://github.com/MrBlockClock/agentic-development-environment; read-only web tool)";

fn http_client() -> Result<reqwest::Client, AdeError> {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| AdeError::Provider(format!("web client: {error}")))
}

fn validate_http_url(raw: &str) -> Result<reqwest::Url, AdeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AdeError::Config("url is required".into()));
    }
    let url = reqwest::Url::parse(trimmed)
        .map_err(|error| AdeError::Config(format!("invalid url: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AdeError::Config(
            "only http and https urls are allowed".into(),
        ));
    }
    if url.host_str().is_none() {
        return Err(AdeError::Config("url must include a host".into()));
    }
    Ok(url)
}

fn truncate_chars(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }
    let truncated: String = input.chars().take(max).collect();
    format!("{truncated}\n\n…[truncated at {max} chars]")
}

/// Collapse HTML/script noise into readable plain text for the model.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(MAX_BODY_CHARS * 2));
    let mut in_tag = false;
    let mut in_script = false;
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !in_tag && lower[i..].starts_with("<script") {
            in_script = true;
        }
        if in_script && lower[i..].starts_with("</script") {
            in_script = false;
        }
        let ch = bytes[i] as char;
        if ch == '<' {
            in_tag = true;
            i += 1;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            i += 1;
            continue;
        }
        if !in_tag && !in_script {
            out.push(if ch.is_ascii_whitespace() { ' ' } else { ch });
        }
        i += 1;
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&collapsed, MAX_BODY_CHARS)
}

pub async fn web_fetch(url: &str) -> Result<String, AdeError> {
    let parsed = validate_http_url(url)?;
    let client = http_client()?;
    let response = client
        .get(parsed.clone())
        .send()
        .await
        .map_err(|error| AdeError::Provider(format!("web_fetch failed: {error}")))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response
        .text()
        .await
        .map_err(|error| AdeError::Provider(format!("web_fetch body: {error}")))?;
    let is_html = content_type.contains("html")
        || body.trim_start().starts_with("<!DOCTYPE")
        || body.trim_start().starts_with("<html");
    let content = if is_html {
        html_to_text(&body)
    } else {
        truncate_chars(&body, MAX_BODY_CHARS)
    };
    Ok(format!(
        "url: {parsed}\nstatus: {status}\ncontent-type: {content_type}\n\n{content}"
    ))
}

pub async fn web_search(query: &str) -> Result<String, AdeError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(AdeError::Config("query is required".into()));
    }
    let client = http_client()?;
    let response = client
        .get("https://api.duckduckgo.com/")
        .query(&[
            ("q", query),
            ("format", "json"),
            ("no_html", "1"),
            ("skip_disambig", "1"),
        ])
        .send()
        .await
        .map_err(|error| AdeError::Provider(format!("web_search failed: {error}")))?;
    if !response.status().is_success() {
        return Err(AdeError::Provider(format!(
            "web_search HTTP {}",
            response.status()
        )));
    }
    let payload: Value = response
        .json()
        .await
        .map_err(|error| AdeError::Provider(format!("web_search json: {error}")))?;

    let mut lines = vec![format!("query: {query}")];
    if let Some(heading) = payload.get("Heading").and_then(Value::as_str) {
        if !heading.is_empty() {
            lines.push(format!("heading: {heading}"));
        }
    }
    if let Some(abstract_text) = payload.get("AbstractText").and_then(Value::as_str) {
        if !abstract_text.is_empty() {
            lines.push(format!("abstract: {abstract_text}"));
        }
    }
    if let Some(abstract_url) = payload.get("AbstractURL").and_then(Value::as_str) {
        if !abstract_url.is_empty() {
            lines.push(format!("abstract_url: {abstract_url}"));
        }
    }
    if let Some(answer) = payload.get("Answer").and_then(Value::as_str) {
        if !answer.is_empty() {
            lines.push(format!("answer: {answer}"));
        }
    }

    let mut related = Vec::new();
    if let Some(topics) = payload.get("RelatedTopics").and_then(Value::as_array) {
        for topic in topics.iter().take(8) {
            if let Some(text) = topic.get("Text").and_then(Value::as_str) {
                let url = topic.get("FirstURL").and_then(Value::as_str).unwrap_or("");
                related.push(json!({ "text": text, "url": url }));
            } else if let Some(nested) = topic.get("Topics").and_then(Value::as_array) {
                for item in nested.iter().take(3) {
                    if let Some(text) = item.get("Text").and_then(Value::as_str) {
                        let url = item.get("FirstURL").and_then(Value::as_str).unwrap_or("");
                        related.push(json!({ "text": text, "url": url }));
                    }
                }
            }
        }
    }
    if !related.is_empty() {
        lines.push(format!(
            "related:\n{}",
            serde_json::to_string_pretty(&related).unwrap_or_default()
        ));
    }
    if lines.len() == 1 {
        lines.push(
            "No Instant Answer results. Try web_fetch on a specific documentation URL.".into(),
        );
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_schemes() {
        assert!(validate_http_url("file:///etc/passwd").is_err());
        assert!(validate_http_url("ftp://example.com").is_err());
    }

    #[test]
    fn accepts_https() {
        assert!(validate_http_url("https://example.com/docs").is_ok());
    }

    #[test]
    fn strips_script_tags() {
        let text = html_to_text("<html><script>alert(1)</script><p>Hello   world</p></html>");
        assert!(text.contains("Hello world"));
        assert!(!text.contains("alert"));
    }
}
