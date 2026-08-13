use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

pub(super) const PROTOCOL_VERSION: &str = "a3s.workspace.v1";
pub(super) const MAX_MESSAGE_BYTES: usize = 256 * 1024;
pub(super) const MAX_BRIDGE_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_RESOURCE_ID_CHARACTERS: usize = 128;
const MAX_TITLE_CHARACTERS: usize = 160;
const MAX_BOUND: f64 = 16_384.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct WorkspaceBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl WorkspaceBounds {
    fn validate(self) -> Result<Self, String> {
        let values = [self.x, self.y, self.width, self.height];
        if values.iter().any(|value| !value.is_finite()) {
            return Err("workspace bounds must contain finite numbers".to_string());
        }
        if self.x < 0.0 || self.y < 0.0 || self.width < 1.0 || self.height < 1.0 {
            return Err("workspace bounds must be positive and have non-zero size".to_string());
        }
        if values.iter().any(|value| *value > MAX_BOUND) {
            return Err(format!(
                "workspace bounds must not exceed {MAX_BOUND} logical pixels"
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BridgeMode {
    #[default]
    None,
    Typed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct WorkspacePolicy {
    pub bridge: BridgeMode,
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all = "snake_case")]
pub(super) enum WorkspaceTarget {
    LocalApp { path: String },
    Remote { url: String },
    File { path: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub(super) enum WorkspaceCommand {
    #[serde(rename = "workspace.open")]
    Open {
        version: String,
        #[serde(rename = "resourceId")]
        resource_id: String,
        generation: u64,
        title: String,
        target: WorkspaceTarget,
        bounds: WorkspaceBounds,
        #[serde(default)]
        policy: WorkspacePolicy,
    },
    #[serde(rename = "workspace.bounds")]
    Bounds {
        version: String,
        #[serde(rename = "resourceId")]
        resource_id: String,
        generation: u64,
        bounds: WorkspaceBounds,
    },
    #[serde(rename = "workspace.occlusion")]
    Occlusion {
        version: String,
        #[serde(rename = "resourceId")]
        resource_id: String,
        generation: u64,
        occluded: bool,
    },
    #[serde(rename = "workspace.close")]
    Close {
        version: String,
        #[serde(rename = "resourceId")]
        resource_id: String,
        generation: u64,
    },
    #[serde(rename = "workspace.post_message")]
    PostMessage {
        version: String,
        #[serde(rename = "resourceId")]
        resource_id: String,
        generation: u64,
        payload: Value,
    },
}

impl WorkspaceCommand {
    pub(super) fn parse(body: &str) -> Result<Self, String> {
        if body.len() > MAX_MESSAGE_BYTES {
            return Err(format!(
                "workspace command exceeds the {MAX_MESSAGE_BYTES}-byte limit"
            ));
        }
        let command: Self = serde_json::from_str(body)
            .map_err(|error| format!("invalid workspace command: {error}"))?;
        command.validate()
    }

    fn validate(self) -> Result<Self, String> {
        let (version, resource_id, generation) = match &self {
            Self::Open {
                version,
                resource_id,
                generation,
                ..
            }
            | Self::Bounds {
                version,
                resource_id,
                generation,
                ..
            }
            | Self::Occlusion {
                version,
                resource_id,
                generation,
                ..
            }
            | Self::Close {
                version,
                resource_id,
                generation,
            }
            | Self::PostMessage {
                version,
                resource_id,
                generation,
                ..
            } => (version, resource_id, *generation),
        };
        validate_version(version)?;
        validate_resource_id(resource_id)?;
        if generation == 0 {
            return Err("workspace generation must be greater than zero".to_string());
        }
        match &self {
            Self::Open { title, bounds, .. } => {
                validate_title(title)?;
                bounds.validate()?;
            }
            Self::Bounds { bounds, .. } => {
                bounds.validate()?;
            }
            Self::PostMessage { payload, .. } => {
                let bytes = serde_json::to_vec(payload)
                    .map_err(|error| format!("workspace message is not serializable: {error}"))?;
                if bytes.len() > MAX_BRIDGE_MESSAGE_BYTES {
                    return Err(format!(
                        "workspace message exceeds the {MAX_BRIDGE_MESSAGE_BYTES}-byte limit"
                    ));
                }
            }
            Self::Occlusion { .. } | Self::Close { .. } => {}
        }
        Ok(self)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub(super) enum ViewBridgeMessage {
    #[serde(rename = "workspace.view_ready", rename_all = "camelCase")]
    Ready {
        version: String,
        resource_id: String,
        generation: u64,
    },
    #[serde(rename = "workspace.view_error", rename_all = "camelCase")]
    Error {
        version: String,
        resource_id: String,
        generation: u64,
        message: String,
    },
    #[serde(rename = "workspace.view_message", rename_all = "camelCase")]
    Message {
        version: String,
        resource_id: String,
        generation: u64,
        payload: Value,
    },
}

impl ViewBridgeMessage {
    pub(super) fn parse(body: &str) -> Result<Self, String> {
        if body.len() > MAX_BRIDGE_MESSAGE_BYTES {
            return Err(format!(
                "workspace view message exceeds the {MAX_BRIDGE_MESSAGE_BYTES}-byte limit"
            ));
        }
        let message: Self = serde_json::from_str(body)
            .map_err(|error| format!("invalid workspace view message: {error}"))?;
        let (version, resource_id, generation) = message.identity();
        validate_version(version)?;
        validate_resource_id(resource_id)?;
        if generation == 0 {
            return Err("workspace view generation must be greater than zero".to_string());
        }
        match &message {
            Self::Error { message, .. } => {
                let length = message.trim().chars().count();
                if length == 0 || length > 800 || message.chars().any(char::is_control) {
                    return Err(
                        "workspace view error must contain 1–800 printable characters".to_string(),
                    );
                }
            }
            Self::Message { payload, .. } => {
                let payload = serde_json::to_vec(payload).map_err(|error| {
                    format!("workspace view payload is not serializable: {error}")
                })?;
                if payload.len() > MAX_BRIDGE_MESSAGE_BYTES {
                    return Err(format!(
                        "workspace view payload exceeds the {MAX_BRIDGE_MESSAGE_BYTES}-byte limit"
                    ));
                }
            }
            Self::Ready { .. } => {}
        }
        Ok(message)
    }

    pub(super) fn identity(&self) -> (&str, &str, u64) {
        match self {
            Self::Ready {
                version,
                resource_id,
                generation,
            }
            | Self::Error {
                version,
                resource_id,
                generation,
                ..
            }
            | Self::Message {
                version,
                resource_id,
                generation,
                ..
            } => (version, resource_id, *generation),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResourceIdentity {
    pub resource_id: String,
    pub generation: u64,
}

#[derive(Debug, Clone)]
pub(super) struct ValidatedOpen {
    pub identity: ResourceIdentity,
    pub url: String,
    pub bounds: WorkspaceBounds,
    pub navigation: NavigationPolicy,
    pub bridge: BridgeMode,
}

#[derive(Debug, Clone)]
pub(super) struct NavigationPolicy {
    allowed_origins: Vec<String>,
    allowed_file_roots: Vec<PathBuf>,
}

impl NavigationPolicy {
    pub(super) fn allows(&self, value: &str) -> bool {
        if value == "about:blank" {
            return true;
        }
        let Ok(url) = Url::parse(value) else {
            return false;
        };
        match url.scheme() {
            "http" | "https" => {
                !has_credentials(&url)
                    && self
                        .allowed_origins
                        .iter()
                        .any(|origin| origin == &url.origin().ascii_serialization())
            }
            "file" => url
                .to_file_path()
                .ok()
                .and_then(|path| path.canonicalize().ok())
                .is_some_and(|path| {
                    self.allowed_file_roots
                        .iter()
                        .any(|root| path.starts_with(root))
                }),
            _ => false,
        }
    }
}

pub(super) struct ValidationContext<'a> {
    pub shell_url: &'a Url,
    pub allowed_file_roots: &'a [PathBuf],
}

pub(super) fn validate_open(
    resource_id: String,
    generation: u64,
    title: String,
    target: WorkspaceTarget,
    bounds: WorkspaceBounds,
    policy: WorkspacePolicy,
    context: &ValidationContext<'_>,
) -> Result<ValidatedOpen, String> {
    validate_resource_id(&resource_id)?;
    if generation == 0 {
        return Err("workspace generation must be greater than zero".to_string());
    }
    validate_title(&title)?;
    let bounds = bounds.validate()?;

    let (url, initial_origin, file_root) = match target {
        WorkspaceTarget::LocalApp { path } => {
            if !matches!(context.shell_url.scheme(), "http" | "https") {
                return Err(
                    "local workspace applications require an http or https shell origin"
                        .to_string(),
                );
            }
            validate_local_app_path(&path)?;
            let target = context
                .shell_url
                .join(&path)
                .map_err(|error| format!("invalid local workspace application path: {error}"))?;
            if target.origin() != context.shell_url.origin() {
                return Err("local workspace application must stay on the shell origin".to_string());
            }
            (
                target.to_string(),
                Some(target.origin().ascii_serialization()),
                None,
            )
        }
        WorkspaceTarget::Remote { url } => {
            let target = validated_remote_url(&url)?;
            (
                target.to_string(),
                Some(target.origin().ascii_serialization()),
                None,
            )
        }
        WorkspaceTarget::File { path } => {
            let canonical = validated_file_path(&path, context.allowed_file_roots)?;
            let target = Url::from_file_path(&canonical)
                .map_err(|()| "workspace file path cannot be represented as a URL".to_string())?;
            let root = context
                .allowed_file_roots
                .iter()
                .find(|root| canonical.starts_with(root.as_path()))
                .cloned()
                .ok_or_else(|| "workspace file is outside every allowed root".to_string())?;
            (target.to_string(), None, Some(root))
        }
    };

    let mut allowed_origins = initial_origin.into_iter().collect::<Vec<_>>();
    for origin in policy.allowed_origins {
        let origin = validated_origin(&origin)?;
        if !allowed_origins.contains(&origin) {
            allowed_origins.push(origin);
        }
    }
    let allowed_file_roots = file_root.into_iter().collect();

    Ok(ValidatedOpen {
        identity: ResourceIdentity {
            resource_id,
            generation,
        },
        url,
        bounds,
        navigation: NavigationPolicy {
            allowed_origins,
            allowed_file_roots,
        },
        bridge: policy.bridge,
    })
}

pub(super) fn validated_shell_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("invalid shell URL: {error}"))?;
    match url.scheme() {
        "http" | "https" if !has_credentials(&url) => Ok(url),
        "file" if !has_credentials(&url) => Ok(url),
        _ => Err("shell URL must be credential-free http, https, or file".to_string()),
    }
}

/// Returns whether `source` is the exact shell document selected at startup.
///
/// A same-origin check is not sufficient for the privileged host bridge: any
/// other page on that origin could otherwise navigate the shell and issue
/// native workspace commands. Fragments are intentionally ignored because
/// they never select a different network resource, while path and query remain
/// pinned.
pub(super) fn same_shell_document(expected: &Url, source: &str) -> bool {
    let Ok(source) = Url::parse(source) else {
        return false;
    };
    match expected.scheme() {
        "http" | "https" => {
            !has_credentials(&source)
                && source.scheme() == expected.scheme()
                && source.origin() == expected.origin()
                && source.path() == expected.path()
                && source.query() == expected.query()
        }
        "file" => {
            !has_credentials(&source)
                && source.scheme() == "file"
                && source.host_str() == expected.host_str()
                && source.path() == expected.path()
                && source.query() == expected.query()
        }
        _ => false,
    }
}

fn validate_version(version: &str) -> Result<(), String> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(format!(
            "unsupported workspace protocol version {version:?}; expected {PROTOCOL_VERSION:?}"
        ))
    }
}

fn validate_resource_id(resource_id: &str) -> Result<(), String> {
    let length = resource_id.chars().count();
    if length == 0 || length > MAX_RESOURCE_ID_CHARACTERS {
        return Err(format!(
            "workspace resource id must contain between 1 and {MAX_RESOURCE_ID_CHARACTERS} characters"
        ));
    }
    if !resource_id.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err("workspace resource id contains unsupported characters".to_string());
    }
    Ok(())
}

fn validate_title(title: &str) -> Result<(), String> {
    let trimmed = title.trim();
    let length = trimmed.chars().count();
    if length == 0 || length > MAX_TITLE_CHARACTERS {
        return Err(format!(
            "workspace title must contain between 1 and {MAX_TITLE_CHARACTERS} characters"
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("workspace title must not contain control characters".to_string());
    }
    Ok(())
}

fn validate_local_app_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') || path.starts_with("//") || path.contains('\\') {
        return Err("local workspace application path must be an absolute URL path".to_string());
    }
    let path_without_query = path.split(['?', '#']).next().unwrap_or(path);
    reject_encoded_path_controls(path_without_query)?;
    if Path::new(path_without_query)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("local workspace application path must not traverse parents".to_string());
    }
    if path.chars().any(|character| character == '\0') {
        return Err(
            "local workspace application path must not contain null characters".to_string(),
        );
    }
    Ok(())
}

/// Reject encoded path separators, dot segments, NUL, and encoded percent.
///
/// URL parsers and application routers do not all decode at the same stage.
/// Rejecting `%25` also prevents a double-decoding router from turning an
/// apparently harmless `%252e%252e` segment into `..` after validation.
fn reject_encoded_path_controls(path: &str) -> Result<(), String> {
    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len() {
            return Err("local workspace application path contains malformed encoding".to_string());
        }
        let Some(high) = hex_value(bytes[index + 1]) else {
            return Err("local workspace application path contains malformed encoding".to_string());
        };
        let Some(low) = hex_value(bytes[index + 2]) else {
            return Err("local workspace application path contains malformed encoding".to_string());
        };
        let decoded = (high << 4) | low;
        if matches!(decoded, b'.' | b'/' | b'\\' | b'%' | b'\0') {
            return Err(
                "local workspace application path contains encoded traversal controls".to_string(),
            );
        }
        index += 3;
    }
    Ok(())
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn validated_remote_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("invalid workspace URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("remote workspace URL must use http or https".to_string());
    }
    if has_credentials(&url) {
        return Err("remote workspace URL must not contain credentials".to_string());
    }
    Ok(url)
}

fn validated_origin(value: &str) -> Result<String, String> {
    let url = validated_remote_url(value)?;
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(
            "allowed workspace origins must not contain a path, query, or fragment".to_string(),
        );
    }
    Ok(url.origin().ascii_serialization())
}

fn validated_file_path(value: &str, allowed_roots: &[PathBuf]) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err("workspace file path must be absolute".to_string());
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("could not resolve workspace file {value:?}: {error}"))?;
    if !canonical.is_file() {
        return Err("workspace file target must be a regular file".to_string());
    }
    if !allowed_roots.iter().any(|root| canonical.starts_with(root)) {
        return Err("workspace file is outside every allowed root".to_string());
    }
    Ok(canonical)
}

fn has_credentials(url: &Url) -> bool {
    !url.username().is_empty() || url.password().is_some()
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub(super) enum WorkspaceHostEvent<'a> {
    #[serde(rename = "workspace.host_ready", rename_all = "camelCase")]
    HostReady { version: &'static str, native: bool },
    #[serde(rename = "workspace.lifecycle", rename_all = "camelCase")]
    Lifecycle {
        version: &'static str,
        resource_id: &'a str,
        generation: u64,
        phase: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<&'a str>,
    },
    #[serde(rename = "workspace.view_message", rename_all = "camelCase")]
    ViewMessage {
        version: &'static str,
        resource_id: &'a str,
        generation: u64,
        payload: &'a Value,
    },
    #[serde(rename = "workspace.host_error", rename_all = "camelCase")]
    HostError {
        version: &'static str,
        message: &'a str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_url() -> Url {
        Url::parse("http://127.0.0.1:4180/").unwrap()
    }

    fn bounds() -> WorkspaceBounds {
        WorkspaceBounds {
            x: 400.0,
            y: 24.0,
            width: 800.0,
            height: 700.0,
        }
    }

    #[test]
    fn validates_typed_local_and_remote_targets() {
        let shell = shell_url();
        let context = ValidationContext {
            shell_url: &shell,
            allowed_file_roots: &[],
        };
        let local = validate_open(
            "office:document".to_string(),
            3,
            "Contract".to_string(),
            WorkspaceTarget::LocalApp {
                path: "/workspace/office".to_string(),
            },
            bounds(),
            WorkspacePolicy {
                bridge: BridgeMode::Typed,
                allowed_origins: vec![],
            },
            &context,
        )
        .unwrap();
        assert_eq!(local.url, "http://127.0.0.1:4180/workspace/office");
        assert!(local.navigation.allows(&local.url));
        assert!(!local.navigation.allows("javascript:alert(1)"));

        let remote = validate_open(
            "remote:docs".to_string(),
            3,
            "Docs".to_string(),
            WorkspaceTarget::Remote {
                url: "https://example.com/app".to_string(),
            },
            bounds(),
            WorkspacePolicy::default(),
            &context,
        )
        .unwrap();
        assert!(remote.navigation.allows("https://example.com/next"));
        assert!(!remote.navigation.allows("https://other.example/"));
    }

    #[test]
    fn rejects_credentials_unsafe_schemes_and_parent_traversal() {
        let shell = shell_url();
        let context = ValidationContext {
            shell_url: &shell,
            allowed_file_roots: &[],
        };
        for target in [
            WorkspaceTarget::Remote {
                url: "javascript:alert(1)".to_string(),
            },
            WorkspaceTarget::Remote {
                url: "https://user:secret@example.com/".to_string(),
            },
            WorkspaceTarget::LocalApp {
                path: "/workspace/../admin".to_string(),
            },
        ] {
            assert!(validate_open(
                "resource".to_string(),
                1,
                "Title".to_string(),
                target,
                bounds(),
                WorkspacePolicy::default(),
                &context,
            )
            .is_err());
        }
    }

    #[test]
    fn admits_files_only_below_explicit_roots() {
        let temporary =
            std::env::temp_dir().join(format!("a3s-webview-protocol-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temporary);
        std::fs::create_dir_all(&temporary).unwrap();
        let file = temporary.join("report.html");
        std::fs::write(&file, "<h1>Report</h1>").unwrap();
        let root = temporary.canonicalize().unwrap();
        let shell = shell_url();
        let context = ValidationContext {
            shell_url: &shell,
            allowed_file_roots: std::slice::from_ref(&root),
        };
        let open = validate_open(
            "local-report".to_string(),
            1,
            "Report".to_string(),
            WorkspaceTarget::File {
                path: file.to_string_lossy().into_owned(),
            },
            bounds(),
            WorkspacePolicy::default(),
            &context,
        )
        .unwrap();
        assert!(open.navigation.allows(&open.url));
        std::fs::remove_dir_all(temporary).unwrap();
    }

    #[test]
    fn command_parser_rejects_stale_protocol_shapes() {
        let valid = serde_json::json!({
            "type": "workspace.bounds",
            "version": PROTOCOL_VERSION,
            "resourceId": "draft-1",
            "generation": 4,
            "bounds": bounds(),
        });
        assert!(WorkspaceCommand::parse(&valid.to_string()).is_ok());

        let mut unknown = valid.clone();
        unknown["unexpected"] = Value::Bool(true);
        assert!(WorkspaceCommand::parse(&unknown.to_string()).is_err());

        let mut old = valid;
        old["version"] = Value::String("a3s.workspace.v0".to_string());
        assert!(WorkspaceCommand::parse(&old.to_string()).is_err());
    }

    #[test]
    fn parses_generation_scoped_occlusion_commands() {
        let command = serde_json::json!({
            "type": "workspace.occlusion",
            "version": PROTOCOL_VERSION,
            "resourceId": "draft-1",
            "generation": 4,
            "occluded": true,
        });
        assert!(matches!(
            WorkspaceCommand::parse(&command.to_string()).unwrap(),
            WorkspaceCommand::Occlusion {
                resource_id,
                generation: 4,
                occluded: true,
                ..
            } if resource_id == "draft-1"
        ));

        let mut missing_state = command.clone();
        missing_state.as_object_mut().unwrap().remove("occluded");
        assert!(WorkspaceCommand::parse(&missing_state.to_string()).is_err());

        let mut stale_generation = command;
        stale_generation["generation"] = Value::from(0);
        assert!(WorkspaceCommand::parse(&stale_generation.to_string()).is_err());
    }

    #[test]
    fn pins_shell_ipc_to_the_initial_document() {
        let shell = Url::parse("http://127.0.0.1:4180/avatar?mode=workspace").unwrap();
        assert!(same_shell_document(
            &shell,
            "http://127.0.0.1:4180/avatar?mode=workspace#active"
        ));
        assert!(!same_shell_document(
            &shell,
            "http://127.0.0.1:4180/workspace?mode=workspace"
        ));
        assert!(!same_shell_document(
            &shell,
            "http://127.0.0.1:4180/avatar?mode=other"
        ));
        assert!(!same_shell_document(
            &shell,
            "http://localhost:4180/avatar?mode=workspace"
        ));
        assert!(!same_shell_document(
            &shell,
            "https://127.0.0.1:4180/avatar?mode=workspace"
        ));
    }

    #[test]
    fn rejects_encoded_and_double_encoded_local_path_traversal() {
        let shell = shell_url();
        let context = ValidationContext {
            shell_url: &shell,
            allowed_file_roots: &[],
        };
        for path in [
            "/workspace/%2e%2e/admin",
            "/workspace/%2Fadmin",
            "/workspace/%5cadmin",
            "/workspace/%252e%252e/admin",
            "/workspace/%00admin",
            "/workspace/%zz/admin",
        ] {
            assert!(
                validate_open(
                    "resource".to_string(),
                    1,
                    "Title".to_string(),
                    WorkspaceTarget::LocalApp {
                        path: path.to_string(),
                    },
                    bounds(),
                    WorkspacePolicy::default(),
                    &context,
                )
                .is_err(),
                "unsafe path should be rejected: {path}"
            );
        }

        let safe = validate_open(
            "resource".to_string(),
            1,
            "Title".to_string(),
            WorkspaceTarget::LocalApp {
                path: "/workspace/r%C3%A9sum%C3%A9?mode=edit".to_string(),
            },
            bounds(),
            WorkspacePolicy::default(),
            &context,
        );
        assert!(safe.is_ok());
    }
}
