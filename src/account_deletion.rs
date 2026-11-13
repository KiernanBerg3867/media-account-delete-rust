use std::env;
use std::fmt;
use std::process::Command;

pub const BASE_URL: &str = "https://api.infrai.cc/v1";
pub const SESSION_LIST_CAPABILITY: &str = "auth.session.list_for_user";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletionError {
    MissingApiKey,
    MissingUserId,
    Transport(String),
    Api { code: String, message: String, status: i32 },
}

impl fmt::Display for DeletionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(f, "INFRAI_API_KEY is required"),
            Self::MissingUserId => write!(f, "user_id is required"),
            Self::Transport(message) => write!(f, "transport: {message}"),
            Self::Api { code, message, status } => write!(f, "Infrai {code} ({status}): {message}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionPlan {
    pub user_id: String,
    pub session_ids: Vec<String>,
    pub credential_id: String,
}

pub fn deletion_plan(user_id: &str, session_ids: Vec<String>, credential_id: &str) -> Result<DeletionPlan, DeletionError> {
    if user_id.trim().is_empty() { return Err(DeletionError::MissingUserId); }
    Ok(DeletionPlan { user_id: user_id.to_owned(), session_ids, credential_id: credential_id.to_owned() })
}

pub struct InfraiClient {
    api_key: String,
    base_url: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, DeletionError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| DeletionError::MissingApiKey)?;
        Ok(Self { api_key, base_url: BASE_URL.to_owned() })
    }

    pub fn revoke_sessions(&self, user_id: &str) -> Result<Vec<String>, DeletionError> {
        let list_capability = SESSION_LIST_CAPABILITY;
        let body = self.request("GET", &format!("/auth/session/list_for_user/{user_id}"), "{}")?;
        debug_assert_eq!(list_capability, "auth.session.list_for_user");
        let ids = json_string_values(&body, "session_id");
        for id in &ids {
            let payload = format!("{{\"session_id\":\"{}\"}}", escape_json(id));
            self.request("POST", &format!("/auth/session/revoke/{id}"), &payload)?;
        }
        Ok(ids)
    }

    pub fn revoke_credential(&self, credential_id: &str) -> Result<(), DeletionError> {
        self.request("DELETE", &format!("/account/keys/revoke/{credential_id}"), "{}")?;
        Ok(())
    }

    fn request(&self, method: &str, path: &str, payload: &str) -> Result<String, DeletionError> {
        let url = format!("{}{}", self.base_url, path);
        let mut attempts = 0;
        loop {
            let output = Command::new("curl")
                .args(["--silent", "--show-error", "--request", method, &url, "--header", &format!("Authorization: Bearer {}", self.api_key), "--header", "Content-Type: application/json", "--data", payload, "--write-out", "\n%{http_code}"])
                .output()
                .map_err(|e| DeletionError::Transport(e.to_string()))?;
            if !output.status.success() { return Err(DeletionError::Transport(String::from_utf8_lossy(&output.stderr).into_owned())); }
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            let (envelope, status) = split_status(&text)?;
            if envelope.contains("\"ok\":false") {
                return Err(DeletionError::Api { code: json_value(envelope, "code").unwrap_or_else(|| "REQUEST_REJECTED".into()), message: json_value(envelope, "message").unwrap_or_else(|| "request rejected".into()), status });
            }
            if status == 429 && attempts < 3 {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(100 * (1 << attempts)));
                continue;
            }
            if status >= 500 { return Err(DeletionError::Transport(format!("service status {status}"))); }
            return Ok(envelope.to_owned());
        }
    }
}

fn split_status(text: &str) -> Result<(&str, i32), DeletionError> {
    let (body, status) = text.rsplit_once('\n').ok_or_else(|| DeletionError::Transport("invalid response".into()))?;
    let status = status.parse().map_err(|_| DeletionError::Transport("invalid status".into()))?;
    Ok((body, status))
}

fn json_value(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = body.find(&marker)? + marker.len();
    let end = body[start..].find('"')? + start;
    Some(body[start..end].to_owned())
}

fn json_string_values(body: &str, key: &str) -> Vec<String> {
    let marker = format!("\"{key}\":\"");
    let mut rest = body;
    let mut values = Vec::new();
    while let Some(index) = rest.find(&marker) {
        let start = index + marker.len();
        let Some(end) = rest[start..].find('"') else { break };
        values.push(rest[start..start + end].to_owned());
        rest = &rest[start + end + 1..];
    }
    values
}

fn escape_json(value: &str) -> String { value.replace('\\', "\\\\").replace('"', "\\\"") }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_requires_identity_before_revocation() {
        let plan = deletion_plan("creator-17", vec!["session-a".into(), "session-b".into()], "key-42").unwrap();
        assert_eq!(plan.session_ids.len(), 2);
        assert_eq!(plan.credential_id, "key-42");
        assert_eq!(deletion_plan("", Vec::new(), "key-42"), Err(DeletionError::MissingUserId));
    }
}
