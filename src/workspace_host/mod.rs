mod protocol;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use protocol::{
    same_shell_document, validate_open, validated_shell_url, BridgeMode, NavigationPolicy,
    ResourceIdentity, ValidatedOpen, ValidationContext, ViewBridgeMessage, WorkspaceBounds,
    WorkspaceCommand, WorkspaceHostEvent, PROTOCOL_VERSION,
};
use serde_json::Value;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use url::Url;
use wry::{BackgroundThrottlingPolicy, NewWindowResponse, PageLoadEvent, Rect, WebViewBuilder};

#[cfg(target_os = "macos")]
use objc2_app_kit::{NSAnimatablePropertyContainer, NSAnimationContext, NSView, NSWorkspace};
#[cfg(target_os = "macos")]
use wry::WebViewExtMacOS;

pub(crate) const USAGE: &str = "usage: a3s-webview --workspace-host --url <http(s)://…|file://…> \
[--width N] [--height N] [--title T] [--allow-file-root <absolute-path>]…";

const DEFAULT_WIDTH: f64 = 1440.0;
const DEFAULT_HEIGHT: f64 = 900.0;
const LOAD_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_OPEN_VIEWS: usize = 8;
const MAX_PENDING_MESSAGES: usize = 64;
const MAX_PENDING_MESSAGE_BYTES: usize = 512 * 1024;

