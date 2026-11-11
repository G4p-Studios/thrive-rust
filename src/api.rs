use serde_json::{Value, json};
use std::sync::Arc;

pub const SUPPORTED_NOTIFICATION_TYPES: &[&str] = &[
    "mention",
    "status",
    "reblog",
    "follow",
    "follow_request",
    "favourite",
    "poll",
    "update",
    "quote",
    "quoted_update",
    "added_to_collection",
    "collection_update",
];

pub const QUOTE_POSTS_API_VERSION: i64 = 7;
pub const COLLECTIONS_API_VERSION: i64 = 10;

#[derive(Clone)]
pub struct Mastodon {
    inner: Arc<Inner>,
}

struct Inner {
    client: reqwest::blocking::Client,
    base: String,
    token: String,
    host: String,
}

impl Mastodon {
    pub fn from_session(session: &crate::session::Session) -> Result<Self, String> {
        let base = normalize_instance(&session.instance_url)?;
        let host = base.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string();
        let client = reqwest::blocking::Client::builder()
            .user_agent("Thrive")
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                base,
                token: session.access_token.clone(),
                host,
            }),
        })
    }

    pub fn register(instance: &str) -> Result<(String, String, String), String> {
        let base = normalize_instance(instance)?;
        let client = reqwest::blocking::Client::builder()
            .user_agent("Thrive")
            .build()
            .map_err(|error| error.to_string())?;
        let response = client
            .post(format!("{base}/api/v1/apps"))
            .form(&[
                ("client_name", "Thrive"),
                ("redirect_uris", "urn:ietf:wg:oauth:2.0:oob"),
                ("scopes", "read write follow"),
            ])
            .send()
            .map_err(|error| error.to_string())?;
        let value = read_json(response)?;
        let client_id = required_text(&value, "client_id")?;
        let client_secret = required_text(&value, "client_secret")?;
        Ok((base, client_id, client_secret))
    }

    pub fn authorize_url(base: &str, client_id: &str) -> String {
        format!(
            "{base}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}",
            urlencoding::encode(client_id),
            urlencoding::encode("urn:ietf:wg:oauth:2.0:oob"),
            urlencoding::encode("read write follow")
        )
    }

    pub fn log_in(base: &str, client_id: &str, client_secret: &str, code: &str) -> Result<String, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Thrive")
            .build()
            .map_err(|error| error.to_string())?;
        let response = client
            .post(format!("{base}/oauth/token"))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("redirect_uri", "urn:ietf:wg:oauth:2.0:oob"),
                ("scope", "read write follow"),
            ])
            .send()
            .map_err(|error| error.to_string())?;
        let value = read_json(response)?;
        required_text(&value, "access_token")
    }

    pub fn stream_url(&self) -> String {
        format!(
            "wss://{}/api/v1/streaming?stream=user&access_token={}",
            self.inner.host,
            urlencoding::encode(&self.inner.token)
        )
    }

    pub fn me(&self) -> Result<Value, String> {
        self.get("/api/v1/accounts/verify_credentials", &[])
    }

    pub fn load_timeline(&self, timeline: &str, max_id: Option<&str>) -> Result<Vec<Value>, String> {
        let max_id = max_id.map(|id| ("max_id", id.to_string()));
        let mut query = vec![("limit", "40".to_string())];
        if let Some(item) = max_id {
            query.push(item);
        }
        let value = if timeline == "home" {
            self.get("/api/v1/timelines/home", &query)?
        } else if timeline == "local" {
            query.push(("local", "true".to_string()));
            self.get("/api/v1/timelines/public", &query)?
        } else if timeline == "federated" {
            self.get("/api/v1/timelines/public", &query)?
        } else if timeline == "sent" {
            let me = self.me()?;
            let id = required_text(&me, "id")?;
            let statuses = self.get(&format!("/api/v1/accounts/{id}/statuses"), &query)?;
            return Ok(as_array(statuses)
                .into_iter()
                .filter(|status| status.get("reblog").map(|value| value.is_null()).unwrap_or(true))
                .collect());
        } else if timeline == "direct_messages" {
            let conversations = self.get("/api/v1/conversations", &query)?;
            return Ok(as_array(conversations)
                .into_iter()
                .filter_map(|item| item.get("last_status").cloned())
                .filter(|status| !status.is_null())
                .collect());
        } else if timeline == "favourites" {
            self.get("/api/v1/favourites", &query)?
        } else if timeline == "bookmarks" {
            self.get("/api/v1/bookmarks", &query)?
        } else if timeline == "notifications" {
            self.notifications(&query, None)?
        } else if timeline == "mentions" {
            let notes = self.notifications(&query, Some(&["mention"]))?;
            return Ok(as_array(notes)
                .into_iter()
                .filter_map(|item| item.get("status").cloned())
                .filter(|status| !status.is_null())
                .collect());
        } else if let Some(id) = timeline.strip_prefix("user:") {
            let mut params = query;
            params.push(("exclude_direct", "true".to_string()));
            self.get(&format!("/api/v1/accounts/{id}/statuses"), &params)?
        } else if let Some(q) = timeline.strip_prefix("search:") {
            let mut params = vec![("q", q.to_string()), ("type", "statuses".to_string())];
            if let Some((_, id)) = query.iter().find(|(key, _)| *key == "max_id") {
                params.push(("max_id", id.clone()));
            }
            let result = self.get("/api/v2/search", &params)?;
            return Ok(as_array(result.get("statuses").cloned().unwrap_or(Value::Null)));
        } else if let Some(tag) = timeline.strip_prefix("hashtag:") {
            self.get(&format!("/api/v1/timelines/tag/{tag}"), &query)?
        } else if let Some(id) = timeline.strip_prefix("list:") {
            self.get(&format!("/api/v1/timelines/list/{id}"), &query)?
        } else {
            Value::Array(Vec::new())
        };
        Ok(as_array(value))
    }

    pub fn notifications(&self, extra: &[(&str, String)], types: Option<&[&str]>) -> Result<Value, String> {
        let mut pairs = Vec::new();
        for (key, value) in extra {
            pairs.push(((*key).to_string(), value.clone()));
        }
        if let Some(types) = types {
            for kind in types {
                pairs.push(("types[]".to_string(), (*kind).to_string()));
            }
        }
        for kind in SUPPORTED_NOTIFICATION_TYPES {
            pairs.push(("supported_types[]".to_string(), (*kind).to_string()));
        }
        self.get_pairs("/api/v1/notifications", &pairs)
    }

    pub fn notification_policy(&self) -> Result<Value, String> {
        self.get("/api/v2/notifications/policy", &[])
    }

    pub fn update_notification_policy(&self, policy: &Value) -> Result<Value, String> {
        let mut pairs = Vec::new();
        if let Some(object) = policy.as_object() {
            for (key, value) in object {
                if let Some(text) = value.as_str() {
                    pairs.push((key.clone(), text.to_string()));
                }
            }
        }
        self.request("PATCH", "/api/v2/notifications/policy", Body::Form(pairs))
    }

    pub fn search_v2(&self, query: &str, extra: &[(&str, String)]) -> Result<Value, String> {
        let mut pairs = vec![("q".to_string(), query.to_string())];
        for (key, value) in extra {
            pairs.push(((*key).to_string(), value.clone()));
        }
        self.get_pairs("/api/v2/search", &pairs)
    }

    pub fn status_post(&self, fields: Vec<(String, String)>) -> Result<Value, String> {
        self.request("POST", "/api/v1/statuses", Body::Form(fields))
    }

    pub fn status_action(&self, id: &str, action: &str) -> Result<Value, String> {
        let path = if action.is_empty() {
            format!("/api/v1/statuses/{id}")
        } else {
            format!("/api/v1/statuses/{id}/{action}")
        };
        let method = if action.is_empty() { "DELETE" } else { "POST" };
        self.request(method, &path, Body::Empty)
    }

    pub fn status_update(&self, id: &str, text: &str, spoiler: Option<&str>) -> Result<Value, String> {
        let mut fields = vec![("status".to_string(), text.to_string())];
        if let Some(spoiler) = spoiler {
            fields.push(("spoiler_text".to_string(), spoiler.to_string()));
        }
        self.request("PUT", &format!("/api/v1/statuses/{id}"), Body::Form(fields))
    }

    pub fn poll_vote(&self, id: &str, choice: i64) -> Result<Value, String> {
        self.request(
            "POST",
            &format!("/api/v1/polls/{id}/votes"),
            Body::Form(vec![("choices[]".to_string(), choice.to_string())]),
        )
    }

    pub fn status_context(&self, id: &str) -> Result<Value, String> {
        self.get(&format!("/api/v1/statuses/{id}/context"), &[])
    }

    pub fn relationships(&self, id: &str) -> Result<Value, String> {
        self.get_pairs("/api/v1/accounts/relationships", &[("id[]".to_string(), id.to_string())])
    }

    pub fn account_action(&self, id: &str, action: &str) -> Result<Value, String> {
        self.request("POST", &format!("/api/v1/accounts/{id}/{action}"), Body::Empty)
    }

    pub fn account(&self, id: &str) -> Result<Value, String> {
        self.get(&format!("/api/v1/accounts/{id}"), &[])
    }

    pub fn account_list(&self, path: &str) -> Result<Vec<Value>, String> {
        Ok(as_array(self.get_pairs(path, &[("limit".to_string(), "80".to_string())])?))
    }

    pub fn follow_request(&self, id: &str, action: &str) -> Result<Value, String> {
        self.request("POST", &format!("/api/v1/follow_requests/{id}/{action}"), Body::Empty)
    }

    pub fn dismiss_notification(&self, id: &str) -> Result<Value, String> {
        self.request("POST", &format!("/api/v1/notifications/{id}/dismiss"), Body::Empty)
    }

    pub fn clear_notifications(&self) -> Result<Value, String> {
        self.request("POST", "/api/v1/notifications/clear", Body::Empty)
    }

    pub fn instance(&self) -> Result<Value, String> {
        self.get("/api/v1/instance", &[])
    }

    pub fn instance_v2(&self) -> Result<Value, String> {
        self.get("/api/v2/instance", &[])
    }

    pub fn upload_media(&self, path: &std::path::Path, description: &str) -> Result<Value, String> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("upload").to_string();
        let part = reqwest::blocking::multipart::Part::bytes(bytes).file_name(name);
        let mut form = reqwest::blocking::multipart::Form::new().part("file", part);
        if !description.is_empty() {
            form = form.text("description", description.to_string());
        }
        let response = self
            .inner
            .client
            .post(format!("{}/api/v2/media", self.inner.base))
            .bearer_auth(&self.inner.token)
            .multipart(form)
            .send()
            .map_err(|error| error.to_string())?;
        read_json(response)
    }

    pub fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        let pairs = query
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect::<Vec<_>>();
        self.get_pairs(path, &pairs)
    }

    fn get_pairs(&self, path: &str, pairs: &[(String, String)]) -> Result<Value, String> {
        self.request("GET", path, Body::Query(pairs.to_vec()))
    }

    pub fn call(&self, method: &str, path: &str, fields: Vec<(String, String)>) -> Result<Value, String> {
        let body = if method == "GET" {
            Body::Query(fields)
        } else if fields.is_empty() {
            Body::Empty
        } else {
            Body::Form(fields)
        };
        self.request(method, path, body)
    }

    pub fn call_json(&self, method: &str, path: &str, body: Value) -> Result<Value, String> {
        self.request(method, path, Body::Json(body))
    }

    fn request(&self, method: &str, path: &str, body: Body) -> Result<Value, String> {
        let url = format!("{}{path}", self.inner.base);
        let mut builder = self.inner.client.request(
            reqwest::Method::from_bytes(method.as_bytes()).map_err(|error| error.to_string())?,
            &url,
        );
        builder = builder.bearer_auth(&self.inner.token);
        builder = match body {
            Body::Empty => builder,
            Body::Query(pairs) => builder.query(&pairs),
            Body::Form(pairs) => builder
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(encode_form(&pairs)),
            Body::Json(value) => builder.json(&value),
        };
        let response = builder.send().map_err(|error| error.to_string())?;
        if method == "DELETE" {
            return status_only(response);
        }
        read_json(response)
    }
}

