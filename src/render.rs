use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

pub fn html_to_plain_text(content_html: &str) -> String {
    let processed = content_html
        .replace("<br />", "\n")
        .replace("<br>", "\n")
        .replace("</p>", "\n\n");
    strip_html(&processed).trim().to_string()
}

pub fn strip_html(raw_html: &str) -> String {
    static TAGS: OnceLock<Regex> = OnceLock::new();
    let tags = TAGS.get_or_init(|| Regex::new(r"<[^<]+?>").expect("tag regex"));
    unescape(&tags.replace_all(raw_html, "").to_string())
}

pub fn get_quoted_status(status: &Value) -> Option<&Value> {
    let quote = status.get("quote")?;
    if quote.is_null() {
        return None;
    }
    quote.get("quoted_status").filter(|value| !value.is_null())
}

pub fn quote_url_for_status(quoted_status: Option<&Value>) -> String {
    let Some(quoted) = quoted_status else {
        return String::new();
    };
    text(quoted, "url")
        .or_else(|| text(quoted, "uri"))
        .unwrap_or_default()
}

pub fn strip_quote_url_from_text(content: &str, quoted_url: &str) -> String {
    if quoted_url.is_empty() {
        return content.trim().to_string();
    }
    let pattern = format!(r"^(?:RE|QT):\s*{}\s*", regex::escape(quoted_url));
    let stripped = Regex::new(&pattern)
        .ok()
        .map(|regex| regex.replace(content, "").to_string())
        .unwrap_or_else(|| content.to_string());
    stripped
        .trim()
        .strip_suffix(quoted_url)
        .unwrap_or(stripped.trim())
        .trim()
        .to_string()
}

pub fn strip_quote_url_from_html(content_html: &str, quoted_url: &str) -> String {
    if quoted_url.is_empty() {
        return content_html.to_string();
    }
    let plain = html_to_plain_text(content_html);
    let cleaned_plain = strip_quote_url_from_text(&plain, quoted_url);
    if cleaned_plain == plain {
        return content_html.to_string();
    }
    let mut cleaned = Regex::new(r"^\s*(<p\b[^>]*>\s*)?(?:RE|QT):\s*")
        .expect("prefix")
        .replace(content_html, "$1")
        .to_string();
    let link_pattern = format!(
        r#"<a\b[^>]*\bhref=["']{}["'][^>]*>.*?</a>"#,
        regex::escape(quoted_url)
    );
    if let Ok(regex) = Regex::new(&link_pattern) {
        cleaned = regex.replace(&cleaned, "").to_string();
    }
    for url_text in [
        quoted_url.to_string(),
        escape_html(quoted_url),
        escape_html(quoted_url).replace('\'', "&#x27;"),
    ] {
        if let Some(index) = cleaned.find(&url_text) {
            cleaned.replace_range(index..index + url_text.len(), "");
        }
    }
    let cleaned = Regex::new(r"<p\b[^>]*>\s*</p>")
        .expect("empty p")
        .replace_all(&cleaned, "")
        .to_string();
    Regex::new(r"\s+</p>")
        .expect("space p")
        .replace_all(&cleaned, "</p>")
        .trim()
        .to_string()
}

pub fn account_label(account: &Value) -> String {
    let display = text(account, "display_name")
        .or_else(|| text(account, "username"))
        .unwrap_or_else(|| "Unknown".to_string());
    match text(account, "acct") {
        Some(acct) if !acct.is_empty() => format!("{display} (@{acct})"),
        _ => display,
    }
}

pub fn build_status_body_html(status: &Value) -> String {
    let quoted = get_quoted_status(status);
    let quoted_url = quote_url_for_status(quoted);
    let content_html = strip_quote_url_from_html(status.get("content").and_then(Value::as_str).unwrap_or(""), &quoted_url);
    let mut parts = vec![
        "<article class=\"post-content\" aria-labelledby=\"post-content-heading\">".to_string(),
        "<h1 id=\"post-content-heading\">Post content</h1>".to_string(),
    ];
    if let Some(spoiler) = text(status, "spoiler_text") {
        parts.push(format!(
            "<p class=\"content-warning\"><strong>Content warning:</strong> {}</p>",
            escape_html(&spoiler)
        ));
    }
    parts.push(if content_html.is_empty() {
        "<p></p>".to_string()
    } else {
        content_html
    });
    parts.push("</article>".to_string());
    if let Some(quoted_status) = quoted {
        let quoted_account = quoted_status.get("account").cloned().unwrap_or(Value::Null);
        let quoted_label = escape_html(&account_label(&quoted_account));
        let quoted_content = quoted_status
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("");
        parts.push("<article class=\"quoted-post\" aria-labelledby=\"quoted-post-heading\">".to_string());
        parts.push(format!(
            "<h2 id=\"quoted-post-heading\">Quoted post by {quoted_label}</h2>"
        ));
        parts.push(if quoted_content.is_empty() {
            "<p></p>".to_string()
        } else {
            quoted_content.to_string()
        });
        parts.push("</article>".to_string());
    }
    parts.join("\n")
}

