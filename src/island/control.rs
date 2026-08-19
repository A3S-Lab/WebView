use std::fs::OpenOptions;
#[cfg(unix)]
use std::fs::Permissions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

const CONTROL_REQUEST_SCHEMA: &str = "a3s.agent_control_request.v1";
const CONTROL_REQUEST_DIRECTORY: &str = "control-requests";
const MAX_IPC_BYTES: usize = 24 * 1024;
const MAX_ACTIVITY_ID_CHARS: usize = 160;
const MAX_INSTANCE_ID_CHARS: usize = 160;
const CONTROL_TOKEN_HEX_CHARS: usize = 32;
pub(super) const MAX_REQUEST_BYTES: usize = 24 * 1024;
const MAX_CONTROL_FUTURE_MS: u64 = 17_000;
pub(crate) const MAX_REPLY_CHARS: usize = 1_000;
const MAX_REPLY_BYTES: usize = 4 * 1024;
const MAX_FORM_PAYLOAD_CHARS: usize = 4_096;
const MAX_FORM_PAYLOAD_BYTES: usize = 12 * 1024;
const MAX_SECRET_BYTES: usize = 16 * 1024;
const MAX_VERIFICATION_CODE_CHARS: usize = 64;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ControlTransport {
    #[default]
    DurableQueue,
    EphemeralSocket,
    #[serde(other)]
    Unknown,
}