#[derive(Debug)]
struct HostArgs {
    shell_url: Url,
    width: f64,
    height: f64,
    title: String,
    allowed_file_roots: Vec<PathBuf>,
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<HostArgs, String> {
    let mut shell_url = None;
    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut title = "A3S Workspace".to_string();
    let mut allowed_file_roots = Vec::new();
    let mut arguments = args.into_iter();
    while let Some(argument) = arguments.next() {
        let mut next = || {
            arguments
                .next()
                .ok_or_else(|| format!("{argument} needs a value"))
        };
        match argument.as_str() {
            "--url" => shell_url = Some(validated_shell_url(&next()?)?),
            "--width" => {
                width = next()?
                    .parse::<f64>()
                    .map_err(|_| "--width must be a number".to_string())?;
            }
            "--height" => {
                height = next()?
                    .parse::<f64>()
                    .map_err(|_| "--height must be a number".to_string())?;
            }
            "--title" => title = validated_title(&next()?)?,
            "--allow-file-root" => {
                let raw = PathBuf::from(next()?);
                if !raw.is_absolute() {
                    return Err("--allow-file-root must be absolute".to_string());
                }
                let root = raw.canonicalize().map_err(|error| {
                    format!(
                        "could not resolve allowed file root {}: {error}",
                        raw.display()
                    )
                })?;
                if !root.is_dir() {
                    return Err(format!(
                        "allowed file root must be a directory: {}",
                        root.display()
                    ));
                }
                if !allowed_file_roots.contains(&root) {
                    allowed_file_roots.push(root);
                }
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown workspace host argument: {other}")),
        }
    }
    let shell_url = shell_url.ok_or_else(|| "--url is required".to_string())?;
    if !width.is_finite() || !height.is_finite() {
        return Err("workspace host size must be finite".to_string());
    }
    Ok(HostArgs {
        shell_url,
        width: width.clamp(640.0, 4_000.0),
        height: height.clamp(480.0, 3_000.0),
        title,
        allowed_file_roots,
    })
}

fn validated_title(value: &str) -> Result<String, String> {
    let title = value.trim();
    if title.is_empty() || title.chars().count() > 160 || title.chars().any(char::is_control) {
        return Err("workspace host title must contain 1–160 printable characters".to_string());
    }
    Ok(title.to_string())
}

#[derive(Debug)]
enum HostEvent {
    ShellIpc {
        source: String,
        body: String,
    },
    ViewIpc {
        identity: ResourceIdentity,
        source: String,
        body: String,
    },
    ViewLoad {
        identity: ResourceIdentity,
        started: bool,
        url: String,
    },
    ShellLoad {
        finished: bool,
    },
}

struct HostedView {
    identity: ResourceIdentity,
    url: String,
    bounds: WorkspaceBounds,
    navigation: NavigationPolicy,
    bridge: BridgeMode,
    webview: wry::WebView,
    load_deadline: Option<Instant>,
    ready: bool,
    occluded: bool,
    pending_messages: PendingMessages,
}

#[derive(Default)]
struct PendingMessages {
    items: VecDeque<(Value, usize)>,
    bytes: usize,
}

impl PendingMessages {
    fn push(&mut self, payload: Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(&payload)
            .map_err(|error| format!("serialize pending workspace message: {error}"))?
            .len();
        if self.items.len() >= MAX_PENDING_MESSAGES
            || self.bytes.saturating_add(bytes) > MAX_PENDING_MESSAGE_BYTES
        {
            return Err(format!(
                "workspace pending message queue exceeds {MAX_PENDING_MESSAGES} messages or {MAX_PENDING_MESSAGE_BYTES} bytes"
            ));
        }
        self.items.push_back((payload, bytes));
        self.bytes += bytes;
        Ok(())
    }

    fn front(&self) -> Option<&Value> {
        self.items.front().map(|(payload, _)| payload)
    }

    fn pop_front(&mut self) -> Result<(), String> {
        let Some((_, bytes)) = self.items.pop_front() else {
            return Err("workspace pending message queue changed unexpectedly".to_string());
        };
        self.bytes = self.bytes.saturating_sub(bytes);
        Ok(())
    }
}

struct HostRuntime {
    shell_url: Url,
    allowed_file_roots: Vec<PathBuf>,
    active_resource_id: Option<String>,
    views: HashMap<String, HostedView>,
    latest_generation: u64,
}

impl HostRuntime {
    fn new(shell_url: Url, allowed_file_roots: Vec<PathBuf>) -> Self {
        Self {
            shell_url,
            allowed_file_roots,
            active_resource_id: None,
            views: HashMap::new(),
            latest_generation: 0,
        }
    }

    fn is_current(&self, identity: &ResourceIdentity) -> bool {
        self.views
            .get(&identity.resource_id)
            .is_some_and(|view| view.identity.generation == identity.generation)
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.views
            .values()
            .filter_map(|view| view.load_deadline)
            .min()
    }
}

pub(crate) fn run<I: IntoIterator<Item = String>>(args: I) -> Result<(), String> {
    let args = parse_args(args)?;
    let event_loop = EventLoopBuilder::<HostEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(&args.title)
        .with_inner_size(LogicalSize::new(args.width, args.height))
        .with_focused(true)
        .build(&event_loop)
        .map_err(|error| format!("create workspace host window: {error}"))?;
    let shell_bounds = full_window_bounds(&window);
    let proxy = event_loop.create_proxy();
    let ipc_proxy = proxy.clone();
    let shell_load_proxy = proxy.clone();
    let expected_shell_url = args.shell_url.clone();
    let navigation_shell_url = args.shell_url.clone();
    let shell_webview = WebViewBuilder::new()
        .with_url(args.shell_url.as_str())
        .with_bounds(shell_bounds)
        .with_initialization_script(SHELL_BRIDGE_SCRIPT)
        .with_navigation_handler(move |url| {
            same_shell_document(&navigation_shell_url, &url) || url == "about:blank"
        })
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_on_page_load_handler(move |event, _| {
            let _ = shell_load_proxy.send_event(HostEvent::ShellLoad {
                finished: matches!(event, PageLoadEvent::Finished),
            });
        })
        .with_ipc_handler(move |request| {
            let _ = ipc_proxy.send_event(HostEvent::ShellIpc {
                source: request.uri().to_string(),
                body: request.body().to_string(),
            });
        })
        .with_background_throttling(BackgroundThrottlingPolicy::Disabled)
        .build_as_child(&window)
        .map_err(|error| format!("create workspace shell webview: {error}"))?;

    let mut runtime = HostRuntime::new(args.shell_url, args.allowed_file_roots);
    event_loop.run(move |event, _, control_flow| {
        *control_flow = runtime
            .next_deadline()
            .map_or(ControlFlow::Wait, ControlFlow::WaitUntil);
        match event {
            Event::UserEvent(HostEvent::ShellIpc { source, body }) => {
                if !same_shell_document(&expected_shell_url, &source) {
                    emit_host_error(
                        &shell_webview,
                        "rejected workspace command from an untrusted shell origin",
                    );
                    return;
                }
                let command = match WorkspaceCommand::parse(&body) {
                    Ok(command) => command,
                    Err(error) => {
                        emit_host_error(&shell_webview, &error);
                        return;
                    }
                };
                if let Err(error) =
                    handle_command(command, &window, &proxy, &shell_webview, &mut runtime)
                {
                    emit_host_error(&shell_webview, &error);
                }
            }
            Event::UserEvent(HostEvent::ViewIpc {
                identity,
                source,
                body,
            }) => {
                if identity.generation < runtime.latest_generation {
                    return;
                }
                let Some(view) = runtime.views.get(&identity.resource_id) else {
                    return;
                };
                if view.identity.generation != identity.generation
                    || view.bridge != BridgeMode::Typed
                    || !view.navigation.allows(&source)
                {
                    return;
                }
                let Ok(message) = ViewBridgeMessage::parse(&body) else {
                    return;
                };
                let (_, resource_id, generation) = message.identity();
                if resource_id != identity.resource_id || generation != identity.generation {
                    return;
                }
                match message {
                    ViewBridgeMessage::Ready { .. } => {
                        let Some(view) = runtime.views.get_mut(&identity.resource_id) else {
                            return;
                        };
                        view.load_deadline = None;
                        if let Err(error) = flush_pending_messages(view) {
                            view.ready = false;
                            let _ = view.webview.set_visible(false);
                            emit_lifecycle(
                                &shell_webview,
                                &identity,
                                "error",
                                Some(&view.url),
                                Some(&error),
                            );
                            return;
                        }
                        view.ready = true;
                        if runtime.active_resource_id.as_deref() == Some(&identity.resource_id)
                            && !view.occluded
                        {
                            if let Err(error) =
                                reveal_webview(&view.webview, clamped_rect(view.bounds, &window))
                            {
                                view.ready = false;
                                let _ = view.webview.set_visible(false);
                                emit_lifecycle(
                                    &shell_webview,
                                    &identity,
                                    "error",
                                    Some(&view.url),
                                    Some(&error),
                                );
                                return;
                            }
                        }
                        emit_lifecycle(&shell_webview, &identity, "ready", Some(&view.url), None);
                    }
                    ViewBridgeMessage::Error { message, .. } => {
                        let Some(view) = runtime.views.get_mut(&identity.resource_id) else {
                            return;
                        };
                        view.ready = false;
                        view.load_deadline = None;
                        let _ = view.webview.set_visible(false);
                        emit_lifecycle(
                            &shell_webview,
                            &identity,
                            "error",
                            Some(&view.url),
                            Some(&message),
                        );
                    }
                    ViewBridgeMessage::Message { payload, .. } => {
                        let event = WorkspaceHostEvent::ViewMessage {
                            version: PROTOCOL_VERSION,
                            resource_id: &identity.resource_id,
                            generation: identity.generation,
                            payload: &payload,
                        };
                        if let Err(error) = dispatch_shell_event(&shell_webview, &event) {
                            emit_host_error(&shell_webview, &error);
                        }
                    }
                }
            }
            Event::UserEvent(HostEvent::ViewLoad {
                identity,
                started,
                url,
            }) => {
                if identity.generation < runtime.latest_generation {
                    return;
                }
                let Some(view) = runtime.views.get_mut(&identity.resource_id) else {
                    return;
                };
                if view.identity.generation != identity.generation {
                    return;
                }
                if started {
                    view.ready = false;
                    view.load_deadline = Some(Instant::now() + LOAD_TIMEOUT);
                    let staged = if view.occluded {
                        conceal_webview(&view.webview, &window)
                    } else {
                        stage_webview(&view.webview, &window)
                    };
                    if let Err(error) = staged {
                        emit_host_error(
                            &shell_webview,
                            &format!("stage workspace content: {error}"),
                        );
                    }
                    emit_lifecycle(&shell_webview, &identity, "loading", Some(&url), None);
                } else if view.bridge == BridgeMode::None {
                    view.load_deadline = None;
                    if runtime.active_resource_id.as_deref() == Some(&identity.resource_id)
                        && !view.occluded
                    {
                        if let Err(error) =
                            reveal_webview(&view.webview, clamped_rect(view.bounds, &window))
                        {
                            view.ready = false;
                            emit_lifecycle(
                                &shell_webview,
                                &identity,
                                "error",
                                Some(&url),
                                Some(&error),
                            );
                            return;
                        }
                    }
                    view.ready = true;
                    emit_lifecycle(&shell_webview, &identity, "ready", Some(&url), None);
                }
            }
            Event::UserEvent(HostEvent::ShellLoad { finished: true }) => {
                if let Err(error) = dispatch_shell_event(
                    &shell_webview,
                    &WorkspaceHostEvent::HostReady {
                        version: PROTOCOL_VERSION,
                        native: true,
                    },
                ) {
                    eprintln!("a3s-webview: workspace host: {error}");
                }
            }
            Event::UserEvent(HostEvent::ShellLoad { finished: false }) => {}
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            }
            | Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { .. },
                ..
            } => {
                if let Err(error) = shell_webview.set_bounds(full_window_bounds(&window)) {
                    emit_host_error(
                        &shell_webview,
                        &format!("resize workspace shell webview: {error}"),
                    );
                }
                for view in runtime.views.values() {
                    let bounds = if view.ready {
                        clamped_rect(view.bounds, &window)
                    } else {
                        staging_rect(&window)
                    };
                    if let Err(error) = view.webview.set_bounds(bounds) {
                        emit_host_error(
                            &shell_webview,
                            &format!("resize workspace content webview: {error}"),
                        );
                    }
                }
            }
            Event::MainEventsCleared => {
                let now = Instant::now();
                let timed_out = runtime
                    .views
                    .values_mut()
                    .filter_map(|view| {
                        let expired = view.load_deadline.is_some_and(|deadline| deadline <= now);
                        if !expired {
                            return None;
                        }
                        view.load_deadline = None;
                        view.ready = false;
                        let _ = view.webview.set_visible(false);
                        Some((view.identity.clone(), view.url.clone()))
                    })
                    .collect::<Vec<_>>();
                for (identity, url) in timed_out {
                    emit_lifecycle(
                        &shell_webview,
                        &identity,
                        "error",
                        Some(&url),
                        Some("workspace content did not become ready within 30 seconds"),
                    );
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                runtime.views.clear();
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn handle_command(
    command: WorkspaceCommand,
    window: &Window,
    proxy: &EventLoopProxy<HostEvent>,
    shell_webview: &wry::WebView,
    runtime: &mut HostRuntime,
) -> Result<(), String> {
    match command {
        WorkspaceCommand::Open {
            resource_id,
            generation,
            title,
            target,
            bounds,
            policy,
            ..
        } => {
            if generation < runtime.latest_generation {
                return Err("stale workspace open was rejected".to_string());
            }
            if generation > runtime.latest_generation {
                retire_stale_views(generation, shell_webview, runtime);
            }
            runtime.latest_generation = generation;
            let context = ValidationContext {
                shell_url: &runtime.shell_url,
                allowed_file_roots: &runtime.allowed_file_roots,
            };
            let open = validate_open(
                resource_id,
                generation,
                title,
                target,
                bounds,
                policy,
                &context,
            )?;
            open_view(open, window, proxy, shell_webview, runtime)
        }
        WorkspaceCommand::Bounds {
            resource_id,
            generation,
            bounds,
            ..
        } => {
            if generation < runtime.latest_generation {
                return Err("stale workspace bounds were rejected".to_string());
            }
            let identity = ResourceIdentity {
                resource_id,
                generation,
            };
            let Some(view) = runtime.views.get_mut(&identity.resource_id) else {
                return Err("workspace bounds target is not open".to_string());
            };
            if view.identity.generation != identity.generation {
                return Err("stale workspace bounds were rejected".to_string());
            }
            view.bounds = bounds;
            if view.ready && !view.occluded {
                view.webview
                    .set_bounds(clamped_rect(bounds, window))
                    .map_err(|error| format!("apply workspace bounds: {error}"))
            } else {
                Ok(())
            }
        }
        WorkspaceCommand::Occlusion {
            resource_id,
            generation,
            occluded,
            ..
        } => {
            if generation < runtime.latest_generation {
                return Err("stale workspace occlusion was rejected".to_string());
            }
            let identity = ResourceIdentity {
                resource_id,
                generation,
            };
            let Some(view) = runtime.views.get_mut(&identity.resource_id) else {
                return Err("workspace occlusion target is not open".to_string());
            };
            if view.identity.generation != identity.generation {
                return Err("stale workspace occlusion was rejected".to_string());
            }
            view.occluded = occluded;
            if runtime.active_resource_id.as_deref() != Some(&identity.resource_id) || !view.ready {
                return Ok(());
            }
            if occluded {
                conceal_webview(&view.webview, window)
            } else {
                restore_webview(&view.webview, clamped_rect(view.bounds, window))
            }
        }
        WorkspaceCommand::Close {
            resource_id,
            generation,
            ..
        } => {
            if generation < runtime.latest_generation {
                return Err("stale workspace close was rejected".to_string());
            }
            let identity = ResourceIdentity {
                resource_id,
                generation,
            };
            if !runtime.is_current(&identity) {
                return Err("stale workspace close was rejected".to_string());
            }
            runtime.views.remove(&identity.resource_id);
            if runtime.active_resource_id.as_deref() == Some(&identity.resource_id) {
                runtime.active_resource_id = None;
            }
            emit_lifecycle(shell_webview, &identity, "closed", None, None);
            Ok(())
        }
        WorkspaceCommand::PostMessage {
            resource_id,
            generation,
            payload,
            ..
        } => {
            if generation < runtime.latest_generation {
                return Err("stale workspace message was rejected".to_string());
            }
            let identity = ResourceIdentity {
                resource_id,
                generation,
            };
            let Some(view) = runtime.views.get_mut(&identity.resource_id) else {
                return Err("workspace message target is not open".to_string());
            };
            if view.identity.generation != identity.generation {
                return Err("stale workspace message was rejected".to_string());
            }
            if view.bridge != BridgeMode::Typed {
                return Err("workspace target did not grant a typed message bridge".to_string());
            }
            if view.ready {
                dispatch_view_message(&view.webview, &identity, &payload)
            } else {
                queue_pending_message(view, payload)
            }
        }
    }
}

fn open_view(
    open: ValidatedOpen,
    window: &Window,
    proxy: &EventLoopProxy<HostEvent>,
    shell_webview: &wry::WebView,
    runtime: &mut HostRuntime,
) -> Result<(), String> {
    if let Some(existing) = runtime.views.get(&open.identity.resource_id) {
        if existing.identity.generation > open.identity.generation {
            return Err("stale workspace open was rejected".to_string());
        }
        if existing.identity.generation == open.identity.generation {
            if existing.url != open.url || existing.bridge != open.bridge {
                return Err(
                    "workspace resource configuration changed without a new generation".to_string(),
                );
            }
            for view in runtime.views.values() {
                let _ = view.webview.set_visible(false);
            }
            let Some(existing) = runtime.views.get_mut(&open.identity.resource_id) else {
                return Err("workspace view disappeared while it was being activated".to_string());
            };
            existing.bounds = open.bounds;
            existing
                .webview
                .set_bounds(clamped_rect(open.bounds, window))
                .map_err(|error| format!("activate workspace content: {error}"))?;
            if existing.ready && !existing.occluded {
                reveal_webview(&existing.webview, clamped_rect(open.bounds, window))?;
            }
            runtime.active_resource_id = Some(open.identity.resource_id.clone());
            emit_lifecycle(
                shell_webview,
                &open.identity,
                if existing.ready { "ready" } else { "loading" },
                Some(&open.url),
                None,
            );
            return Ok(());
        }
    } else if runtime.views.len() >= MAX_OPEN_VIEWS {
        return Err(format!(
            "workspace host already contains the maximum of {MAX_OPEN_VIEWS} open views"
        ));
    }

    if let Some(replaced) = runtime.views.remove(&open.identity.resource_id) {
        emit_lifecycle(
            shell_webview,
            &replaced.identity,
            "closed",
            Some(&replaced.url),
            Some("replaced by a newer workspace generation"),
        );
    }
    for view in runtime.views.values() {
        let _ = view.webview.set_visible(false);
    }

    let identity = open.identity.clone();
    let load_identity = identity.clone();
    let load_proxy = proxy.clone();
    let ipc_identity = identity.clone();
    let ipc_proxy = proxy.clone();
    let navigation = open.navigation.clone();
    let bounds = staging_rect(window);
    let mut builder = WebViewBuilder::new()
        .with_url(&open.url)
        .with_bounds(bounds)
        .with_visible(true)
        .with_background_throttling(BackgroundThrottlingPolicy::Disabled)
        .with_navigation_handler(move |url| navigation.allows(&url))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_on_page_load_handler(move |event, url| {
            let _ = load_proxy.send_event(HostEvent::ViewLoad {
                identity: load_identity.clone(),
                started: matches!(event, PageLoadEvent::Started),
                url,
            });
        });
    if open.bridge == BridgeMode::Typed {
        builder = builder
            .with_initialization_script(&view_bridge_script(&identity)?)
            .with_ipc_handler(move |request| {
                let _ = ipc_proxy.send_event(HostEvent::ViewIpc {
                    identity: ipc_identity.clone(),
                    source: request.uri().to_string(),
                    body: request.body().to_string(),
                });
            });
    }
    let webview = builder
        .build_as_child(window)
        .map_err(|error| format!("create workspace content webview: {error}"))?;
    stage_webview(&webview, window)?;
    runtime.active_resource_id = Some(identity.resource_id.clone());
    runtime.views.insert(
        identity.resource_id.clone(),
        HostedView {
            identity: identity.clone(),
            url: open.url.clone(),
            bounds: open.bounds,
            navigation: open.navigation,
            bridge: open.bridge,
            webview,
            load_deadline: Some(Instant::now() + LOAD_TIMEOUT),
            ready: false,
            occluded: false,
            pending_messages: PendingMessages::default(),
        },
    );
    emit_lifecycle(shell_webview, &identity, "loading", Some(&open.url), None);
    Ok(())
}

fn retire_stale_views(generation: u64, shell_webview: &wry::WebView, runtime: &mut HostRuntime) {
    let stale = runtime
        .views
        .iter()
        .filter(|(_, view)| view.identity.generation < generation)
        .map(|(resource_id, view)| (resource_id.clone(), view.identity.clone(), view.url.clone()))
        .collect::<Vec<_>>();
    for (resource_id, identity, url) in stale {
        runtime.views.remove(&resource_id);
        if runtime.active_resource_id.as_deref() == Some(&resource_id) {
            runtime.active_resource_id = None;
        }
        emit_lifecycle(
            shell_webview,
            &identity,
            "closed",
            Some(&url),
            Some("retired by a newer workspace generation"),
        );
    }
}

fn full_window_bounds(window: &Window) -> Rect {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    Rect {
        position: LogicalPosition::new(0.0, 0.0).into(),
        size: LogicalSize::new(size.width.max(1.0), size.height.max(1.0)).into(),
    }
}

fn clamped_rect(bounds: WorkspaceBounds, window: &Window) -> Rect {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let x = bounds.x.clamp(0.0, (size.width - 1.0).max(0.0));
    let y = bounds.y.clamp(0.0, (size.height - 1.0).max(0.0));
    let width = bounds.width.min((size.width - x).max(1.0)).max(1.0);
    let height = bounds.height.min((size.height - y).max(1.0)).max(1.0);
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(width, height).into(),
    }
}

fn staging_rect(window: &Window) -> Rect {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    Rect {
        position: LogicalPosition::new((size.width - 1.0).max(0.0), (size.height - 1.0).max(0.0))
            .into(),
        size: LogicalSize::new(1.0, 1.0).into(),
    }
}

fn conceal_webview(webview: &wry::WebView, window: &Window) -> Result<(), String> {
    webview
        .set_bounds(staging_rect(window))
        .map_err(|error| format!("stage occluded workspace content: {error}"))?;
    webview
        .set_visible(false)
        .map_err(|error| format!("occlude workspace content: {error}"))
}

#[cfg(target_os = "macos")]
fn restore_webview(webview: &wry::WebView, bounds: Rect) -> Result<(), String> {
    webview
        .set_bounds(bounds)
        .map_err(|error| format!("restore workspace content bounds: {error}"))?;
    webview
        .set_visible(true)
        .map_err(|error| format!("restore workspace content: {error}"))?;
    let native = webview.webview();
    let view: &NSView = &native;
    view.setAlphaValue(1.0);
    webview
        .focus()
        .map_err(|error| format!("restore workspace content focus: {error}"))
}

#[cfg(not(target_os = "macos"))]
fn restore_webview(webview: &wry::WebView, bounds: Rect) -> Result<(), String> {
    webview
        .set_bounds(bounds)
        .map_err(|error| format!("restore workspace content bounds: {error}"))?;
    webview
        .set_visible(true)
        .map_err(|error| format!("restore workspace content: {error}"))?;
    webview
        .focus()
        .map_err(|error| format!("restore workspace content focus: {error}"))
}

fn emit_lifecycle(
    shell_webview: &wry::WebView,
    identity: &ResourceIdentity,
    phase: &str,
    url: Option<&str>,
    message: Option<&str>,
) {
    let event = WorkspaceHostEvent::Lifecycle {
        version: PROTOCOL_VERSION,
        resource_id: &identity.resource_id,
        generation: identity.generation,
        phase,
        url,
        message,
    };
    if let Err(error) = dispatch_shell_event(shell_webview, &event) {
        eprintln!("a3s-webview: workspace host: {error}");
    }
}

fn emit_host_error(shell_webview: &wry::WebView, message: &str) {
    let event = WorkspaceHostEvent::HostError {
        version: PROTOCOL_VERSION,
        message,
    };
    if let Err(error) = dispatch_shell_event(shell_webview, &event) {
        eprintln!("a3s-webview: workspace host: {error}");
    }
}

fn dispatch_shell_event(
    shell_webview: &wry::WebView,
    event: &WorkspaceHostEvent<'_>,
) -> Result<(), String> {
    let detail = serde_json::to_string(event)
        .map_err(|error| format!("serialize workspace host event: {error}"))?;
    shell_webview
        .evaluate_script(&format!(
            "window.dispatchEvent(new CustomEvent('a3s-workspace-event',{{detail:{detail}}}));"
        ))
        .map_err(|error| format!("dispatch workspace host event: {error}"))
}

fn dispatch_view_message(
    webview: &wry::WebView,
    identity: &ResourceIdentity,
    payload: &Value,
) -> Result<(), String> {
    let detail = serde_json::to_string(&serde_json::json!({
        "version": PROTOCOL_VERSION,
        "resourceId": identity.resource_id,
        "generation": identity.generation,
        "payload": payload,
    }))
    .map_err(|error| format!("serialize host-to-workspace message: {error}"))?;
    webview
        .evaluate_script(&format!(
            "window.__a3sWorkspaceMessages=window.__a3sWorkspaceMessages||[];\
             window.__a3sWorkspaceMessages.push({detail});\
             window.dispatchEvent(new CustomEvent('a3s-workspace-message',{{detail:{detail}}}));"
        ))
        .map_err(|error| format!("dispatch host-to-workspace message: {error}"))
}

fn queue_pending_message(view: &mut HostedView, payload: Value) -> Result<(), String> {
    view.pending_messages.push(payload)
}

fn flush_pending_messages(view: &mut HostedView) -> Result<(), String> {
    while let Some(payload) = view.pending_messages.front() {
        dispatch_view_message(&view.webview, &view.identity, payload)?;
        view.pending_messages.pop_front()?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn stage_webview(webview: &wry::WebView, window: &Window) -> Result<(), String> {
    webview
        .set_bounds(staging_rect(window))
        .map_err(|error| format!("set workspace staging bounds: {error}"))?;
    webview
        .set_visible(true)
        .map_err(|error| format!("keep staged workspace content running: {error}"))?;
    let native = webview.webview();
    let view: &NSView = &native;
    view.setAlphaValue(0.0);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn stage_webview(webview: &wry::WebView, window: &Window) -> Result<(), String> {
    webview
        .set_bounds(staging_rect(window))
        .map_err(|error| format!("set workspace staging bounds: {error}"))?;
    webview
        .set_visible(true)
        .map_err(|error| format!("keep staged workspace content running: {error}"))
}

#[cfg(target_os = "macos")]
fn reveal_webview(webview: &wry::WebView, bounds: Rect) -> Result<(), String> {
    let native = webview.webview();
    let view: &NSView = &native;
    view.setAlphaValue(0.0);
    webview
        .set_bounds(bounds)
        .map_err(|error| format!("position workspace content: {error}"))?;
    webview
        .set_visible(true)
        .map_err(|error| format!("show workspace content: {error}"))?;

    if NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
        view.setAlphaValue(1.0);
        return webview
            .focus()
            .map_err(|error| format!("focus workspace content: {error}"));
    }

    NSAnimationContext::beginGrouping();
    let context = NSAnimationContext::currentContext();
    context.setDuration(0.18);
    view.animator().setAlphaValue(1.0);
    NSAnimationContext::endGrouping();
    webview
        .focus()
        .map_err(|error| format!("focus workspace content: {error}"))
}

#[cfg(not(target_os = "macos"))]
fn reveal_webview(webview: &wry::WebView, bounds: Rect) -> Result<(), String> {
    webview
        .set_bounds(bounds)
        .map_err(|error| format!("position workspace content: {error}"))?;
    webview
        .set_visible(true)
        .map_err(|error| format!("show workspace content: {error}"))?;
    webview
        .focus()
        .map_err(|error| format!("focus workspace content: {error}"))
}

fn view_bridge_script(identity: &ResourceIdentity) -> Result<String, String> {
    let resource_id = serde_json::to_string(&identity.resource_id)
        .map_err(|error| format!("serialize workspace resource id: {error}"))?;
    Ok(format!(
        "Object.defineProperty(window,'a3sWorkspaceView',{{value:Object.freeze({{\
         version:'{PROTOCOL_VERSION}',\
         resourceId:{resource_id},\
         generation:{generation},\
         consumePending:function(){{var q=window.__a3sWorkspaceMessages||[];\
           window.__a3sWorkspaceMessages=[];return q;}},\
         ready:function(){{window.ipc.postMessage(JSON.stringify({{\
           version:'{PROTOCOL_VERSION}',type:'workspace.view_ready',\
           resourceId:{resource_id},generation:{generation}\
         }}));}},\
         fail:function(message){{window.ipc.postMessage(JSON.stringify({{\
           version:'{PROTOCOL_VERSION}',type:'workspace.view_error',\
           resourceId:{resource_id},generation:{generation},message:String(message)\
         }}));}},\
         postMessage:function(payload){{window.ipc.postMessage(JSON.stringify({{\
           version:'{PROTOCOL_VERSION}',type:'workspace.view_message',\
           resourceId:{resource_id},generation:{generation},payload:payload\
         }}));}}\
       }}),configurable:false,writable:false}});",
        generation = identity.generation,
    ))
}

const SHELL_BRIDGE_SCRIPT: &str = r#"
Object.defineProperty(window,'a3sWorkspaceHost',{value:Object.freeze({
  version:'a3s.workspace.v1',
  native:true,
  postMessage:function(command){
    if(!command||typeof command!=='object')throw new TypeError('workspace command must be an object');
    window.ipc.postMessage(JSON.stringify(command));
  }
}),configurable:false,writable:false});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(values: &[&str]) -> Result<HostArgs, String> {
        parse_args(values.iter().map(|value| value.to_string()))
    }

    #[test]
    fn parses_workspace_host_window_options() {
        let args = parse(&[
            "--url",
            "http://127.0.0.1:4180",
            "--width",
            "50000",
            "--height",
            "100",
            "--title",
            "Avatar Workspace",
        ])
        .unwrap();
        assert_eq!(args.shell_url.as_str(), "http://127.0.0.1:4180/");
        assert_eq!(args.width, 4_000.0);
        assert_eq!(args.height, 480.0);
        assert_eq!(args.title, "Avatar Workspace");
    }

    #[test]
    fn rejects_credentialed_or_unsupported_shell_urls() {
        assert!(parse(&["--url", "javascript:alert(1)"]).is_err());
        assert!(parse(&["--url", "https://user@example.com/"]).is_err());
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn bridge_script_carries_resource_generation_and_typed_envelope() {
        let script = view_bridge_script(&ResourceIdentity {
            resource_id: "draft-1".to_string(),
            generation: 7,
        })
        .unwrap();
        assert!(script.contains("a3sWorkspaceView"));
        assert!(script.contains("workspace.view_message"));
        assert!(script.contains("workspace.view_ready"));
        assert!(script.contains("workspace.view_error"));
        assert!(script.contains("draft-1"));
        assert!(script.contains("generation:7"));
    }

    #[test]
    fn pending_queue_is_fifo_and_bounded() {
        let mut queue = PendingMessages::default();
        queue.push(serde_json::json!({"revision": 1})).unwrap();
        queue.push(serde_json::json!({"revision": 2})).unwrap();
        assert_eq!(queue.front().unwrap()["revision"], 1);
        queue.pop_front().unwrap();
        assert_eq!(queue.front().unwrap()["revision"], 2);

        while queue.items.len() < MAX_PENDING_MESSAGES {
            queue.push(Value::Null).unwrap();
        }
        assert!(queue.push(Value::Null).is_err());

        let mut oversized = PendingMessages::default();
        let payload = Value::String("x".repeat(MAX_PENDING_MESSAGE_BYTES));
        assert!(oversized.push(payload).is_err());
    }
}