enum Body {
    Empty,
    Query(Vec<(String, String)>),
    Form(Vec<(String, String)>),
    Json(Value),
}

pub fn normalize_instance(instance: &str) -> Result<String, String> {
    let trimmed = instance.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Please enter the instance URL.".to_string());
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    Ok(with_scheme)
}

/// The machine-readable Mastodon API version from /api/v2/instance, if advertised.
pub fn api_version(instance: &Value) -> Option<i64> {
    let value = instance.pointer("/api_versions/mastodon")?;
    value.as_i64().or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

pub fn collections_from_response(response: &Value) -> Vec<Value> {
    if let Some(list) = response.get("collections").and_then(Value::as_array) {
        return list.clone();
    }
    if let Some(list) = response.get("collections:").and_then(Value::as_array) {
        return list.clone();
    }
    as_array(response.clone())
}

pub fn collection_from_response(response: Value) -> Value {
    response.get("collection").cloned().unwrap_or(response)
}

pub fn flatten_profile(profile: &Value) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let Some(object) = profile.as_object() else {
        return pairs;
    };
    for (key, value) in object {
        if key == "fields_attributes" {
            if let Some(fields) = value.as_array() {
                for (index, field) in fields.iter().enumerate() {
                    pairs.push((
                        format!("fields_attributes[{index}][name]"),
                        field.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                    ));
                    pairs.push((
                        format!("fields_attributes[{index}][value]"),
                        field.get("value").and_then(Value::as_str).unwrap_or("").to_string(),
                    ));
                }
            }
        } else if key == "attribution_domains" {
            if let Some(domains) = value.as_array() {
                for domain in domains {
                    if let Some(domain) = domain.as_str() {
                        pairs.push(("attribution_domains[]".to_string(), domain.to_string()));
                    }
                }
            }
        } else if let Some(text) = value.as_str() {
            pairs.push((key.clone(), text.to_string()));
        } else if let Some(flag) = value.as_bool() {
            pairs.push((key.clone(), flag.to_string()));
        }
    }
    pairs
}

