use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::control::{
    AgentControlActionKind, AuthorizedControl, ControlDescriptor, ControlSubmission,
};

pub(crate) const SNAPSHOT_SCHEMA: &str = "a3s.system_agent_snapshot.v1";
pub(crate) const MAX_SNAPSHOT_BYTES: u64 = 1024 * 1024;
pub(crate) const MAX_ACTIVITIES: usize = 256;
pub(crate) const SNAPSHOT_FRESH_MS: u64 = 10_000;
const SNAPSHOT_FUTURE_SKEW_MS: u64 = 5_000;
const MAX_ID_CHARS: usize = 160;
const MAX_AGENT_CHARS: usize = 64;
const MAX_WORKSPACE_CHARS: usize = 128;
const MAX_TASK_CHARS: usize = 240;
const MAX_REASON_CHARS: usize = 240;
const MAX_CONTROL_ACTIONS: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentState {
    Planning,
    Working,
    WaitingApproval,
    WaitingInput,
    Idle,
    Completed,
    Failed,
    Cancelled,
    #[default]
    #[serde(other)]
    Unknown,
}

impl AgentState {
    pub(crate) fn presentation(self) -> StatePresentation {
        match self {
            Self::Planning => StatePresentation::new("Planning", "planning", "◇"),
            Self::Working => StatePresentation::new("Working", "working", "●"),
            Self::WaitingApproval => StatePresentation::new("Approval needed", "attention", "!"),
            Self::WaitingInput => StatePresentation::new("Input needed", "attention", "?"),
            Self::Idle => StatePresentation::new("Idle", "idle", "–"),
            Self::Completed => StatePresentation::new("Completed", "success", "✓"),
            Self::Failed => StatePresentation::new("Failed", "danger", "×"),
            Self::Cancelled => StatePresentation::new("Cancelled", "cancelled", "–"),
            Self::Unknown => StatePresentation::new("Process detected", "inferred", "○"),
        }
    }

    fn attention_rank(self) -> u8 {
        match self {
            Self::WaitingApproval | Self::WaitingInput => 0,
            Self::Failed => 1,
            Self::Planning | Self::Working => 2,
            Self::Cancelled => 3,
            Self::Unknown => 4,
            Self::Idle => 5,
            Self::Completed => 6,
        }
    }

    fn keeps_island_visible(self) -> bool {
        self != Self::Idle
    }

    fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    fn is_active_work(self) -> bool {
        matches!(self, Self::Planning | Self::Working)
    }

    fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::WaitingApproval | Self::WaitingInput | Self::Failed
        )
    }

    fn is_recent(self) -> bool {
        self.is_terminal()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StatePresentation {
    pub(crate) label: &'static str,
    pub(crate) tone: &'static str,
    pub(crate) glyph: &'static str,
}

impl StatePresentation {
    const fn new(label: &'static str, tone: &'static str, glyph: &'static str) -> Self {
        Self { label, tone, glyph }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentConfidence {
    Exact,
    Process,
    #[default]
    #[serde(other)]
    Unknown,
}

impl AgentConfidence {
    fn evidence_label(self) -> Option<&'static str> {
        match self {
            Self::Exact => None,
            Self::Process => Some("detected / process"),
            Self::Unknown => Some("detected / unknown"),
        }
    }

    fn evidence_rank(self) -> u8 {
        match self {
            Self::Exact => 0,
            Self::Process => 1,
            Self::Unknown => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentVendor {
    A3s,
    OpenAi,
    Anthropic,
    Google,
    Cursor,
    Moonshot,
    Tencent,
    Alibaba,
    DeepSeek,
    Mistral,
    #[default]
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Activity {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) parent_id: Option<String>,
    #[serde(default)]
    pub(crate) agent: String,
    #[serde(default)]
    pub(crate) workspace: Option<String>,
    #[serde(default)]
    pub(crate) task: Option<String>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
    #[serde(default)]
    pub(crate) state: AgentState,
    #[serde(default)]
    pub(crate) confidence: AgentConfidence,
    #[serde(default)]
    pub(crate) vendor: AgentVendor,
    #[serde(default)]
    pub(crate) started_at_ms: Option<u64>,
    #[serde(default)]
    pub(crate) finished_at_ms: Option<u64>,
    #[serde(default)]
    pub(crate) expires_at_ms: Option<u64>,
    #[serde(default)]
    pub(crate) actions: Vec<ControlDescriptor>,
}

impl Activity {
    fn sanitize(mut self, now_ms: u64, evidence_at_ms: u64) -> Option<Self> {
        self.id = sanitize_text(&self.id, MAX_ID_CHARS);
        if self.id.is_empty() {
            return None;
        }
        self.parent_id = sanitize_optional(self.parent_id, MAX_ID_CHARS);
        self.agent = sanitize_text(&self.agent, MAX_AGENT_CHARS);
        if self.agent.is_empty() {
            self.agent = "agent".to_string();
        }
        self.workspace = sanitize_optional(self.workspace, MAX_WORKSPACE_CHARS);
        self.task = sanitize_optional(self.task, MAX_TASK_CHARS);
        self.reason = sanitize_optional(self.reason, MAX_REASON_CHARS);
        let latest_time = now_ms.saturating_add(SNAPSHOT_FUTURE_SKEW_MS);
        self.started_at_ms = self.started_at_ms.filter(|started| *started <= latest_time);
        self.finished_at_ms = self
            .finished_at_ms
            .filter(|finished| *finished <= latest_time);
        if self.state.is_terminal() && self.finished_at_ms.is_none() {
            self.finished_at_ms = Some(evidence_at_ms.min(latest_time));
        }
        if let (Some(started), Some(finished)) = (self.started_at_ms, self.finished_at_ms) {
            self.finished_at_ms = Some(finished.max(started));
        }
        let mut seen = std::collections::HashSet::new();
        self.actions = self
            .actions
            .into_iter()
            .filter_map(|action| action.sanitize(now_ms))
            .filter(|action| seen.insert(action.action))
            .take(MAX_CONTROL_ACTIONS)
            .collect();
        if self.confidence != AgentConfidence::Exact {
            self.actions.clear();
        }
        Some(self)
    }

    fn categories(&self) -> Vec<ActivityCategory> {
        if self.confidence == AgentConfidence::Process {
            return vec![ActivityCategory::Running];
        }
        if self.confidence != AgentConfidence::Exact {
            return Vec::new();
        }
        let mut categories = Vec::with_capacity(2);
        if self.state.needs_attention() {
            categories.push(ActivityCategory::NeedsAttention);
        }
        if self.state.is_active_work() {
            categories.push(ActivityCategory::Running);
        }
        if self.state.is_recent() {
            categories.push(ActivityCategory::Recent);
        }
        categories
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ActivityCategory {
    NeedsAttention,
    Running,
    Recent,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct Snapshot {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    pub(crate) updated_at_ms: u64,
    #[serde(default)]
    pub(crate) degraded: bool,
    #[serde(default)]
    pub(crate) activities: Vec<Activity>,
}

impl Snapshot {
    pub(crate) fn parse(bytes: &[u8], now_ms: u64) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err("snapshot exceeds the 1 MiB size limit".to_string());
        }
        let mut snapshot: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid snapshot JSON: {error}"))?;
        if snapshot.schema != SNAPSHOT_SCHEMA {
            return Err("unsupported snapshot schema".to_string());
        }
        if snapshot.updated_at_ms > now_ms.saturating_add(SNAPSHOT_FUTURE_SKEW_MS) {
            return Err("snapshot timestamp is too far in the future".to_string());
        }
        let legacy_expiry = snapshot.updated_at_ms.saturating_add(SNAPSHOT_FRESH_MS);
        let evidence_at_ms = snapshot.updated_at_ms;
        snapshot.activities.truncate(MAX_ACTIVITIES);
        snapshot.activities = snapshot
            .activities
            .into_iter()
            .filter_map(|activity| activity.sanitize(now_ms, evidence_at_ms))
            .filter(|activity| activity.expires_at_ms.unwrap_or(legacy_expiry) >= now_ms)
            .collect();
        Ok(snapshot)
    }

    pub(crate) fn is_fresh(&self, now_ms: u64) -> bool {
        self.updated_at_ms <= now_ms.saturating_add(SNAPSHOT_FUTURE_SKEW_MS)
            && now_ms.saturating_sub(self.updated_at_ms) <= SNAPSHOT_FRESH_MS
    }

    pub(crate) fn render_json(&self) -> Result<String, String> {
        let primary = self.activities.iter().min_by_key(|activity| {
            (
                activity.state.attention_rank(),
                activity.confidence.evidence_rank(),
            )
        });
        let (headline, detail) = presentation_text(primary);
        let primary_status = primary
            .map(|activity| activity.state.presentation())
            .unwrap_or_else(|| AgentState::Idle.presentation());
        let primary_vendor = primary.map_or(AgentVendor::Other, |activity| activity.vendor);
        let active_work = self.activities.iter().any(|activity| {
            activity.confidence == AgentConfidence::Process
                || (activity.confidence == AgentConfidence::Exact
                    && activity.state.is_active_work())
        });
        let metrics = RenderMetrics::from_activities(&self.activities);
        let attention_keys = attention_keys(&self.activities);
        let primary_child_progress =
            primary.and_then(|activity| child_progress(&self.activities, &activity.id));
        let activities = self
            .activities
            .iter()
            .map(|activity| {
                let status = activity.state.presentation();
                RenderActivity {
                    id: &activity.id,
                    parent_id: activity.parent_id.as_deref(),
                    agent: &activity.agent,
                    workspace: activity.workspace.as_deref(),
                    task: activity.task.as_deref(),
                    reason: activity.reason.as_deref(),
                    state: activity.state,
                    vendor: activity.vendor,
                    status: status.label,
                    tone: status.tone,
                    glyph: status.glyph,
                    inferred: activity.confidence != AgentConfidence::Exact,
                    evidence: activity.confidence.evidence_label(),
                    categories: activity.categories(),
                    child_progress: child_progress(&self.activities, &activity.id),
                    started_at_ms: activity.started_at_ms,
                    finished_at_ms: activity.finished_at_ms,
                    controls: activity
                        .actions
                        .iter()
                        .map(|control| RenderControl {
                            action: control.action,
                            label: control.action.label(),
                            tone: control.action.tone(),
                            token: &control.token,
                            target_instance_id: &control.target_instance_id,
                            expires_at_ms: control.expires_at_ms,
                        })
                        .collect(),
                }
            })
            .collect();
        serde_json::to_string(&RenderSnapshot {
            updated_at_ms: self.updated_at_ms,
            degraded: self.degraded,
            headline,
            detail,
            primary_agent: primary.map(|activity| activity.agent.as_str()),
            primary_workspace: primary.and_then(|activity| activity.workspace.as_deref()),
            primary_reason: primary.and_then(|activity| activity.reason.as_deref()),
            primary_inferred: primary
                .is_some_and(|activity| activity.confidence != AgentConfidence::Exact),
            primary_child_progress,
            status: primary_status.label,
            tone: primary_status.tone,
            glyph: primary_status.glyph,
            vendor: primary_vendor,
            primary_started_at_ms: primary.and_then(|activity| activity.started_at_ms),
            primary_finished_at_ms: primary.and_then(|activity| activity.finished_at_ms),
            active_work,
            metrics,
            attention_keys,
            activities,
        })
        .map_err(|error| format!("serialize island snapshot: {error}"))
    }

    pub(crate) fn has_visible_activity(&self) -> bool {
        self.activities.iter().any(|activity| {
            activity.confidence == AgentConfidence::Process
                || (activity.confidence == AgentConfidence::Exact
                    && activity.state.keeps_island_visible())
        })
    }

    pub(crate) fn authorize_control(
        &self,
        submission: &ControlSubmission,
        now_ms: u64,
    ) -> Option<AuthorizedControl> {
        let activity = self
            .activities
            .iter()
            .find(|activity| activity.id == submission.activity_id)?;
        if activity.confidence != AgentConfidence::Exact {
            return None;
        }
        let control = activity.actions.iter().find(|control| {
            control.action == submission.action
                && control.token == submission.token
                && control.target_instance_id == submission.target_instance_id
                && control.expires_at_ms >= now_ms
        })?;
        Some(AuthorizedControl {
            activity_id: activity.id.clone(),
            action: control.action,
            message: submission.message.clone(),
            token: control.token.clone(),
            target_instance_id: control.target_instance_id.clone(),
            expires_at_ms: control.expires_at_ms,
        })
    }

    pub(crate) fn render_empty_json(updated_at_ms: u64, degraded: bool) -> Result<String, String> {
        Self {
            schema: SNAPSHOT_SCHEMA.to_string(),
            updated_at_ms,
            degraded,
            activities: Vec::new(),
        }
        .render_json()
    }
}

#[derive(Serialize)]
struct RenderSnapshot<'a> {
    updated_at_ms: u64,
    degraded: bool,
    headline: String,
    detail: String,
    primary_agent: Option<&'a str>,
    primary_workspace: Option<&'a str>,
    primary_reason: Option<&'a str>,
    primary_inferred: bool,
    primary_child_progress: Option<RenderChildProgress>,
    status: &'static str,
    tone: &'static str,
    glyph: &'static str,
    vendor: AgentVendor,
    primary_started_at_ms: Option<u64>,
    primary_finished_at_ms: Option<u64>,
    active_work: bool,
    metrics: RenderMetrics,
    attention_keys: Vec<String>,
    activities: Vec<RenderActivity<'a>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
struct RenderMetrics {
    total: usize,
    needs_attention: usize,
    running: usize,
    recent: usize,
    inferred: usize,
}

impl RenderMetrics {
    fn from_activities(activities: &[Activity]) -> Self {
        let mut metrics = Self {
            total: activities.len(),
            ..Self::default()
        };
        for activity in activities {
            if activity.confidence != AgentConfidence::Exact {
                metrics.inferred += 1;
            }
            for category in activity.categories() {
                match category {
                    ActivityCategory::NeedsAttention => metrics.needs_attention += 1,
                    ActivityCategory::Running => metrics.running += 1,
                    ActivityCategory::Recent => metrics.recent += 1,
                }
            }
        }
        metrics
    }
}

#[derive(Serialize)]
struct RenderActivity<'a> {
    id: &'a str,
    parent_id: Option<&'a str>,
    agent: &'a str,
    workspace: Option<&'a str>,
    task: Option<&'a str>,
    reason: Option<&'a str>,
    state: AgentState,
    vendor: AgentVendor,
    status: &'static str,
    tone: &'static str,
    glyph: &'static str,
    inferred: bool,
    evidence: Option<&'static str>,
    categories: Vec<ActivityCategory>,
    child_progress: Option<RenderChildProgress>,
    started_at_ms: Option<u64>,
    finished_at_ms: Option<u64>,
    controls: Vec<RenderControl<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct RenderChildProgress {
    settled: usize,
    total: usize,
}

#[derive(Serialize)]
struct RenderControl<'a> {
    action: AgentControlActionKind,
    label: &'static str,
    tone: &'static str,
    token: &'a str,
    target_instance_id: &'a str,
    expires_at_ms: u64,
}

fn child_progress(activities: &[Activity], parent_id: &str) -> Option<RenderChildProgress> {
    let children = activities.iter().filter(|activity| {
        activity.confidence == AgentConfidence::Exact
            && activity.parent_id.as_deref() == Some(parent_id)
    });
    let mut total = 0usize;
    let mut settled = 0usize;
    for child in children {
        total += 1;
        settled += usize::from(child.state.is_terminal());
    }
    (total > 0).then_some(RenderChildProgress { settled, total })
}

fn attention_keys(activities: &[Activity]) -> Vec<String> {
    let mut keys = Vec::new();
    for activity in activities.iter().filter(|activity| {
        activity.confidence == AgentConfidence::Exact
            && matches!(
                activity.state,
                AgentState::WaitingApproval | AgentState::WaitingInput
            )
    }) {
        match activity.state {
            AgentState::WaitingApproval => {
                keys.extend(
                    activity
                        .actions
                        .iter()
                        .filter(|action| {
                            matches!(
                                action.action,
                                AgentControlActionKind::ApproveOnce
                                    | AgentControlActionKind::ApproveAlways
                                    | AgentControlActionKind::Deny
                            )
                        })
                        .map(|action| attention_key(activity, Some(action.token.as_str()))),
                );
            }
            AgentState::WaitingInput => keys.push(attention_key(activity, None)),
            _ => {}
        }
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

fn attention_key(activity: &Activity, token: Option<&str>) -> String {
    // FNV-1a is deliberately used as a compact, deterministic UI identity.
    // This key is not an authorization primitive; controls still require the
    // full short-lived token and are revalidated by the owning TUI.
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    let state = match activity.state {
        AgentState::WaitingApproval => "approval",
        AgentState::WaitingInput => "input",
        _ => "other",
    };
    let request_identity = token.or(activity.task.as_deref()).unwrap_or_default();
    for part in [state, activity.id.as_str(), request_identity] {
        for byte in part.as_bytes().iter().copied().chain([0xff]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    format!("attention-v1-{hash:016x}")
}

fn presentation_text(primary: Option<&Activity>) -> (String, String) {
    let Some(primary) = primary else {
        return (
            "No active agents".to_string(),
            "Waiting for activity".to_string(),
        );
    };
    let state = primary.state.presentation().label;
    let headline = primary
        .task
        .clone()
        .unwrap_or_else(|| primary.agent.clone());
    let detail = primary
        .workspace
        .clone()
        .unwrap_or_else(|| state.to_string());
    (headline, detail)
}

pub(crate) fn read_snapshot(path: &Path, now_ms: u64) -> Result<Snapshot, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "snapshot has no parent directory".to_string())?;
    super::singleton::validate_private_directory(parent)?;
    let path_metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("open snapshot metadata: {error}"))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err("snapshot path is not a regular file".to_string());
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("open snapshot: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("read snapshot metadata: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_SNAPSHOT_BYTES {
        return Err("snapshot is not a bounded regular file".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err("snapshot must be owned by the current user".to_string());
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("snapshot permissions must not grant group or other access".to_string());
        }
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_SNAPSHOT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read snapshot: {error}"))?;
    Snapshot::parse(&bytes, now_ms)
}

fn sanitize_optional(value: Option<String>, max_chars: usize) -> Option<String> {
    value
        .map(|value| sanitize_text(&value, max_chars))
        .filter(|value| !value.is_empty())
}

fn sanitize_text(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    let mut pending_space = false;
    let mut count = 0usize;
    for character in value.chars() {
        let bidi_control = matches!(
            character,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        );
        if character.is_control() || bidi_control {
            pending_space |= character.is_whitespace();
            continue;
        }
        if character.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if count == max_chars {
            break;
        }
        if pending_space && count < max_chars {
            output.push(' ');
            count += 1;
        }
        pending_space = false;
        if count == max_chars {
            break;
        }
        output.push(character);
        count += 1;
    }
    output
}

#[cfg(test)]
#[path = "status_product_tests.rs"]
mod product_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_json(updated_at_ms: u64, activities: &str) -> Vec<u8> {
        format!(
            r#"{{"schema":"{SNAPSHOT_SCHEMA}","updated_at_ms":{updated_at_ms},"degraded":false,"activities":{activities}}}"#
        )
        .into_bytes()
    }

    #[test]
    fn parses_all_states_and_maps_presentations() {
        let states = [
            ("planning", "Planning", "planning"),
            ("working", "Working", "working"),
            ("waiting_approval", "Approval needed", "attention"),
            ("waiting_input", "Input needed", "attention"),
            ("idle", "Idle", "idle"),
            ("completed", "Completed", "success"),
            ("failed", "Failed", "danger"),
            ("cancelled", "Cancelled", "cancelled"),
            ("unknown", "Process detected", "inferred"),
        ];
        for (state, label, tone) in states {
            let activities = format!(
                r#"[{{"id":"one","agent":"a3s-code","state":"{state}","confidence":"exact"}}]"#
            );
            let snapshot = Snapshot::parse(&snapshot_json(10_000, &activities), 10_000).unwrap();
            let mapped = snapshot.activities[0].state.presentation();
            assert_eq!((mapped.label, mapped.tone), (label, tone));
        }
    }

    #[test]
    fn unknown_future_state_degrades_to_process_detected() {
        let bytes = snapshot_json(
            10_000,
            r#"[{"id":"one","agent":"future","state":"teleporting","confidence":"future"}]"#,
        );
        let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
        assert_eq!(snapshot.activities[0].state, AgentState::Unknown);
        assert_eq!(snapshot.activities[0].confidence, AgentConfidence::Unknown);
    }

    #[test]
    fn enforces_schema_size_and_future_timestamp() {
        let wrong = br#"{"schema":"wrong","updated_at_ms":1,"activities":[]}"#;
        assert!(Snapshot::parse(wrong, 1).is_err());
        assert!(Snapshot::parse(&vec![b' '; MAX_SNAPSHOT_BYTES as usize + 1], 1).is_err());
        assert!(Snapshot::parse(&snapshot_json(10_001, "[]"), 5_000).is_err());
    }

    #[test]
    fn freshness_has_ten_second_ttl() {
        let snapshot = Snapshot::parse(&snapshot_json(20_000, "[]"), 20_000).unwrap();
        assert!(snapshot.is_fresh(30_000));
        assert!(!snapshot.is_fresh(30_001));
    }

    #[test]
    fn expired_rows_are_removed_before_the_snapshot_ttl() {
        let snapshot = Snapshot::parse(
            &snapshot_json(
                20_000,
                r#"[{"id":"old","agent":"codex","state":"working","confidence":"exact","expires_at_ms":21000},{"id":"current","agent":"a3s-code","state":"idle","confidence":"exact","expires_at_ms":30000}]"#,
            ),
            21_001,
        )
        .unwrap();

        assert!(snapshot.is_fresh(21_001));
        assert_eq!(snapshot.activities.len(), 1);
        assert_eq!(snapshot.activities[0].id, "current");
    }

    #[test]
    fn bounds_rows_and_sanitizes_unicode_labels() {
        let rows = (0..300)
            .map(|index| {
                format!(
                    r#"{{"id":"id-{index}","agent":"代理🤖\n\u202eevil","task":"{}","state":"working","confidence":"exact"}}"#,
                    "界".repeat(300)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let bytes = snapshot_json(10_000, &format!("[{rows}]"));
        let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
        assert_eq!(snapshot.activities.len(), MAX_ACTIVITIES);
        assert_eq!(snapshot.activities[0].agent, "代理🤖 evil");
        assert_eq!(
            snapshot.activities[0]
                .task
                .as_ref()
                .unwrap()
                .chars()
                .count(),
            MAX_TASK_CHARS
        );
    }

    #[test]
    fn rendered_payload_contains_only_mapped_bounded_data() {
        let bytes = snapshot_json(
            10_000,
            r#"[{"id":"one","agent":"a3s-code","workspace":"repo","task":"Ship it","state":"completed","confidence":"exact"}]"#,
        );
        let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
        let rendered: serde_json::Value =
            serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();
        assert_eq!(rendered["headline"], "Ship it");
        assert_eq!(rendered["primary_agent"], "a3s-code");
        assert_eq!(rendered["status"], "Completed");
        assert!(rendered["primary_started_at_ms"].is_null());
        assert_eq!(rendered["primary_finished_at_ms"], 10_000);
        assert_eq!(rendered["activities"][0]["status"], "Completed");
        assert_eq!(rendered["activities"][0]["tone"], "success");
    }

    #[test]
    fn rendered_process_evidence_is_explicit() {
        let bytes = snapshot_json(
            10_000,
            r#"[{"id":"one","agent":"codex","task":"active process","state":"unknown","confidence":"process"}]"#,
        );
        let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
        let rendered: serde_json::Value =
            serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();

        assert_eq!(rendered["activities"][0]["evidence"], "detected / process");
    }

    #[test]
    fn rendered_work_rows_include_vendor_time_controls_and_neon_signal() {
        let bytes = snapshot_json(
            10_000,
            r#"[{"id":"parent","agent":"a3s-code","state":"working","confidence":"exact","vendor":"open_ai","started_at_ms":4000,"actions":[{"action":"stop","token":"0123456789abcdef0123456789abcdef","target_instance_id":"parent","expires_at_ms":20000}]}]"#,
        );
        let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
        let rendered: serde_json::Value =
            serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();

        assert_eq!(rendered["active_work"], true);
        assert_eq!(rendered["vendor"], "open_ai");
        assert_eq!(rendered["primary_agent"], "a3s-code");
        assert_eq!(rendered["status"], "Working");
        assert_eq!(rendered["primary_started_at_ms"], 4_000);
        assert!(rendered["primary_finished_at_ms"].is_null());
        assert_eq!(rendered["activities"][0]["started_at_ms"], 4_000);
        assert_eq!(rendered["activities"][0]["vendor"], "open_ai");
        assert_eq!(rendered["activities"][0]["controls"][0]["action"], "stop");
        assert_eq!(rendered["activities"][0]["controls"][0]["label"], "Stop");

        let submission = ControlSubmission {
            activity_id: "parent".to_string(),
            action: AgentControlActionKind::Stop,
            message: None,
            token: "0123456789abcdef0123456789abcdef".to_string(),
            target_instance_id: "parent".to_string(),
        };
        assert!(snapshot.authorize_control(&submission, 10_000).is_some());
        assert!(snapshot.authorize_control(&submission, 20_001).is_none());
    }

    #[test]
    fn terminal_duration_freezes_and_inferred_rows_cannot_expose_controls() {
        let bytes = snapshot_json(
            10_000,
            r#"[{"id":"done","agent":"claude","state":"completed","confidence":"exact","vendor":"anthropic","started_at_ms":4000},{"id":"process","agent":"codex","state":"working","confidence":"process","actions":[{"action":"stop","token":"0123456789abcdef0123456789abcdef","target_instance_id":"process","expires_at_ms":20000}]}]"#,
        );
        let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
        let rendered: serde_json::Value =
            serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();

        assert_eq!(rendered["activities"][0]["finished_at_ms"], 10_000);
        assert!(rendered["activities"][1]["controls"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_live_external_process_outranks_a_retained_terminal_outcome() {
        let bytes = snapshot_json(
            10_000,
            r#"[{"id":"process","agent":"codex","task":"active process","state":"unknown","confidence":"process"},{"id":"exact","agent":"a3s-code","task":"Task complete","state":"completed","confidence":"exact"}]"#,
        );
        let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
        let rendered: serde_json::Value =
            serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();

        assert_eq!(rendered["headline"], "active process");
        assert_eq!(rendered["detail"], "Process detected");
        assert_eq!(rendered["tone"], "inferred");
        assert!(snapshot.has_visible_activity());
    }

    #[test]
    fn a_detected_process_keeps_the_island_visible_when_a3s_is_idle() {
        let bytes = snapshot_json(
            10_000,
            r#"[{"id":"idle","agent":"a3s-code","state":"idle","confidence":"exact"},{"id":"process","agent":"codex","state":"unknown","confidence":"process"}]"#,
        );
        let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();

        assert!(snapshot.has_visible_activity());
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_reader_requires_private_directory_and_file() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "a3s-webview-snapshot-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("system-snapshot.json");
        std::fs::write(&path, snapshot_json(10_000, "[]")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        assert!(read_snapshot(&path, 10_000).is_ok());

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let file_error = read_snapshot(&path, 10_000).unwrap_err();
        assert!(file_error.contains("snapshot permissions"), "{file_error}");

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        let directory_error = read_snapshot(&path, 10_000).unwrap_err();
        assert!(
            directory_error.contains("directory permissions"),
            "{directory_error}"
        );

        std::fs::remove_dir_all(directory).unwrap();
    }
}