impl ControlTransport {
    pub(crate) fn is_supported(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentControlActionKind {
    ApproveOnce,
    ApproveAlways,
    Deny,
    Stop,
    Cancel,
    Reply,
    ApproveSuggestion,
    DismissSuggestion,
    EnableSuggestions,
    DisableSuggestions,
    StartChannelPairing,
    AdvanceChannelPairing,
    SaveLlmConfiguration,
    SetLlmApiKey,
    #[default]
    #[serde(other)]
    Unknown,
}

impl AgentControlActionKind {
    pub(crate) fn is_supported(self) -> bool {
        !matches!(self, Self::Unknown)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ApproveOnce => "Allow",
            Self::ApproveAlways => "Always",
            Self::Deny => "Deny",
            Self::Stop => "Stop",
            Self::Cancel => "Cancel",
            Self::Reply => "Reply",
            Self::ApproveSuggestion => "Send to Codex",
            Self::DismissSuggestion => "Dismiss",
            Self::EnableSuggestions => "Enable suggestions",
            Self::DisableSuggestions => "Pause suggestions",
            Self::StartChannelPairing => "Connect",
            Self::AdvanceChannelPairing => "Continue",
            Self::SaveLlmConfiguration => "Save settings",
            Self::SetLlmApiKey => "Replace API key",
            Self::Unknown => "Unavailable",
        }
    }

    pub(crate) fn tone(self) -> &'static str {
        match self {
            Self::ApproveOnce => "allow",
            Self::ApproveAlways => "always",
            Self::Deny | Self::Stop | Self::Cancel => "destructive",
            Self::Reply => "reply",
            Self::ApproveSuggestion => "allow",
            Self::DismissSuggestion => "destructive",
            Self::EnableSuggestions | Self::DisableSuggestions => "toggle",
            Self::StartChannelPairing | Self::AdvanceChannelPairing => "allow",
            Self::SaveLlmConfiguration | Self::SetLlmApiKey => "allow",
            Self::Unknown => "muted",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ControlDescriptor {
    #[serde(default)]
    pub(crate) action: AgentControlActionKind,
    #[serde(default)]
    pub(crate) transport: ControlTransport,
    #[serde(default)]
    pub(crate) token: String,
    #[serde(default)]
    pub(crate) target_instance_id: String,
    #[serde(default)]
    pub(crate) expires_at_ms: u64,
}

impl ControlDescriptor {
    pub(crate) fn sanitize(self, now_ms: u64) -> Option<Self> {
        (self.action.is_supported()
            && self.transport.is_supported()
            && matches!(
                (self.action, self.transport),
                (
                    AgentControlActionKind::SetLlmApiKey,
                    ControlTransport::EphemeralSocket
                ) | (
                    AgentControlActionKind::ApproveOnce
                        | AgentControlActionKind::ApproveAlways
                        | AgentControlActionKind::Deny
                        | AgentControlActionKind::Stop
                        | AgentControlActionKind::Cancel
                        | AgentControlActionKind::Reply
                        | AgentControlActionKind::ApproveSuggestion
                        | AgentControlActionKind::DismissSuggestion
                        | AgentControlActionKind::EnableSuggestions
                        | AgentControlActionKind::DisableSuggestions
                        | AgentControlActionKind::StartChannelPairing
                        | AgentControlActionKind::AdvanceChannelPairing
                        | AgentControlActionKind::SaveLlmConfiguration,
                    ControlTransport::DurableQueue
                )
            )
            && valid_identifier(&self.target_instance_id, MAX_INSTANCE_ID_CHARS)
            && valid_token(&self.token)
            && self.expires_at_ms >= now_ms
            && self.expires_at_ms <= now_ms.saturating_add(MAX_CONTROL_FUTURE_MS))
        .then_some(self)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ControlSubmission {
    pub(crate) activity_id: String,
    pub(crate) action: AgentControlActionKind,
    pub(crate) transport: ControlTransport,
    pub(crate) message: Option<String>,
    pub(crate) token: String,
    pub(crate) target_instance_id: String,
}

impl std::fmt::Debug for ControlSubmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ControlSubmission")
            .field("activity_id", &self.activity_id)
            .field("action", &self.action)
            .field("transport", &self.transport)
            .field("message", &self.message.as_ref().map(|_| "[REDACTED]"))
            .field("token", &"[REDACTED]")
            .field("target_instance_id", &self.target_instance_id)
            .finish()
    }
}

impl Drop for ControlSubmission {
    fn drop(&mut self) {
        if let Some(message) = &mut self.message {
            message.zeroize();
        }
        self.token.zeroize();
    }
}

#[derive(Deserialize)]
struct ControlIpcMessage {
    #[serde(rename = "type")]
    message_type: String,
    activity_id: String,
    action: AgentControlActionKind,
    #[serde(default)]
    transport: ControlTransport,
    #[serde(default)]
    message: Option<String>,
    token: String,
    target_instance_id: String,
}

pub(crate) fn parse_submission(body: &str) -> Option<ControlSubmission> {
    if body.len() > MAX_IPC_BYTES {
        return None;
    }
    let message: ControlIpcMessage = serde_json::from_str(body).ok()?;
    let reply = match message.action {
        AgentControlActionKind::Reply | AgentControlActionKind::ApproveSuggestion => {
            sanitize_message(message.message)?
        }
        AgentControlActionKind::SaveLlmConfiguration => sanitize_form_payload(message.message)?,
        AgentControlActionKind::SetLlmApiKey => sanitize_secret(message.message)?,
        AgentControlActionKind::AdvanceChannelPairing => {
            sanitize_optional_verification_code(message.message)?
        }
        _ if message.message.is_none() => None,
        _ => return None,
    };
    if message.message_type != "control"
        || !message.action.is_supported()
        || !message.transport.is_supported()
        || !valid_identifier(&message.activity_id, MAX_ACTIVITY_ID_CHARS)
        || !valid_identifier(&message.target_instance_id, MAX_INSTANCE_ID_CHARS)
        || !valid_token(&message.token)
    {
        return None;
    }
    Some(ControlSubmission {
        activity_id: message.activity_id,
        action: message.action,
        transport: message.transport,
        message: reply,
        token: message.token,
        target_instance_id: message.target_instance_id,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct AuthorizedControl {
    pub(crate) activity_id: String,
    pub(crate) action: AgentControlActionKind,
    pub(crate) transport: ControlTransport,
    pub(crate) message: Option<String>,
    pub(crate) token: String,
    pub(crate) target_instance_id: String,
    pub(crate) expires_at_ms: u64,
}

impl std::fmt::Debug for AuthorizedControl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthorizedControl")
            .field("activity_id", &self.activity_id)
            .field("action", &self.action)
            .field("transport", &self.transport)
            .field("message", &self.message.as_ref().map(|_| "[REDACTED]"))
            .field("token", &"[REDACTED]")
            .field("target_instance_id", &self.target_instance_id)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

impl Drop for AuthorizedControl {
    fn drop(&mut self) {
        if let Some(message) = &mut self.message {
            message.zeroize();
        }
        self.token.zeroize();
    }
}

#[derive(Serialize)]
struct ControlProtocolRequest<'a> {
    schema: &'static str,
    request_id: &'a str,
    target_instance_id: &'a str,
    activity_id: &'a str,
    action: AgentControlActionKind,
    transport: ControlTransport,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    token: &'a str,
    created_at_ms: u64,
    expires_at_ms: u64,
}

pub(crate) struct ControlQueue {
    parent: PathBuf,
    queue: PathBuf,
}

impl ControlQueue {
    pub(crate) fn for_snapshot(snapshot: &Path) -> Result<Self, String> {
        let parent = snapshot
            .parent()
            .ok_or_else(|| "snapshot has no control queue parent".to_string())?
            .to_path_buf();
        Ok(Self {
            queue: parent.join(CONTROL_REQUEST_DIRECTORY),
            parent,
        })
    }

    pub(crate) fn submit(&self, control: &AuthorizedControl, now_ms: u64) -> Result<(), String> {
        if control.transport != ControlTransport::DurableQueue
            || control.action == AgentControlActionKind::SetLlmApiKey
        {
            return Err("sensitive controls cannot use the durable queue".to_string());
        }
        if control.expires_at_ms < now_ms
            || control.expires_at_ms > now_ms.saturating_add(MAX_CONTROL_FUTURE_MS)
        {
            return Err("control authorization expired".to_string());
        }
        super::singleton::validate_private_directory(&self.parent)?;
        ensure_private_directory(&self.queue)?;
        super::singleton::validate_private_directory(&self.queue)?;

        let (request_id, bytes) = encode_protocol_request(control, now_ms)?;

        let temporary = self.queue.join(format!(".control-{request_id}.tmp"));
        let path = self.queue.join(format!("control-{request_id}.json"));
        let result = (|| {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
            }
            let mut file = options
                .open(&temporary)
                .map_err(|error| format!("create private control request: {error}"))?;
            file.write_all(&bytes)
                .map_err(|error| format!("write control request: {error}"))?;
            file.flush()
                .map_err(|error| format!("flush control request: {error}"))?;
            drop(file);
            std::fs::rename(&temporary, &path)
                .map_err(|error| format!("publish control request: {error}"))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
}

pub(super) fn encode_protocol_request(
    control: &AuthorizedControl,
    now_ms: u64,
) -> Result<(String, Vec<u8>), String> {
    if control.expires_at_ms < now_ms
        || control.expires_at_ms > now_ms.saturating_add(MAX_CONTROL_FUTURE_MS)
    {
        return Err("control authorization expired".to_string());
    }
    let request_id = next_request_id(now_ms);
    let request = ControlProtocolRequest {
        schema: CONTROL_REQUEST_SCHEMA,
        request_id: &request_id,
        target_instance_id: &control.target_instance_id,
        activity_id: &control.activity_id,
        action: control.action,
        transport: control.transport,
        message: control.message.as_deref(),
        token: &control.token,
        created_at_ms: now_ms,
        expires_at_ms: control.expires_at_ms,
    };
    let bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("serialize control request: {error}"))?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err("control request exceeds the size limit".to_string());
    }
    Ok((request_id, bytes))
}

fn next_request_id(now_ms: u64) -> String {
    let sequence = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{now_ms:x}-{sequence:x}", std::process::id())
}

fn valid_identifier(value: &str, max_chars: usize) -> bool {
    let count = value.chars().count();
    count > 0
        && count <= max_chars
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{206f}'
                )
        })
}

fn valid_token(token: &str) -> bool {
    token.len() == CONTROL_TOKEN_HEX_CHARS && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sanitize_message(message: Option<String>) -> Option<Option<String>> {
    let message = message?;
    let trimmed = message.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_REPLY_BYTES
        || trimmed.chars().count() > MAX_REPLY_CHARS
        || trimmed.chars().any(|character| {
            (character.is_control() && !matches!(character, '\n' | '\t'))
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{206f}'
                )
        })
    {
        return None;
    }
    Some(Some(trimmed.to_string()))
}

fn sanitize_form_payload(message: Option<String>) -> Option<Option<String>> {
    let message = message?;
    let trimmed = message.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_FORM_PAYLOAD_BYTES
        || trimmed.chars().count() > MAX_FORM_PAYLOAD_CHARS
        || trimmed
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return None;
    }
    Some(Some(trimmed.to_string()))
}

fn sanitize_secret(message: Option<String>) -> Option<Option<String>> {
    let message = message?;
    if message.is_empty()
        || message.len() > MAX_SECRET_BYTES
        || message.chars().any(char::is_control)
    {
        return None;
    }
    Some(Some(message))
}

fn sanitize_optional_verification_code(message: Option<String>) -> Option<Option<String>> {
    let Some(message) = message else {
        return Some(None);
    };
    let trimmed = message.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_VERIFICATION_CODE_CHARS
        || trimmed.chars().any(char::is_control)
    {
        return None;
    }
    Some(Some(trimmed.to_string()))
}

#[cfg(unix)]
fn ensure_private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("create private control queue: {error}")),
    }
    std::fs::set_permissions(path, Permissions::from_mode(0o700))
        .map_err(|error| format!("secure private control queue: {error}"))
}