pub fn as_array(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        other => {
            if other.is_null() {
                Vec::new()
            } else {
                vec![other]
            }
        }
    }
}

fn required_text(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|item| item.to_string())
        .filter(|item| !item.is_empty())
        .ok_or_else(|| format!("Missing {key}"))
}

fn encode_form(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", urlencoding::encode(key), urlencoding::encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn read_json(response: reqwest::blocking::Response) -> Result<Value, String> {
    let status = response.status();
    let text = response.text().unwrap_or_default();
    if !status.is_success() {
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| value.get("error").and_then(Value::as_str).map(|item| item.to_string()))
            .unwrap_or(text);
        return Err(if message.is_empty() {
            format!("Request failed ({status})")
        } else {
            message
        });
    }
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn status_only(response: reqwest::blocking::Response) -> Result<Value, String> {
    let status = response.status();
    if status.is_success() {
        Ok(json!({}))
    } else {
        let text = response.text().unwrap_or_default();
        Err(if text.is_empty() {
            format!("Request failed ({status})")
        } else {
            text
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn profile_update_flattens_field_attributes() {
        let pairs = flatten_profile(&json!({
            "display_name": "Alex",
            "fields_attributes": [{"name": "Site", "value": "https://example.com"}],
            "attribution_domains": ["example.com"]
        }));
        assert!(pairs.contains(&("display_name".to_string(), "Alex".to_string())));
        assert!(pairs.contains(&("fields_attributes[0][name]".to_string(), "Site".to_string())));
        assert!(pairs.contains(&("fields_attributes[0][value]".to_string(), "https://example.com".to_string())));
        assert!(pairs.contains(&("attribution_domains[]".to_string(), "example.com".to_string())));
    }

    #[test]
    fn collections_response_accepts_documented_and_typo_keys() {
        assert_eq!(collections_from_response(&json!({"collections": [1, 2]})).len(), 2);
        assert_eq!(collections_from_response(&json!({"collections:": [3]})).len(), 1);
        assert_eq!(collections_from_response(&json!([4])).len(), 1);
    }

    #[test]
    fn api_version_reads_numbers_and_strings() {
        assert_eq!(api_version(&json!({"api_versions": {"mastodon": 7}})), Some(7));
        assert_eq!(api_version(&json!({"api_versions": {"mastodon": "10"}})), Some(10));
        assert_eq!(api_version(&json!({"api_versions": {}})), None);
        assert_eq!(api_version(&json!({})), None);
    }

    #[test]
    fn instance_urls_gain_https() {
        assert_eq!(normalize_instance("vee.seedy.cc").unwrap(), "https://vee.seedy.cc");
        assert_eq!(normalize_instance("https://vee.seedy.cc/").unwrap(), "https://vee.seedy.cc");
    }
}