pub fn status_plain_text(status: &Value) -> String {
    let content = html_to_plain_text(status.get("content").and_then(Value::as_str).unwrap_or(""));
    let quoted_url = quote_url_for_status(get_quoted_status(status));
    strip_quote_url_from_text(&content, &quoted_url)
}

pub fn text<'a>(value: &'a Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(|item| item.to_string()).filter(|item| !item.is_empty())
}

pub fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

pub fn links_in_status(status: &Value) -> Vec<String> {
    static HREF: OnceLock<Regex> = OnceLock::new();
    static URL: OnceLock<Regex> = OnceLock::new();
    let href = HREF.get_or_init(|| Regex::new(r#"(?i)<a\s[^>]*?href="([^"]*)""#).expect("href"));
    let url = URL.get_or_init(|| Regex::new(r#"(?:https?://|www\.)[^\s<>"]+"#).expect("url"));
    let content = status.get("content").and_then(Value::as_str).unwrap_or("");
    let mut raw = Vec::new();
    for capture in href.captures_iter(content) {
        raw.push(capture[1].to_string());
    }
    let plain = strip_html(content);
    for capture in url.find_iter(&plain) {
        raw.push(capture.as_str().to_string());
    }
    let mut cleaned = Vec::new();
    for link in raw {
        let link = link.trim_end_matches(['.', ',', ';', ':', '!', '?']).to_string();
        if link.contains('.') && !link.ends_with('.') && !cleaned.contains(&link) {
            cleaned.push(link);
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preserves_post_html_links() {
        let status = json!({
            "content": "<p>Hello <a href=\"https://example.com\">example</a></p>",
            "account": {"display_name": "Alex", "acct": "alex@example.com"}
        });
        let body = build_status_body_html(&status);
        assert!(body.contains("<a href=\"https://example.com\">example</a>"));
        assert!(body.contains("<h1"));
    }

    #[test]
    fn renders_quoted_status_as_separate_article() {
        let status = json!({
            "content": "<p>Main post</p>",
            "quote": {
                "quoted_status": {
                    "content": "<p>Quoted body</p>",
                    "account": {"display_name": "Quoter", "acct": "quoter@example.com"}
                }
            }
        });
        let body = build_status_body_html(&status);
        assert!(body.contains("Main post"));
        assert!(body.contains("Quoted post by Quoter (@quoter@example.com)"));
        assert!(body.contains("Quoted body"));
    }

    #[test]
    fn escapes_app_generated_labels() {
        let status = json!({
            "content": "<p>Post</p>",
            "spoiler_text": "<script>alert(\"x\")</script>",
            "quote": {
                "quoted_status": {
                    "content": "<p>Quoted</p>",
                    "account": {"display_name": "<b>Name</b>", "acct": "user@example.com"}
                }
            }
        });
        let body = build_status_body_html(&status);
        assert!(body.contains("&lt;script&gt;"));
        assert!(body.contains("&lt;b&gt;Name&lt;/b&gt;"));
        assert!(!body.contains("<script>alert(\"x\")</script>"));
    }

    #[test]
    fn removes_server_prepended_quote_url() {
        let quoted_url = "https://social.example/@quoted/123";
        let status = json!({
            "content": format!("<p>RE: <a href=\"{quoted_url}\">{quoted_url}</a> My response</p>"),
            "quote": {
                "quoted_status": {
                    "url": quoted_url,
                    "content": "<p>Quoted</p>",
                    "account": {"display_name": "Quoted", "acct": "quoted@example.com"}
                }
            }
        });
        let body = build_status_body_html(&status);
        let main = body.split("<article class=\"quoted-post\"").next().unwrap();
        assert!(main.contains("My response"));
        assert!(!main.contains(quoted_url));
    }

    #[test]
    fn plain_text_matches_copy_and_edit_needs() {
        let status = json!({"content": "<p>First<br />Second</p><p>Third</p>"});
        assert_eq!(status_plain_text(&status), "First\nSecond\n\nThird");
    }
}
