use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event_loop::{EventLoop, EventLoopWindowTarget};
use tao::window::{Window, WindowBuilder};

pub(crate) const COLLAPSED_WIDTH: f64 = 392.0;
pub(crate) const COLLAPSED_HEIGHT: f64 = 60.0;
pub(crate) const EXPANDED_WIDTH: f64 = 560.0;
pub(crate) const EXPANDED_HEIGHT: f64 = 360.0;
pub(crate) const HORIZONTAL_GLOW_INSET: f64 = 48.0;
pub(crate) const VERTICAL_GLOW_INSET: f64 = 32.0;
const TOP_MARGIN: f64 = 6.0;
const SCREEN_INSET: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IslandSize {
    Collapsed,
    Expanded,
}

impl IslandSize {
    fn logical_size(self) -> LogicalSize<f64> {
        let surface = match self {
            Self::Collapsed => LogicalSize::new(COLLAPSED_WIDTH, COLLAPSED_HEIGHT),
            Self::Expanded => LogicalSize::new(EXPANDED_WIDTH, EXPANDED_HEIGHT),
        };
        LogicalSize::new(
            surface.width + HORIZONTAL_GLOW_INSET * 2.0,
            surface.height + VERTICAL_GLOW_INSET * 2.0,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MonitorGeometry {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scale_factor_millis: u32,
}

impl MonitorGeometry {
    fn scale_factor(self) -> f64 {
        f64::from(self.scale_factor_millis) / 1000.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowLayout {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

pub(crate) fn configure_event_loop<T: 'static>(event_loop: &mut EventLoop<T>) {
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        event_loop.set_dock_visibility(false);
        event_loop.set_activate_ignoring_other_apps(false);
    }

    #[cfg(not(target_os = "macos"))]
    let _ = event_loop;
}

pub(crate) fn create_window<T: 'static>(
    event_loop: &EventLoopWindowTarget<T>,
) -> Result<Window, String> {
    let builder = WindowBuilder::new()
        .with_title("A3S Agent Island")
        .with_inner_size(IslandSize::Collapsed.logical_size())
        .with_visible(false)
        .with_focused(false)
        .with_focusable(false)
        .with_resizable(false)
        .with_minimizable(false)
        .with_maximizable(false)
        .with_closable(false)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_visible_on_all_workspaces(true);
    platform_builder(builder)
        .build(event_loop)
        .map_err(|error| format!("create agent island window: {error}"))
}

#[cfg(target_os = "macos")]
fn platform_builder(builder: WindowBuilder) -> WindowBuilder {
    use tao::platform::macos::WindowBuilderExtMacOS;
    builder
        .with_has_shadow(false)
        .with_movable_by_window_background(false)
        .with_automatic_window_tabbing(false)
}

#[cfg(target_os = "windows")]
fn platform_builder(builder: WindowBuilder) -> WindowBuilder {
    use tao::platform::windows::WindowBuilderExtWindows;
    builder
        .with_skip_taskbar(true)
        .with_undecorated_shadow(false)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_builder(builder: WindowBuilder) -> WindowBuilder {
    use tao::platform::unix::WindowBuilderExtUnix;
    builder.with_skip_taskbar(true)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn platform_builder(builder: WindowBuilder) -> WindowBuilder {
    builder
}

pub(crate) fn configure_native_window(window: &Window, interactive: bool) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    window.set_always_on_top(true);
    window.set_focusable(interactive);

    #[cfg(target_os = "macos")]
    unsafe {
        use objc2_app_kit::{NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior};
        use tao::platform::macos::WindowExtMacOS;

        // SAFETY: tao owns this live NSWindow and island setup runs on AppKit's
        // main thread immediately after construction. We only mutate window
        // attributes; ownership remains with tao.
        let ns_window = &*(window.ns_window() as *const NSWindow);
        install_unconstrained_frame_override(ns_window)?;
        ns_window.setLevel(NSStatusWindowLevel);
        ns_window.setCollectionBehavior(
            ns_window.collectionBehavior()
                | NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns_window.setExcludedFromWindowsMenu(true);
        ns_window.setMovable(false);
        ns_window.setMovableByWindowBackground(false);
        ns_window.setHidesOnDeactivate(false);
        ns_window.setCanHide(false);
        ns_window.setHasShadow(false);
    }

    #[cfg(target_os = "windows")]
    unsafe {
        use tao::platform::windows::WindowExtWindows;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_APPWINDOW,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };

        let hwnd = window.hwnd() as *mut core::ffi::c_void;
        let styles = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let styles = if interactive {
            ((styles & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW) & !WS_EX_NOACTIVATE
        } else {
            (styles & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
        };
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, styles as isize);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn install_unconstrained_frame_override(ns_window: &objc2_app_kit::NSWindow) -> Result<(), String> {
    use std::mem;
    use std::sync::OnceLock;

    use objc2::ffi::{class_addMethod, class_replaceMethod, method_getTypeEncoding};
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::sel;
    use objc2_app_kit::NSScreen;
    use objc2_foundation::NSRect;

    static OVERRIDDEN_WINDOW_CLASS: OnceLock<Result<&'static AnyClass, String>> = OnceLock::new();

    unsafe extern "C-unwind" fn unconstrained_frame(
        _window: *mut AnyObject,
        _command: Sel,
        frame: NSRect,
        _screen: *mut NSScreen,
    ) -> NSRect {
        frame
    }

    let object: &AnyObject = ns_window;
    let tao_window_class = object.class();
    let overridden_class = OVERRIDDEN_WINDOW_CLASS.get_or_init(|| {
        let selector = sel!(constrainFrameRect:toScreen:);
        let inherited_method = tao_window_class.instance_method(selector).ok_or_else(|| {
            format!(
                "window class {} does not implement constrainFrameRect:toScreen:",
                tao_window_class.name().to_string_lossy()
            )
        })?;
        // Keep AppKit's exact method encoding, including architecture-specific
        // NSRect layout details, while adding an override to TaoWindow itself.
        let encoding = unsafe { method_getTypeEncoding(inherited_method) };
        if encoding.is_null() {
            return Err(
                "constrainFrameRect:toScreen: has no Objective-C type encoding".to_string(),
            );
        }
        let callback = unconstrained_frame
            as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, NSRect, *mut NSScreen) -> NSRect;
        // SAFETY: Objective-C IMP erases the callback signature. `callback`
        // exactly matches `constrainFrameRect:toScreen:` and the inherited
        // encoding is passed unchanged. Island mode owns the process's only
        // Tao window, and setup runs on AppKit's main thread before first show.
        unsafe {
            let implementation = mem::transmute::<
                unsafe extern "C-unwind" fn(*mut AnyObject, Sel, NSRect, *mut NSScreen) -> NSRect,
                Imp,
            >(callback);
            let class = std::ptr::from_ref(tao_window_class).cast_mut();
            if !class_addMethod(class, selector, implementation, encoding).as_bool() {
                let _ = class_replaceMethod(class, selector, implementation, encoding);
            }
        }
        Ok(tao_window_class)
    });
    let overridden_class = overridden_class.as_ref().map_err(Clone::clone)?;
    // AppKit may isa-swizzle the instance while WKWebView is attaching. A
    // runtime subclass inherits the override installed on TaoWindow, so only
    // reject an unrelated replacement class.
    let inherits_override =
        std::iter::successors(Some(tao_window_class), |class| class.superclass())
            .any(|class| std::ptr::eq(class, *overridden_class));
    if !inherits_override {
        return Err(format!(
            "agent island window class {} does not inherit overridden class {}",
            tao_window_class.name().to_string_lossy(),
            overridden_class.name().to_string_lossy(),
        ));
    }
    Ok(())
}

pub(crate) fn show_without_focus(window: &Window) {
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2_app_kit::NSWindow;
        use tao::platform::macos::WindowExtMacOS;

        // SAFETY: the pointer is tao's live NSWindow and this runs on the main
        // thread. orderFrontRegardless shows it without making it key.
        let ns_window = &*(window.ns_window() as *const NSWindow);
        ns_window.orderFrontRegardless();
    }

    #[cfg(target_os = "windows")]
    unsafe {
        use tao::platform::windows::WindowExtWindows;
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};

        let _ = ShowWindow(window.hwnd() as *mut core::ffi::c_void, SW_SHOWNOACTIVATE);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    window.set_visible(true);
}

pub(crate) fn resize_and_center(window: &Window, size: IslandSize) {
    #[cfg(target_os = "macos")]
    if resize_and_center_macos(window, size) {
        return;
    }

    let Some(monitor) = window
        .primary_monitor()
        .or_else(|| window.current_monitor())
    else {
        window.set_inner_size(size.logical_size());
        return;
    };
    let geometry = MonitorGeometry {
        position: monitor.position(),
        size: monitor.size(),
        scale_factor_millis: (monitor.scale_factor() * 1000.0).round().max(1.0) as u32,
    };
    let layout = layout_for_monitor(geometry, size);
    if window.inner_size() != layout.size {
        window.set_inner_size(layout.size);
    }
    let position_changed = match window.outer_position() {
        Ok(position) => position != layout.position,
        Err(_) => true,
    };
    if position_changed {
        window.set_outer_position(layout.position);
    }
}

#[cfg(target_os = "macos")]
fn resize_and_center_macos(window: &Window, size: IslandSize) -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSScreen, NSWindow};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use tao::platform::macos::WindowExtMacOS;

    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    // `mainScreen` follows the screen containing the user's active/key window,
    // which is a better island target than the helper's hidden startup frame.
    let ns_window = unsafe { &*(window.ns_window() as *const NSWindow) };
    let Some(screen) = NSScreen::mainScreen(mtm).or_else(|| ns_window.screen()) else {
        return false;
    };
    let screen_frame = screen.frame();
    let requested = size.logical_size();
    let width = requested
        .width
        .min((screen_frame.size.width - SCREEN_INSET * 2.0).max(1.0));
    let top_offset = TOP_MARGIN - VERTICAL_GLOW_INSET;
    let height = requested
        .height
        .min((screen_frame.size.height - top_offset - SCREEN_INSET).max(1.0));

    // Tao dispatches setContentSize/setFrameTopLeftPoint asynchronously and
    // AppKit constrains the latter to visibleFrame (below the menu bar). Apply
    // the complete borderless frame synchronously against NSScreen.frame so a
    // status-level island can occupy the physical top edge without size drift.
    let x = screen_frame.origin.x + (screen_frame.size.width - width) / 2.0;
    let y = screen_frame.origin.y + screen_frame.size.height - top_offset - height;
    let target = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
    let current = ns_window.frame();
    let changed = (current.origin.x - target.origin.x).abs() > 0.25
        || (current.origin.y - target.origin.y).abs() > 0.25
        || (current.size.width - target.size.width).abs() > 0.25
        || (current.size.height - target.size.height).abs() > 0.25;
    if changed {
        // Frame mutation is synchronous even when redisplay is deferred.
        // WebKit can then commit the host and CSS motion in one display pass
        // instead of forcing a redundant intermediate repaint.
        ns_window.setFrame_display(target, false);
    }
    true
}

fn layout_for_monitor(monitor: MonitorGeometry, requested: IslandSize) -> WindowLayout {
    let scale = monitor.scale_factor().max(0.001);
    let requested = requested.logical_size();
    let max_width = (f64::from(monitor.size.width) / scale - SCREEN_INSET * 2.0).max(1.0);
    let top_offset = TOP_MARGIN - VERTICAL_GLOW_INSET;
    let max_height = (f64::from(monitor.size.height) / scale - top_offset - SCREEN_INSET).max(1.0);
    let logical_width = requested.width.min(max_width);
    let logical_height = requested.height.min(max_height);
    let width = (logical_width * scale).round().max(1.0) as u32;
    let height = (logical_height * scale).round().max(1.0) as u32;
    let x_offset = (i64::from(monitor.size.width) - i64::from(width)) / 2;
    let x = i64::from(monitor.position.x).saturating_add(x_offset);
    let y = i64::from(monitor.position.y).saturating_add((top_offset * scale).round() as i64);
    WindowLayout {
        position: PhysicalPosition::new(
            x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        ),
        size: PhysicalSize::new(width, height),
    }
}

pub(crate) fn warn_if_wayland_positioning_is_degraded() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var("XDG_SESSION_TYPE").is_ok_and(|value| value == "wayland")
    {
        eprintln!(
            "a3s-webview: the Wayland compositor may ignore global island positioning; use an XWayland session for exact top-center placement"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_collapsed_window_in_physical_pixels_at_2x() {
        let layout = layout_for_monitor(
            MonitorGeometry {
                position: PhysicalPosition::new(0, 0),
                size: PhysicalSize::new(3024, 1964),
                scale_factor_millis: 2000,
            },
            IslandSize::Collapsed,
        );
        assert_eq!(layout.size, PhysicalSize::new(976, 248));
        assert_eq!(layout.position, PhysicalPosition::new(1024, -52));
    }

    #[test]
    fn accounts_for_negative_monitor_origins() {
        let layout = layout_for_monitor(
            MonitorGeometry {
                position: PhysicalPosition::new(-1920, -120),
                size: PhysicalSize::new(1920, 1080),
                scale_factor_millis: 1000,
            },
            IslandSize::Expanded,
        );
        assert_eq!(layout.size, PhysicalSize::new(656, 424));
        assert_eq!(layout.position, PhysicalPosition::new(-1288, -146));
    }

    #[test]
    fn clamps_to_tiny_displays_without_underflow() {
        let layout = layout_for_monitor(
            MonitorGeometry {
                position: PhysicalPosition::new(50, 80),
                size: PhysicalSize::new(200, 100),
                scale_factor_millis: 1250,
            },
            IslandSize::Expanded,
        );
        assert!(layout.size.width <= 180);
        assert!(layout.size.height <= 123);
        assert!(layout.position.x >= 50);
    }

    #[test]
    fn glow_bleed_contains_the_collapsed_aura_before_native_clipping() {
        let size = IslandSize::Collapsed.logical_size();

        assert_eq!(size, LogicalSize::new(488.0, 124.0));
        const {
            assert!(HORIZONTAL_GLOW_INSET >= 46.0);
            assert!(VERTICAL_GLOW_INSET >= 30.0);
        }
    }
}
