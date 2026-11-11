use crate::render::{html_to_plain_text, strip_quote_url_from_text};
use crate::timeutil::relative_time;
use serde_json::Value;

pub fn source_status(status: &Value) -> Value {
    status
        .get("reblog")
        .filter(|value| !value.is_null())
        .cloned()
        .unwrap_or_else(|| status.clone())
}

pub fn display_name(account: &Value) -> String {
    account
        .get("display_name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| account.get("username").and_then(Value::as_str))
        .unwrap_or("Unknown")
        .to_string()
}

pub fn app_name(status: &Value) -> String {
    match status.get("application") {
        Some(Value::Object(app)) => app
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_string(),
        Some(Value::String(name)) => name.clone(),
        _ => "Unknown".to_string(),
    }
}

fn avatar_url(account: &Value, show_avatars: bool) -> Option<String> {
    if !show_avatars {
        return None;
    }
    account
        .get("avatar_static")
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty())
        .or_else(|| account.get("avatar").and_then(Value::as_str).filter(|url| !url.is_empty()))
        .map(str::to_string)
}

pub fn row_from_status(status: &Value, show_avatars: bool) -> Option<(Vec<String>, Option<String>)> {
    if status.is_null() {
        return None;
    }
    let is_boost = status.get("reblog").is_some_and(|value| !value.is_null());
    let source = source_status(status);
    let account = status.get("account")?;
    let avatar = avatar_url(account, show_avatars);
    let author = display_name(account);
    let content = html_to_plain_text(source.get("content").and_then(Value::as_str).unwrap_or(""));
    let mut content_cell = if is_boost {
        let original = source.get("account").cloned().unwrap_or(Value::Null);
        let body = source
            .get("spoiler_text")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(|spoiler| format!("CW: {spoiler}"))
            .unwrap_or(content);
        format!(
            "boosting {} (@{}): {body}",
            display_name(&original),
            original.get("acct").and_then(Value::as_str).unwrap_or("")
        )
    } else if let Some(spoiler) = status.get("spoiler_text").and_then(Value::as_str).filter(|value| !value.is_empty()) {
        format!("CW: {spoiler}")
    } else {
        content
    };
    if let Some(quoted) = source.get("quote").and_then(|quote| quote.get("quoted_status")).filter(|value| !value.is_null()) {
        let quoted_account = quoted.get("account").cloned().unwrap_or(Value::Null);
        let quoted_content = html_to_plain_text(quoted.get("content").and_then(Value::as_str).unwrap_or(""));
        let quoted_url = quoted
            .get("url")
            .and_then(Value::as_str)
            .or_else(|| quoted.get("uri").and_then(Value::as_str))
            .unwrap_or("");
        content_cell = strip_quote_url_from_text(&content_cell, quoted_url);
        content_cell = format!(
            "quoting {} (@{}): \"{quoted_content}\". {author} added \"{content_cell}\"",
            display_name(&quoted_account),
            quoted_account.get("acct").and_then(Value::as_str).unwrap_or("")
        );
    }
    if source.get("poll").is_some_and(|value| !value.is_null()) {
        content_cell.push_str(" [Poll]");
    }
    let created = source.get("created_at").and_then(Value::as_str).unwrap_or("");
    Some((
        vec![author, content_cell, relative_time(created), app_name(&source)],
        avatar,
    ))
}

pub fn notification_text(notification: &Value) -> String {
    let kind = notification.get("type").and_then(Value::as_str).unwrap_or("");
    if let Some(fallback) = notification.get("fallback").filter(|value| value.is_object()) {
        let parts = ["title", "summary", "details"]
            .iter()
            .filter_map(|key| fallback.get(*key).and_then(Value::as_str))
            .map(crate::render::strip_html)
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>();
        if !parts.is_empty() {
            return parts.join(": ");
        }
        return format!("Unsupported notification: {kind}");
    }
    let account = notification.get("account").cloned().unwrap_or(Value::Null);
    let user = display_name(&account);
    let status = notification.get("status").filter(|value| !value.is_null());
    let content = status
        .and_then(|status| status.get("content").and_then(Value::as_str))
        .map(html_to_plain_text)
        .unwrap_or_default();
    match kind {
        "favourite" if status.is_some() => format!("{user} favourited your post: {content}"),
        "favourite" => format!("{user} favourited a post that is no longer available."),
        "reblog" if status.is_some() => format!("{user} boosted your post: {content}"),
        "reblog" => format!("{user} boosted a post that is no longer available."),
        "mention" if status.is_some() => format!("{user} mentioned you: {content}"),
        "mention" => format!("{user} mentioned you in a post that is no longer available."),
        "follow" => format!("{user} followed you."),
        "follow_request" => format!("{user} requested to follow you."),
        "status" if status.is_some() => format!("{user} posted: {content}"),
        "status" => format!("{user} posted a status that is no longer available."),
        "poll" if status.and_then(|item| item.get("poll")).and_then(|poll| poll.get("expired")).and_then(Value::as_bool).unwrap_or(false) => {
            format!("Poll ended in {user}'s post: {content}")
        }
        "poll" => format!("A poll notification from {user} is no longer available."),
        "update" if status.is_some() => format!("{user}'s post you interacted with was edited: {content}"),
        "update" => format!("{user}'s edited post is no longer available."),
        "quote" if status.is_some() => format!("{user} quoted your post: {content}"),
        "quote" => format!("{user} quoted a post that is no longer available."),
        "quoted_update" if status.is_some() => format!("{user} edited a post quoted by your post: {content}"),
        "quoted_update" => format!("{user} edited a post you quoted."),
        "added_to_collection" => format!("{user} added you to {}.", collection_name(notification)),
        "collection_update" => format!("{user} updated {}, which features you.", collection_name(notification)),
        _ => format!("{user}: {kind}"),
    }
}

fn collection_name(notification: &Value) -> String {
    notification
        .pointer("/collection/name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .unwrap_or("an unnamed collection")
        .to_string()
}

pub fn row_from_notification(notification: &Value, show_avatars: bool) -> (Vec<String>, Option<String>) {
    let account = notification.get("account").cloned().unwrap_or(Value::Null);
    let avatar = avatar_url(&account, show_avatars);
    let display = notification_text(notification);
    let (author, content) = display.split_once(':').map(|(author, content)| (author.to_string(), content.trim().to_string())).unwrap_or((display, String::new()));
    let created = notification.get("created_at").and_then(Value::as_str).unwrap_or("");
    let status = notification.get("status").cloned().unwrap_or(Value::Null);
    (vec![author, content, relative_time(created), app_name(&status)], avatar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quote_notifications_name_the_quoter() {
        let account = json!({"display_name": "Sam", "acct": "sam"});
        let quoted = json!({"type": "quote", "account": account, "status": {"content": "<p>Nice</p>"}});
        assert_eq!(notification_text(&quoted), "Sam quoted your post: Nice");
        let gone = json!({"type": "quoted_update", "account": account});
        assert_eq!(notification_text(&gone), "Sam edited a post you quoted.");
    }

    #[test]
    fn collection_notifications_name_the_collection() {
        let account = json!({"display_name": "Sam"});
        let named = json!({"type": "added_to_collection", "account": account, "collection": {"name": "Friends"}});
        assert_eq!(notification_text(&named), "Sam added you to Friends.");
        let unnamed = json!({"type": "collection_update", "account": account, "collection": {}});
        assert_eq!(notification_text(&unnamed), "Sam updated an unnamed collection, which features you.");
    }
}