#[cfg(not(unix))]
fn ensure_private_directory(path: &Path) -> Result<(), String> {
    match std::fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(format!("create private control queue: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "a3s-webview-island-control-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
        }
        path
    }

    fn submission_json(action: &str) -> String {
        format!(
            r#"{{"type":"control","activity_id":"instance:child","action":"{action}","token":"0123456789abcdef0123456789abcdef","target_instance_id":"instance"}}"#
        )
    }

    #[test]
    fn ipc_accepts_only_bounded_supported_controls() {
        let submission = parse_submission(&submission_json("cancel")).unwrap();
        assert_eq!(submission.action, AgentControlActionKind::Cancel);
        assert!(parse_submission(&submission_json("future_action")).is_none());
        assert!(parse_submission(&"x".repeat(MAX_IPC_BYTES + 1)).is_none());
    }

    #[test]
    fn ipc_accepts_a_bounded_reply_and_rejects_missing_or_oversized_text() {
        let reply = r#"{"type":"control","activity_id":"instance","action":"reply","message":"Use the safer path.","token":"0123456789abcdef0123456789abcdef","target_instance_id":"instance"}"#;
        let submission = parse_submission(reply).unwrap();
        assert_eq!(submission.action, AgentControlActionKind::Reply);
        assert_eq!(submission.message.as_deref(), Some("Use the safer path."));

        let missing = r#"{"type":"control","activity_id":"instance","action":"reply","token":"0123456789abcdef0123456789abcdef","target_instance_id":"instance"}"#;
        assert!(parse_submission(missing).is_none());
        let oversized = format!(
            r#"{{"type":"control","activity_id":"instance","action":"reply","message":"{}","token":"0123456789abcdef0123456789abcdef","target_instance_id":"instance"}}"#,
            "x".repeat(MAX_REPLY_CHARS + 1)
        );
        assert!(parse_submission(&oversized).is_none());
    }

    #[test]
    fn suggestion_send_requires_the_complete_bounded_edited_draft() {
        let send = r#"{"type":"control","activity_id":"suggestion:one","action":"approve_suggestion","message":"Re-check the exact boundary.","token":"0123456789abcdef0123456789abcdef","target_instance_id":"reviewer-surface"}"#;
        let submission = parse_submission(send).unwrap();
        assert_eq!(submission.action, AgentControlActionKind::ApproveSuggestion);
        assert_eq!(
            submission.message.as_deref(),
            Some("Re-check the exact boundary.")
        );

        let missing = r#"{"type":"control","activity_id":"suggestion:one","action":"approve_suggestion","token":"0123456789abcdef0123456789abcdef","target_instance_id":"reviewer-surface"}"#;
        assert!(parse_submission(missing).is_none());
        let forged_dismiss = r#"{"type":"control","activity_id":"suggestion:one","action":"dismiss_suggestion","message":"hidden text","token":"0123456789abcdef0123456789abcdef","target_instance_id":"reviewer-surface"}"#;
        assert!(parse_submission(forged_dismiss).is_none());
    }

    #[test]
    fn api_keys_require_the_ephemeral_transport_and_are_redacted_from_debug() {
        let body = r#"{"type":"control","activity_id":"llm:settings","action":"set_llm_api_key","transport":"ephemeral_socket","message":"top-secret-value","token":"0123456789abcdef0123456789abcdef","target_instance_id":"surface"}"#;
        let submission = parse_submission(body).unwrap();
        assert_eq!(submission.transport, ControlTransport::EphemeralSocket);
        assert!(!format!("{submission:?}").contains("top-secret-value"));

        let wrong_transport = body.replace("ephemeral_socket", "durable_queue");
        let submission = parse_submission(&wrong_transport).unwrap();
        let descriptor = ControlDescriptor {
            action: submission.action,
            transport: submission.transport,
            token: submission.token.clone(),
            target_instance_id: submission.target_instance_id.clone(),
            expires_at_ms: 10_000,
        };
        assert!(descriptor.sanitize(1_000).is_none());
    }

    #[test]
    fn queue_publishes_one_private_versioned_request() {
        let directory = temp_dir();
        let queue = ControlQueue::for_snapshot(&directory.join("system-snapshot.json")).unwrap();
        let control = AuthorizedControl {
            activity_id: "instance:child".to_string(),
            action: AgentControlActionKind::Cancel,
            transport: ControlTransport::DurableQueue,
            message: None,
            token: "0123456789abcdef0123456789abcdef".to_string(),
            target_instance_id: "instance".to_string(),
            expires_at_ms: 11_000,
        };
        queue.submit(&control, 1_000).unwrap();

        let queue_path = directory.join(CONTROL_REQUEST_DIRECTORY);
        let entries = std::fs::read_dir(&queue_path)
            .unwrap()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        let body = std::fs::read(entries[0].path()).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["schema"], CONTROL_REQUEST_SCHEMA);
        assert_eq!(value["action"], "cancel");
        assert_eq!(value["target_instance_id"], "instance");
        assert!(value.get("message").is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                entries[0].metadata().unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(&queue_path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn durable_queue_rejects_secret_bearing_controls_before_creating_a_directory() {
        let directory = temp_dir();
        let queue = ControlQueue::for_snapshot(&directory.join("system-snapshot.json")).unwrap();
        let control = AuthorizedControl {
            activity_id: "llm:settings".to_string(),
            action: AgentControlActionKind::SetLlmApiKey,
            transport: ControlTransport::EphemeralSocket,
            message: Some("top-secret-value".to_string()),
            token: "0123456789abcdef0123456789abcdef".to_string(),
            target_instance_id: "surface".to_string(),
            expires_at_ms: 11_000,
        };
        assert!(queue.submit(&control, 1_000).is_err());
        assert!(!directory.join(CONTROL_REQUEST_DIRECTORY).exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
