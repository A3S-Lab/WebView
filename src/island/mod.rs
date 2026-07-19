mod control;
mod html;
mod preference;
mod singleton;
mod status;
mod window;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use wry::{BackgroundThrottlingPolicy, WebViewBuilder};

use self::singleton::IslandLock;
use self::status::{read_snapshot, Snapshot, SNAPSHOT_FRESH_MS};
use self::window::IslandSize;

pub(crate) const USAGE: &str =
    "usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>";
const POLL_INTERVAL: Duration = Duration::from_millis(1_000);
const RECENTER_INTERVAL: Duration = Duration::from_secs(2);
const SHUTDOWN_GRACE_MS: u64 = 20_000;

#[derive(Clone, Debug, PartialEq, Eq)]
struct IslandArgs {
    snapshot: PathBuf,
    lock_file: PathBuf,
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<IslandArgs, String> {
    let mut snapshot = None;
    let mut lock_file = None;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut next = || it.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--snapshot" => snapshot = Some(PathBuf::from(next()?)),
            "--lock-file" => lock_file = Some(PathBuf::from(next()?)),
            other => return Err(format!("unknown agent-island argument: {other}")),
        }
    }
    let snapshot = snapshot.ok_or("--snapshot is required")?;
    let lock_file = lock_file.ok_or("--lock-file is required")?;
    if !snapshot.is_absolute() || !lock_file.is_absolute() {
        return Err("agent-island paths must be absolute".to_string());
    }
    if snapshot == lock_file {
        return Err("snapshot and lock paths must be different".to_string());
    }
    if snapshot.parent() != lock_file.parent() {
        return Err("snapshot and lock paths must share one private directory".to_string());
    }
    Ok(IslandArgs {
        snapshot,
        lock_file,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum IslandEvent {
    Ready,
    Expand,
    CollapseComplete,
    Disable,
    Control(control::ControlSubmission),
}

fn parse_ipc(body: &str) -> Option<IslandEvent> {
    match body {
        "ready" => Some(IslandEvent::Ready),
        "expand" => Some(IslandEvent::Expand),
        "collapse-complete" => Some(IslandEvent::CollapseComplete),
        "disable" => Some(IslandEvent::Disable),
        _ => control::parse_submission(body).map(IslandEvent::Control),
    }
}

pub(crate) fn run<I: IntoIterator<Item = String>>(args: I) -> Result<(), String> {
    let args = parse_args(args)?;
    if preference::is_disabled_for_snapshot(&args.snapshot) {
        return Ok(());
    }
    let Some(singleton) = IslandLock::acquire(&args.lock_file)? else {
        // Every TUI may race to start the helper. Lock contention proves that
        // the per-user island already exists, so this helper exits successfully.
        return Ok(());
    };
    window::warn_if_wayland_positioning_is_degraded();

    let mut event_loop = EventLoopBuilder::<IslandEvent>::with_user_event().build();
    window::configure_event_loop(&mut event_loop);
    let window = window::create_window(&event_loop)?;
    window::configure_native_window(&window, false)?;
    window::resize_and_center(&window, IslandSize::Collapsed);

    let proxy = event_loop.create_proxy();
    let builder = WebViewBuilder::new()
        .with_html(html::island_html())
        .with_transparent(true)
        .with_accept_first_mouse(true)
        .with_focused(false)
        // The island intentionally never activates or becomes key. Keep its
        // document resident during long tasks; the page separately bypasses
        // timeline-based transitions whenever WebKit reports it as hidden.
        .with_background_throttling(BackgroundThrottlingPolicy::Disabled)
        .with_ipc_handler(move |request| {
            if let Some(event) = parse_ipc(request.body()) {
                let _ = proxy.send_event(event);
            }
        });
    let webview = build_webview(builder, &window)?;
    // Wry may adjust its host frame while attaching WKWebView. Reassert the
    // exact physical top-center frame after attachment and before first show.
    window::resize_and_center(&window, IslandSize::Collapsed);
    sync_webview_bounds(&webview, &window)?;
    window::configure_native_window(&window, false)?;
    window::show_without_focus(&window);

    let started = Instant::now();
    let preference_snapshot = args.snapshot.clone();
    let mut snapshots = SnapshotRuntime::new(args.snapshot, started)?;
    let mut next_poll = Instant::now();
    let mut next_recenter = Instant::now() + RECENTER_INTERVAL;
    let mut web_ready = false;
    let mut expanded = false;
    let singleton_guard = singleton;

    event_loop.run(move |event, _, control_flow| {
        let now = Instant::now();
        *control_flow = ControlFlow::WaitUntil(next_poll.min(next_recenter));
        // Keep the advisory lock alive for the entire native event loop.
        let _ = &singleton_guard;

        match event {
            Event::UserEvent(IslandEvent::Ready) => {
                web_ready = true;
                if preference::is_disabled_for_snapshot(&preference_snapshot)
                    || snapshots.poll(&webview, true, now)
                {
                    *control_flow = ControlFlow::Exit;
                }
                next_poll = now + POLL_INTERVAL;
            }
            Event::UserEvent(IslandEvent::Expand) if !expanded => {
                expanded = true;
                window::resize_and_center(&window, IslandSize::Expanded);
                if let Err(error) = sync_webview_bounds(&webview, &window)
                    .and_then(|()| window::configure_native_window(&window, true))
                {
                    eprintln!("a3s-webview: agent island: {error}");
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                let _ = webview
                    .evaluate_script("window.a3sIsland && window.a3sIsland.setExpanded(true);");
            }
            Event::UserEvent(IslandEvent::CollapseComplete) => {
                if expanded {
                    expanded = false;
                    window::resize_and_center(&window, IslandSize::Collapsed);
                    if let Err(error) = sync_webview_bounds(&webview, &window)
                        .and_then(|()| window::configure_native_window(&window, false))
                    {
                        eprintln!("a3s-webview: agent island: {error}");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
                // Keep the page transition locked until native resizing has
                // completed. This acknowledgement prevents a rapid second
                // click from posting an Expand that native state must ignore.
                let _ = webview
                    .evaluate_script("window.a3sIsland && window.a3sIsland.finishCollapse();");
            }
            Event::UserEvent(IslandEvent::Disable) => {
                match preference::disable_for_snapshot(&preference_snapshot) {
                    Ok(()) => {
                        *control_flow = ControlFlow::Exit;
                    }
                    Err(error) => {
                        eprintln!("a3s-webview: agent island: {error}");
                        let _ = webview.evaluate_script(
                            "window.a3sIsland && window.a3sIsland.disableResult(false);",
                        );
                    }
                }
            }
            Event::UserEvent(IslandEvent::Control(submission)) => {
                let activity_id = submission.activity_id.clone();
                let action = submission.action;
                let result = snapshots.submit_control(submission, epoch_ms());
                let (accepted, message) = match result {
                    Ok(()) => (
                        true,
                        if action == control::AgentControlActionKind::Reply {
                            "Queued"
                        } else {
                            "Sent"
                        }
                        .to_string(),
                    ),
                    Err(error) => {
                        snapshots.report_error(error.clone());
                        (false, "Try again".to_string())
                    }
                };
                match control_result_script(&activity_id, action, accepted, &message).and_then(
                    |script| {
                        webview
                            .evaluate_script(&script)
                            .map_err(|error| format!("update island control: {error}"))
                    },
                ) {
                    Ok(()) => {}
                    Err(error) => snapshots.report_error(error),
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested | WindowEvent::Destroyed,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Resized(_),
                ..
            } => {
                // Let the OS apply its suggested DPI size first, then restore
                // the logical island size and top-center position next cycle.
                next_recenter = now;
            }
            Event::MainEventsCleared => {
                if now >= next_poll {
                    if preference::is_disabled_for_snapshot(&preference_snapshot)
                        || snapshots.poll(&webview, web_ready, now)
                    {
                        *control_flow = ControlFlow::Exit;
                    }
                    next_poll = now + POLL_INTERVAL;
                }
                if now >= next_recenter {
                    window::resize_and_center(
                        &window,
                        if expanded {
                            IslandSize::Expanded
                        } else {
                            IslandSize::Collapsed
                        },
                    );
                    if let Err(error) = sync_webview_bounds(&webview, &window) {
                        eprintln!("a3s-webview: agent island: {error}");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    next_recenter = now + RECENTER_INTERVAL;
                }
            }
            _ => {}
        }
    });
}

#[cfg(target_os = "macos")]
fn build_webview<'a>(
    builder: WebViewBuilder<'a>,
    window: &'a tao::window::Window,
) -> Result<wry::WebView, String> {
    builder
        // Wry's top-level macOS build path activates NSApplication. A child
        // WKWebView preserves Tao's non-activating window contract instead.
        .build_as_child(window)
        .map_err(|error| format!("create agent island webview: {error}"))
}

#[cfg(target_os = "windows")]
fn build_webview<'a>(
    builder: WebViewBuilder<'a>,
    window: &'a tao::window::Window,
) -> Result<wry::WebView, String> {
    builder
        .build(window)
        .map_err(|error| format!("create agent island webview: {error}"))
}

#[cfg(target_os = "linux")]
fn build_webview<'a>(
    builder: WebViewBuilder<'a>,
    window: &'a tao::window::Window,
) -> Result<wry::WebView, String> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;

    let container = window
        .default_vbox()
        .ok_or_else(|| "agent island window has no GTK container".to_string())?;
    builder
        .build_gtk(container)
        .map_err(|error| format!("create agent island GTK webview: {error}"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn build_webview<'a>(
    builder: WebViewBuilder<'a>,
    window: &'a tao::window::Window,
) -> Result<wry::WebView, String> {
    builder
        .build(window)
        .map_err(|error| format!("create agent island webview: {error}"))
}

fn sync_webview_bounds(webview: &wry::WebView, window: &tao::window::Window) -> Result<(), String> {
    let size = window.inner_size();
    webview
        .set_bounds(wry::Rect {
            position: tao::dpi::PhysicalPosition::new(0, 0).into(),
            size: size.into(),
        })
        .map_err(|error| format!("resize agent island webview: {error}"))
}

struct SnapshotRuntime {
    path: PathBuf,
    control_queue: control::ControlQueue,
    started: Instant,
    watchdog: ShutdownWatchdog,
    current_payload: Option<String>,
    payload_deadline: Option<Instant>,
    payload_evidence_at_ms: Option<u64>,
    current_useful: bool,
    current_snapshot: Option<Snapshot>,
    submitted_control_tokens: HashSet<String>,
    sent_payload: Option<String>,
    last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SnapshotObservation {
    Active,
    Inactive,
    Unavailable,
}

impl SnapshotRuntime {
    fn new(path: PathBuf, started: Instant) -> Result<Self, String> {
        let control_queue = control::ControlQueue::for_snapshot(&path)?;
        Ok(Self {
            path,
            control_queue,
            started,
            watchdog: ShutdownWatchdog::new(0),
            current_payload: None,
            payload_deadline: None,
            payload_evidence_at_ms: None,
            current_useful: false,
            current_snapshot: None,
            submitted_control_tokens: HashSet::new(),
            sent_payload: None,
            last_error: None,
        })
    }

    /// Returns true when stale/empty evidence has exceeded its grace period.
    fn poll(&mut self, webview: &wry::WebView, web_ready: bool, now: Instant) -> bool {
        let monotonic_ms = duration_ms(now.saturating_duration_since(self.started));
        let epoch_ms = epoch_ms();
        let mut fresh_inactive = false;
        match read_snapshot(&self.path, epoch_ms) {
            Ok(snapshot) => {
                self.last_error = None;
                match self.display_snapshot(snapshot, epoch_ms, now) {
                    Ok(SnapshotObservation::Active) => {
                        self.watchdog.observe(monotonic_ms, true);
                    }
                    Ok(SnapshotObservation::Inactive) => {
                        self.watchdog.observe(monotonic_ms, false);
                        fresh_inactive = true;
                    }
                    Ok(SnapshotObservation::Unavailable) => {
                        self.watchdog.observe(monotonic_ms, false);
                    }
                    Err(error) => {
                        self.watchdog.observe(monotonic_ms, false);
                        self.report_error(error);
                    }
                }
            }
            Err(error) => {
                self.watchdog.observe(monotonic_ms, false);
                if let Err(expiry_error) = self.clear_expired_payload(epoch_ms, now) {
                    self.report_error(expiry_error);
                }
                self.report_error(error);
            }
        }

        if web_ready {
            let unsent = self
                .current_payload
                .as_ref()
                .filter(|payload| self.sent_payload.as_ref() != Some(*payload))
                .cloned();
            if let Some(payload) = unsent {
                match snapshot_update_script(&payload).and_then(|script| {
                    webview
                        .evaluate_script(&script)
                        .map_err(|error| format!("update island webview: {error}"))
                }) {
                    Ok(()) => self.sent_payload = Some(payload),
                    Err(error) => self.report_error(error),
                }
            }
        }
        fresh_inactive || self.watchdog.should_shutdown(monotonic_ms)
    }

    fn display_snapshot(
        &mut self,
        snapshot: Snapshot,
        epoch_now_ms: u64,
        monotonic_now: Instant,
    ) -> Result<SnapshotObservation, String> {
        if !snapshot.is_fresh(epoch_now_ms) {
            self.clear_payload(epoch_now_ms)?;
            return Ok(SnapshotObservation::Unavailable);
        }

        let evidence_at_ms = snapshot.updated_at_ms;
        match self.payload_evidence_at_ms {
            Some(previous) if evidence_at_ms < previous => {
                self.clear_expired_payload(epoch_now_ms, monotonic_now)?;
                return Ok(if self.payload_deadline.is_some() && self.current_useful {
                    SnapshotObservation::Active
                } else {
                    SnapshotObservation::Unavailable
                });
            }
            Some(previous) if evidence_at_ms == previous => {}
            _ => {
                let remaining_ms = evidence_at_ms
                    .saturating_add(SNAPSHOT_FRESH_MS)
                    .saturating_sub(epoch_now_ms)
                    .min(SNAPSHOT_FRESH_MS);
                self.payload_deadline = monotonic_now
                    .checked_add(Duration::from_millis(remaining_ms))
                    .or(Some(monotonic_now));
                self.payload_evidence_at_ms = Some(evidence_at_ms);
            }
        }

        if self
            .payload_deadline
            .is_none_or(|deadline| monotonic_now > deadline)
        {
            self.clear_payload(epoch_now_ms)?;
            return Ok(SnapshotObservation::Unavailable);
        }

        self.current_useful = snapshot.has_visible_activity();
        let current_tokens = snapshot
            .activities
            .iter()
            .flat_map(|activity| activity.actions.iter())
            .map(|action| action.token.as_str())
            .collect::<HashSet<_>>();
        self.submitted_control_tokens
            .retain(|token| current_tokens.contains(token.as_str()));
        let mut snapshot = snapshot;
        for activity in &mut snapshot.activities {
            activity
                .actions
                .retain(|action| !self.submitted_control_tokens.contains(&action.token));
        }
        self.current_payload = Some(snapshot.render_json()?);
        self.current_snapshot = Some(snapshot);
        Ok(if self.current_useful {
            SnapshotObservation::Active
        } else {
            SnapshotObservation::Inactive
        })
    }

    fn clear_expired_payload(
        &mut self,
        epoch_now_ms: u64,
        monotonic_now: Instant,
    ) -> Result<(), String> {
        if self
            .payload_deadline
            .is_some_and(|deadline| monotonic_now > deadline)
        {
            self.clear_payload(epoch_now_ms)?;
        }
        Ok(())
    }

    fn clear_payload(&mut self, now_ms: u64) -> Result<(), String> {
        if self.payload_deadline.take().is_some()
            || self.current_payload.is_none()
            || self.current_useful
        {
            self.current_useful = false;
            self.current_snapshot = None;
            self.submitted_control_tokens.clear();
            self.current_payload = Some(Snapshot::render_empty_json(now_ms, true)?);
        } else {
            self.current_useful = false;
            self.current_snapshot = None;
            self.submitted_control_tokens.clear();
        }
        Ok(())
    }

    fn submit_control(
        &mut self,
        submission: control::ControlSubmission,
        now_ms: u64,
    ) -> Result<(), String> {
        if self.submitted_control_tokens.contains(&submission.token) {
            return Err("control decision was already submitted".to_string());
        }
        let control = self
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.authorize_control(&submission, now_ms))
            .ok_or_else(|| "control decision is stale or unavailable".to_string())?;
        self.control_queue.submit(&control, now_ms)?;
        self.submitted_control_tokens.insert(control.token);
        Ok(())
    }

    fn report_error(&mut self, error: String) {
        if self.last_error.as_ref() != Some(&error) {
            eprintln!("a3s-webview: agent island: {error}");
            self.last_error = Some(error);
        }
    }
}

fn snapshot_update_script(payload: &str) -> Result<String, String> {
    let encoded = serde_json::to_string(payload)
        .map_err(|error| format!("encode island update script: {error}"))?;
    Ok(format!(
        "window.a3sIsland && window.a3sIsland.update(JSON.parse({encoded}));"
    ))
}

fn control_result_script(
    activity_id: &str,
    action: control::AgentControlActionKind,
    accepted: bool,
    message: &str,
) -> Result<String, String> {
    let payload = serde_json::json!({
        "activity_id": activity_id,
        "action": action,
        "accepted": accepted,
        "message": message,
    });
    let payload = serde_json::to_string(&payload)
        .map_err(|error| format!("serialize island control result: {error}"))?;
    let encoded = serde_json::to_string(&payload)
        .map_err(|error| format!("encode island control result: {error}"))?;
    Ok(format!(
        "window.a3sIsland && window.a3sIsland.controlResult(JSON.parse({encoded}));"
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShutdownWatchdog {
    last_useful_ms: u64,
}

impl ShutdownWatchdog {
    const fn new(now_ms: u64) -> Self {
        Self {
            last_useful_ms: now_ms,
        }
    }

    fn observe(&mut self, now_ms: u64, fresh_and_nonempty: bool) {
        if fresh_and_nonempty {
            self.last_useful_ms = now_ms;
        }
    }

    fn should_shutdown(self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_useful_ms) >= SHUTDOWN_GRACE_MS
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_private_sibling_paths() {
        let root = std::env::temp_dir().join("a3s-agent-island-args");
        let parsed = parse_args([
            "--snapshot".to_string(),
            root.join("system-snapshot.json")
                .to_string_lossy()
                .into_owned(),
            "--lock-file".to_string(),
            root.join("island.lock").to_string_lossy().into_owned(),
        ])
        .unwrap();
        assert_eq!(parsed.snapshot, root.join("system-snapshot.json"));
        assert_eq!(parsed.lock_file, root.join("island.lock"));
    }

    #[test]
    fn rejects_relative_duplicate_and_cross_directory_paths() {
        assert!(parse_args(
            ["--snapshot", "snapshot.json", "--lock-file", "island.lock"]
                .into_iter()
                .map(str::to_string)
        )
        .is_err());
        assert!(parse_args(
            ["--snapshot", "/tmp/a.json", "--lock-file", "/tmp/a.json"]
                .into_iter()
                .map(str::to_string)
        )
        .is_err());
        assert!(parse_args(
            [
                "--snapshot",
                "/tmp/a.json",
                "--lock-file",
                "/var/tmp/a.lock"
            ]
            .into_iter()
            .map(str::to_string)
        )
        .is_err());
    }

    #[test]
    fn persisted_opt_out_skips_window_and_singleton_initialization() {
        let root = std::env::temp_dir().join(format!(
            "a3s-agent-island-disabled-run-{}-{}",
            std::process::id(),
            epoch_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let snapshot = root.join("system-snapshot.json");
        let lock = root.join("island.lock");
        preference::disable_for_snapshot(&snapshot).unwrap();

        assert_eq!(
            run([
                "--snapshot".to_string(),
                snapshot.to_string_lossy().into_owned(),
                "--lock-file".to_string(),
                lock.to_string_lossy().into_owned(),
            ]),
            Ok(())
        );
        assert!(!lock.exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ipc_parser_accepts_handshakes_and_bounded_controls_only() {
        assert_eq!(parse_ipc("ready"), Some(IslandEvent::Ready));
        assert_eq!(parse_ipc("expand"), Some(IslandEvent::Expand));
        assert_eq!(
            parse_ipc("collapse-complete"),
            Some(IslandEvent::CollapseComplete)
        );
        assert_eq!(parse_ipc("disable"), Some(IslandEvent::Disable));
        assert!(matches!(
            parse_ipc(
                r#"{"type":"control","activity_id":"parent","action":"stop","token":"0123456789abcdef0123456789abcdef","target_instance_id":"parent"}"#
            ),
            Some(IslandEvent::Control(control::ControlSubmission {
                action: control::AgentControlActionKind::Stop,
                ..
            }))
        ));
        assert_eq!(parse_ipc("expand\n"), None);
        assert_eq!(parse_ipc("close"), None);
    }

    #[test]
    fn stale_or_empty_watchdog_uses_monotonic_grace() {
        let mut watchdog = ShutdownWatchdog::new(0);
        watchdog.observe(9_000, true);
        assert!(!watchdog.should_shutdown(28_999));
        assert!(watchdog.should_shutdown(29_000));
    }

    #[test]
    fn fresh_activity_extends_watchdog_but_empty_does_not() {
        let mut watchdog = ShutdownWatchdog::new(0);
        watchdog.observe(15_000, true);
        watchdog.observe(20_000, false);
        assert!(!watchdog.should_shutdown(34_999));
        assert!(watchdog.should_shutdown(35_000));
    }

    #[test]
    fn update_script_treats_snapshot_as_json_data() {
        let payload = r#"{"headline":"</script><img src=x>","activities":[]}"#;
        let script = snapshot_update_script(payload).unwrap();
        assert!(script.starts_with("window.a3sIsland &&"));
        assert!(script.contains("JSON.parse("));
        assert!(script.contains("\\\"headline\\\""));
    }

    #[test]
    fn control_result_script_treats_identifiers_as_json_data() {
        let script = control_result_script(
            "row'</script>",
            control::AgentControlActionKind::Deny,
            false,
            "Try again",
        )
        .unwrap();
        assert!(script.contains("controlResult(JSON.parse("));
        assert!(script.contains("\\\"activity_id\\\""));
        assert!(script.contains("\\\"action\\\":\\\"deny\\\""));
    }

    #[test]
    fn stale_snapshot_payload_is_cleared_at_the_ttl() {
        let bytes = format!(
            r#"{{"schema":"{}","updated_at_ms":1000,"activities":[{{"id":"one","agent":"codex","state":"working","confidence":"exact"}}]}}"#,
            status::SNAPSHOT_SCHEMA
        );
        let snapshot = Snapshot::parse(bytes.as_bytes(), 1_000).unwrap();
        let started = Instant::now();
        let mut runtime =
            SnapshotRuntime::new(std::env::temp_dir().join("snapshot.json"), started).unwrap();

        runtime.display_snapshot(snapshot, 1_000, started).unwrap();
        runtime
            .clear_expired_payload(11_000, started + Duration::from_secs(10))
            .unwrap();
        let still_fresh: serde_json::Value =
            serde_json::from_str(runtime.current_payload.as_deref().unwrap()).unwrap();
        assert_eq!(still_fresh["activities"].as_array().unwrap().len(), 1);

        runtime
            .clear_expired_payload(
                11_001,
                started + Duration::from_millis(SNAPSHOT_FRESH_MS + 1),
            )
            .unwrap();
        let expired: serde_json::Value =
            serde_json::from_str(runtime.current_payload.as_deref().unwrap()).unwrap();
        assert!(expired["activities"].as_array().unwrap().is_empty());
        assert_eq!(expired["degraded"], true);
    }

    #[test]
    fn wall_clock_rollback_does_not_refresh_old_snapshot_evidence() {
        let bytes = format!(
            r#"{{"schema":"{}","updated_at_ms":1000,"activities":[{{"id":"one","agent":"codex","state":"working","confidence":"exact"}}]}}"#,
            status::SNAPSHOT_SCHEMA
        );
        let snapshot = Snapshot::parse(bytes.as_bytes(), 1_000).unwrap();
        let started = Instant::now();
        let mut runtime =
            SnapshotRuntime::new(std::env::temp_dir().join("snapshot.json"), started).unwrap();

        assert_eq!(
            runtime
                .display_snapshot(snapshot.clone(), 1_000, started)
                .unwrap(),
            SnapshotObservation::Active
        );
        assert_eq!(
            runtime
                .display_snapshot(
                    snapshot,
                    0,
                    started + Duration::from_millis(SNAPSHOT_FRESH_MS + 1),
                )
                .unwrap(),
            SnapshotObservation::Unavailable
        );
        let payload: serde_json::Value =
            serde_json::from_str(runtime.current_payload.as_deref().unwrap()).unwrap();
        assert!(payload["activities"].as_array().unwrap().is_empty());
    }

    #[test]
    fn fresh_idle_snapshot_stays_active_while_an_external_agent_is_detected() {
        let bytes = format!(
            r#"{{"schema":"{}","updated_at_ms":1000,"activities":[{{"id":"idle","agent":"a3s-code","state":"idle","confidence":"exact"}},{{"id":"process","agent":"codex","state":"unknown","confidence":"process"}}]}}"#,
            status::SNAPSHOT_SCHEMA
        );
        let snapshot = Snapshot::parse(bytes.as_bytes(), 1_000).unwrap();
        let started = Instant::now();
        let mut runtime =
            SnapshotRuntime::new(std::env::temp_dir().join("snapshot.json"), started).unwrap();

        assert_eq!(
            runtime.display_snapshot(snapshot, 1_000, started).unwrap(),
            SnapshotObservation::Active
        );
        assert!(runtime.current_useful);
    }

    #[test]
    fn fresh_idle_snapshot_without_external_activity_requests_clean_shutdown() {
        let bytes = format!(
            r#"{{"schema":"{}","updated_at_ms":1000,"activities":[{{"id":"idle","agent":"a3s-code","state":"idle","confidence":"exact"}}]}}"#,
            status::SNAPSHOT_SCHEMA
        );
        let snapshot = Snapshot::parse(bytes.as_bytes(), 1_000).unwrap();
        let started = Instant::now();
        let mut runtime =
            SnapshotRuntime::new(std::env::temp_dir().join("snapshot.json"), started).unwrap();

        assert_eq!(
            runtime.display_snapshot(snapshot, 1_000, started).unwrap(),
            SnapshotObservation::Inactive
        );
        assert!(!runtime.current_useful);
    }
}
