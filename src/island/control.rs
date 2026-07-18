use std::fs::OpenOptions;
#[cfg(unix)]
use std::fs::Permissions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

const CONTROL_REQUEST_SCHEMA: &str = "a3s.agent_control_request.v1";
const CONTROL_REQUEST_DIRECTORY: &str = "control-requests";
const MAX_IPC_BYTES: usize = 2 * 1024;
const MAX_ACTIVITY_ID_CHARS: usize = 160;
const MAX_INSTANCE_ID_CHARS: usize = 160;
const CONTROL_TOKEN_HEX_CHARS: usize = 32;
const MAX_REQUEST_BYTES: usize = 8 * 1024;
const MAX_CONTROL_FUTURE_MS: u64 = 17_000;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentControlActionKind {
    ApproveOnce,
    ApproveAlways,
    Deny,
    Stop,
    Cancel,
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
            Self::Unknown => "Unavailable",
        }
    }

    pub(crate) fn tone(self) -> &'static str {
        match self {
            Self::ApproveOnce => "allow",
            Self::ApproveAlways => "always",
            Self::Deny | Self::Stop | Self::Cancel => "destructive",
            Self::Unknown => "muted",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ControlDescriptor {
    #[serde(default)]
    pub(crate) action: AgentControlActionKind,
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
            && valid_identifier(&self.target_instance_id, MAX_INSTANCE_ID_CHARS)
            && valid_token(&self.token)
            && self.expires_at_ms >= now_ms
            && self.expires_at_ms <= now_ms.saturating_add(MAX_CONTROL_FUTURE_MS))
        .then_some(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ControlSubmission {
    pub(crate) activity_id: String,
    pub(crate) action: AgentControlActionKind,
    pub(crate) token: String,
    pub(crate) target_instance_id: String,
}

#[derive(Deserialize)]
struct ControlIpcMessage {
    #[serde(rename = "type")]
    message_type: String,
    activity_id: String,
    action: AgentControlActionKind,
    token: String,
    target_instance_id: String,
}

pub(crate) fn parse_submission(body: &str) -> Option<ControlSubmission> {
    if body.len() > MAX_IPC_BYTES {
        return None;
    }
    let message: ControlIpcMessage = serde_json::from_str(body).ok()?;
    if message.message_type != "control"
        || !message.action.is_supported()
        || !valid_identifier(&message.activity_id, MAX_ACTIVITY_ID_CHARS)
        || !valid_identifier(&message.target_instance_id, MAX_INSTANCE_ID_CHARS)
        || !valid_token(&message.token)
    {
        return None;
    }
    Some(ControlSubmission {
        activity_id: message.activity_id,
        action: message.action,
        token: message.token,
        target_instance_id: message.target_instance_id,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthorizedControl {
    pub(crate) activity_id: String,
    pub(crate) action: AgentControlActionKind,
    pub(crate) token: String,
    pub(crate) target_instance_id: String,
    pub(crate) expires_at_ms: u64,
}

#[derive(Serialize)]
struct ControlProtocolRequest<'a> {
    schema: &'static str,
    request_id: &'a str,
    target_instance_id: &'a str,
    activity_id: &'a str,
    action: AgentControlActionKind,
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
        if control.expires_at_ms < now_ms
            || control.expires_at_ms > now_ms.saturating_add(MAX_CONTROL_FUTURE_MS)
        {
            return Err("control authorization expired".to_string());
        }
        super::singleton::validate_private_directory(&self.parent)?;
        ensure_private_directory(&self.queue)?;
        super::singleton::validate_private_directory(&self.queue)?;

        let request_id = next_request_id(now_ms);
        let request = ControlProtocolRequest {
            schema: CONTROL_REQUEST_SCHEMA,
            request_id: &request_id,
            target_instance_id: &control.target_instance_id,
            activity_id: &control.activity_id,
            action: control.action,
            token: &control.token,
            created_at_ms: now_ms,
            expires_at_ms: control.expires_at_ms,
        };
        let bytes = serde_json::to_vec(&request)
            .map_err(|error| format!("serialize control request: {error}"))?;
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("control request exceeds the size limit".to_string());
        }

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
    fn queue_publishes_one_private_versioned_request() {
        let directory = temp_dir();
        let queue = ControlQueue::for_snapshot(&directory.join("system-snapshot.json")).unwrap();
        let control = AuthorizedControl {
            activity_id: "instance:child".to_string(),
            action: AgentControlActionKind::Cancel,
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
}
