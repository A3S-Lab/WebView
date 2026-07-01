//! `a3s-webview` — a tiny native WebView window helper for the a3s code TUI.
//!
//! The TUI is a terminal app and can't embed a WebView in its text grid, so when
//! 书安OS's progressive API returns a `view` object (url + size) that is a
//! *partial* page meant for a sized popup (not a full browser tab), it spawns this:
//!
//! ```text
//! a3s-webview --url https://os.example.com/embed/x --width 720 --height 520 --title "..."
//! ```
//!
//! It opens one native window at the requested size, loads the URL, and runs
//! until the window is closed.
//!
//! ## Navigation controls
//! back / forward / reload are exposed as buttons. On macOS they are NATIVE
//! `NSButton`s placed on the trailing (right) side of the window titlebar via an
//! `NSTitlebarAccessoryViewController` (see the `macos_titlebar` module); a click
//! posts a `UserEvent` through the tao event loop, and the run-closure runs the
//! matching `history.back()` / `history.forward()` / `location.reload()` in the
//! webview. On Linux/Windows the same three controls are injected as an HTML
//! toolbar (`NAV_TOOLBAR_SCRIPT`).
//!
//! ## Auth
//! The 书安OS web app authenticates from `localStorage`, not a cookie — so a
//! freshly opened WebView would land on the login page. Its `restoreAuth` (see
//! apps/web `models/auth.model.ts`) requires `auth_token`/`access_token`, an
//! optional `refresh_token`, AND an `auth_user` object — a token alone is not
//! enough. Before navigation a wry initialization script seeds the tokens from
//! the `A3S_OS_TOKEN` / `A3S_OS_REFRESH_TOKEN` env vars the TUI exports (so they
//! never appear in argv / `ps`), then resolves the current user with a
//! same-origin `GET /api/v1/users/me` and seeds `auth_user` too. Override the token
//! env name with `--token-env`, disable all of it with `--no-auth`.
//! `--header 'Name: Value'` (repeatable) still attaches raw request headers.

use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::{
    http::{HeaderMap, HeaderName, HeaderValue},
    WebViewBuilder,
};

#[cfg(target_os = "macos")]
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use tao::{event_loop::EventLoopProxy, platform::macos::WindowExtMacOS};

struct Args {
    url: String,
    width: f64,
    height: f64,
    title: String,
    headers: HeaderMap,
    token_env: String,
    no_auth: bool,
}

const USAGE: &str = "usage: a3s-webview --url <http(s)://…> [--width N] [--height N] \
[--title T] [--header 'Name: Value']… [--token-env NAME] [--no-auth]";

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut url: Option<String> = None;
    let mut width = 900.0_f64;
    let mut height = 680.0_f64;
    let mut title = String::from("渐进式UI");
    let mut headers = HeaderMap::new();
    let mut token_env = String::from("A3S_OS_TOKEN");
    let mut no_auth = false;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut next = || it.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--url" => url = Some(next()?),
            "--title" => title = next()?,
            "--token-env" => token_env = next()?,
            "--no-auth" => no_auth = true,
            "--width" => {
                width = next()?
                    .parse()
                    .map_err(|_| "--width must be a number".to_string())?
            }
            "--height" => {
                height = next()?
                    .parse()
                    .map_err(|_| "--height must be a number".to_string())?
            }
            "--header" => {
                let raw = next()?;
                let (name, value) = raw
                    .split_once(':')
                    .ok_or_else(|| format!("--header must be 'Name: Value', got {raw:?}"))?;
                let name = HeaderName::from_bytes(name.trim().as_bytes())
                    .map_err(|e| format!("bad header name: {e}"))?;
                let value = HeaderValue::from_str(value.trim())
                    .map_err(|e| format!("bad header value: {e}"))?;
                headers.insert(name, value);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let url = url.ok_or("--url is required")?;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("--url must start with http:// or https://".to_string());
    }
    Ok(Args {
        url,
        // Clamp so a bad API size can't open a 1px or 50000px window.
        width: width.clamp(240.0, 4000.0),
        height: height.clamp(180.0, 3000.0),
        title,
        headers,
        token_env,
        no_auth,
    })
}

/// A string escaped into a single-quoted JS string literal (incl. the quotes).
fn js_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// JS run at document-start (before the page's own scripts) that injects the full
/// 书安OS session into `localStorage` so the SPA's `restoreAuth` loads
/// authenticated. That needs THREE things, not just the token: `auth_token` /
/// `access_token`, an optional `refresh_token`, and an `auth_user` object (a user
/// with an `id`) — without `auth_user` the SPA clears auth and shows the login
/// page. We seed the tokens from the inherited env, then resolve the current user
/// with a same-origin `GET /api/v1/users/me` (Bearer = the token) and store it too.
/// The fetch is synchronous on purpose: it must finish before the page's own
/// scripts run, so a deferred fetch would race `restoreAuth`.
///
/// Returns `None` for an empty token. `token` is the access token; `refresh` the
/// optional refresh token.
fn auth_init_script(token: &str, refresh: Option<&str>) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    let tok = js_str(token);
    let refresh_line = match refresh {
        Some(r) if !r.is_empty() => {
            format!("localStorage.setItem('refresh_token',{});", js_str(r))
        }
        _ => String::new(),
    };
    // ponytail: sync XHR is deprecated but is the reliable way to seed `auth_user`
    // before the SPA boots; this is a controlled, same-origin embedded webview.
    Some(format!(
        "try{{\
           localStorage.setItem('access_token',{tok});\
           localStorage.setItem('auth_token',{tok});\
           {refresh_line}\
           if(!localStorage.getItem('auth_user')){{\
             var x=new XMLHttpRequest();\
             x.open('GET','/api/v1/users/me',false);\
             x.setRequestHeader('Authorization','Bearer '+{tok});\
             x.send();\
             if(x.status>=200&&x.status<300){{\
               var r=JSON.parse(x.responseText);\
               var u=(r&&r.data)?r.data:r;\
               localStorage.setItem('auth_user',JSON.stringify(u));\
             }}\
           }}\
         }}catch(e){{}}"
    ))
}

/// JS injected into every popup on Linux/Windows: a small fixed top-right toolbar
/// with back / forward / reload, wired to the page history + reload. On macOS the
/// equivalent lives in the NATIVE window titlebar (see the `macos_titlebar`
/// module), so this script is not injected there. Added on `DOMContentLoaded`
/// (so `document.body` exists) and appended to the documentElement as a fallback
/// so an SPA re-rendering its root can't drop it.
#[cfg_attr(target_os = "macos", allow(dead_code))]
const NAV_TOOLBAR_SCRIPT: &str = r#"
window.addEventListener('DOMContentLoaded',function(){try{
var bar=document.createElement('div');
bar.style.cssText='position:fixed;top:10px;right:10px;z-index:2147483647;display:flex;gap:4px;padding:4px;background:rgba(30,30,40,.78);border-radius:8px;box-shadow:0 1px 6px rgba(0,0,0,.35);font-family:system-ui,sans-serif';
function mk(l,t,f){var b=document.createElement('button');b.textContent=l;b.title=t;
b.style.cssText='all:unset;cursor:pointer;width:26px;height:26px;line-height:26px;text-align:center;color:#e6e6f0;font-size:15px;border-radius:6px';
b.onmouseenter=function(){b.style.background='rgba(255,255,255,.15)'};
b.onmouseleave=function(){b.style.background='transparent'};
b.onclick=f;return b;}
bar.appendChild(mk('←','Back',function(){history.back()}));
bar.appendChild(mk('→','Forward',function(){history.forward()}));
bar.appendChild(mk('↻','Reload',function(){location.reload()}));
(document.body||document.documentElement).appendChild(bar);
}catch(e){}});
"#;

/// A navigation command posted from a native macOS titlebar button into the tao
/// event loop; the run-closure turns it into the matching `webview.evaluate_script`.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum UserEvent {
    Back,
    Forward,
    Reload,
}

/// The tao event-loop proxy the native titlebar buttons post through. Set once at
/// startup (main thread) before the buttons can fire; the objc action methods read
/// it to translate a click into a `UserEvent`. `EventLoopProxy<UserEvent>` is
/// `Send + Sync` (macOS impl wraps a `crossbeam_channel::Sender`), so it is a valid
/// `static`.
#[cfg(target_os = "macos")]
static NAV_PROXY: OnceLock<EventLoopProxy<UserEvent>> = OnceLock::new();

/// Native macOS titlebar navigation buttons (back / forward / reload), placed on
/// the trailing side of the window titlebar via an
/// `NSTitlebarAccessoryViewController`. This replaces the HTML toolbar on macOS.
#[cfg(target_os = "macos")]
mod macos_titlebar {
    use super::{UserEvent, NAV_PROXY};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, Sel};
    use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSButton, NSCellImagePosition, NSImage, NSImageScaling, NSLayoutAttribute,
        NSTitlebarAccessoryViewController, NSView, NSWindow,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    use std::ffi::c_void;

    define_class!(
        // SAFETY:
        // - The superclass `NSObject` has no subclassing requirements.
        // - `NavTarget` does not implement `Drop`.
        // - `MainThreadOnly` is correct: it is only created and messaged on the
        //   main thread (from the AppKit run loop / the setup call in `main`).
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "A3sWebViewNavTarget"]
        pub(crate) struct NavTarget;

        impl NavTarget {
            #[unsafe(method(navBack:))]
            fn nav_back(&self, _sender: Option<&AnyObject>) {
                Self::post(UserEvent::Back);
            }

            #[unsafe(method(navForward:))]
            fn nav_forward(&self, _sender: Option<&AnyObject>) {
                Self::post(UserEvent::Forward);
            }

            #[unsafe(method(navReload:))]
            fn nav_reload(&self, _sender: Option<&AnyObject>) {
                Self::post(UserEvent::Reload);
            }
        }
    );

    impl NavTarget {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            // No custom ivars: the proxy lives in the `NAV_PROXY` static, so the
            // ivars type defaults to `()`.
            let this = mtm.alloc::<Self>().set_ivars(());
            unsafe { msg_send![super(this), init] }
        }

        fn post(event: UserEvent) {
            if let Some(proxy) = NAV_PROXY.get() {
                // The loop is only gone during teardown; dropping the event then is fine.
                let _ = proxy.send_event(event);
            }
        }
    }

    /// Build three native icon buttons and attach them to the trailing side of the
    /// window titlebar. Returns the custom target, which the caller MUST keep alive
    /// for the lifetime of the window: `NSControl` holds its `target` weakly, so
    /// dropping it turns the next click into a use-after-free.
    ///
    /// # Safety
    /// `ns_window_ptr` must be the live `NSWindow` pointer returned by tao's
    /// `WindowExtMacOS::ns_window()`, and this must be called on the main thread.
    pub(crate) unsafe fn install(ns_window_ptr: *mut c_void) -> Retained<NavTarget> {
        let mtm = MainThreadMarker::new().expect("titlebar setup must run on the main thread");
        // Borrow (do NOT take ownership of) the NSWindow that tao already owns.
        let window: &NSWindow = unsafe { &*(ns_window_ptr as *const NSWindow) };

        let target = NavTarget::new(mtm);
        // Bind the &AnyObject coercion once (deref: Retained -> NavTarget -> NSObject -> AnyObject).
        let target_any: &AnyObject = &target;

        // Lay the buttons out by explicit frame: an NSTitlebarAccessoryViewController
        // renders EMPTY if its view has no concrete size, so we avoid Auto Layout
        // (a frame-less NSStackView collapses to zero) and size everything by hand.
        // Buttons fill the titlebar height (icon centered via ImageOnly), borderless
        // template icons at a small point size so they read as native chrome.
        let (bw, bh) = (30.0_f64, 28.0_f64);
        // symbol = SF Symbol name (macOS 11+); fallback = accessibility label AND
        // the text glyph used if the symbol image is unavailable.
        let make = |symbol: &str, fallback: &str, action: Sel, x: f64| -> Retained<NSButton> {
            let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &NSString::from_str(symbol),
                Some(&NSString::from_str(fallback)),
            );
            if let Some(img) = &image {
                img.setTemplate(true); // tint to the titlebar's label colour
                img.setSize(NSSize::new(14.0, 14.0)); // small, chrome-sized glyph
            }
            let button = match &image {
                Some(image) => unsafe {
                    NSButton::buttonWithImage_target_action(
                        image,
                        Some(target_any),
                        Some(action),
                        mtm,
                    )
                },
                None => unsafe {
                    NSButton::buttonWithTitle_target_action(
                        &NSString::from_str(fallback),
                        Some(target_any),
                        Some(action),
                        mtm,
                    )
                },
            };
            // Borderless icon-only look; the icon centers in the full-height button.
            button.setBordered(false);
            button.setImagePosition(NSCellImagePosition::ImageOnly);
            button.setImageScaling(NSImageScaling::ScaleProportionallyDown);
            button.setFrame(NSRect::new(NSPoint::new(x, 0.0), NSSize::new(bw, bh)));
            button
        };

        let back = make("chevron.backward", "←", sel!(navBack:), 0.0);
        let forward = make("chevron.forward", "→", sel!(navForward:), bw);
        let reload = make("arrow.clockwise", "↻", sel!(navReload:), bw * 2.0);

        // Manually-framed container so the accessory has a concrete size. It is
        // retained by the accessory VC (setView), which is retained by the window.
        let container = NSView::new(mtm);
        container.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(bw * 3.0, bh),
        ));
        container.addSubview(&back);
        container.addSubview(&forward);
        container.addSubview(&reload);

        let accessory = NSTitlebarAccessoryViewController::new(mtm);
        accessory.setView(&container);
        // Trailing == RTL-aware right edge of the titlebar. Use ::Right for a
        // hard right-edge if RTL awareness is unwanted.
        accessory.setLayoutAttribute(NSLayoutAttribute::Trailing);
        window.addTitlebarAccessoryViewController(&accessory);

        target
    }
}

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("a3s-webview: {e}\n{USAGE}");
            std::process::exit(2);
        }
    };

    let auth_script = if args.no_auth {
        None
    } else {
        let refresh = std::env::var("A3S_OS_REFRESH_TOKEN").ok();
        std::env::var(&args.token_env)
            .ok()
            .and_then(|t| auth_init_script(&t, refresh.as_deref()))
    };
    let auth = auth_script.unwrap_or_default();
    // macOS renders navigation in the native titlebar, so the HTML toolbar is only
    // injected on the other platforms.
    #[cfg(target_os = "macos")]
    let init_script = auth;
    #[cfg(not(target_os = "macos"))]
    let init_script = format!("{auth}{NAV_TOOLBAR_SCRIPT}");

    // Carry `UserEvent`s so native titlebar clicks can be routed back into the loop.
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title(&args.title)
        .with_inner_size(LogicalSize::new(args.width, args.height))
        // Spawned detached from the TUI — bring the popup to the front.
        .with_focused(true)
        .build(&event_loop)
        .expect("create window");

    // macOS: install native titlebar buttons on the main thread, after the window
    // exists. Keep `_nav_target` alive for the whole run (weak target-action).
    #[cfg(target_os = "macos")]
    let _nav_target = {
        // Only ever one window/proxy here; a second set() would be ignored.
        let _ = NAV_PROXY.set(event_loop.create_proxy());
        unsafe { macos_titlebar::install(window.ns_window()) }
    };

    let mut builder = WebViewBuilder::new()
        .with_url(&args.url)
        .with_initialization_script(&init_script);
    if !args.headers.is_empty() {
        builder = builder.with_headers(args.headers);
    }
    // Owned by the run-closure so titlebar UserEvents can drive evaluate_script.
    let webview = builder.build(&window).expect("create webview");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(nav) => {
                let js = match nav {
                    UserEvent::Back => "history.back()",
                    UserEvent::Forward => "history.forward()",
                    UserEvent::Reload => "location.reload()",
                };
                let _ = webview.evaluate_script(js);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Result<Args, String> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn requires_http_url() {
        assert!(args(&["--url", "file:///etc/passwd"]).is_err());
        assert!(args(&[]).is_err()); // --url required
        assert!(args(&["--url", "https://os.example.com/x"]).is_ok());
    }

    #[test]
    fn parses_size_title_and_clamps() {
        let a = args(&[
            "--url",
            "https://x",
            "--width",
            "50000",
            "--height",
            "5",
            "--title",
            "T",
        ])
        .unwrap();
        assert_eq!(a.title, "T");
        assert_eq!(a.width, 4000.0); // clamped down
        assert_eq!(a.height, 180.0); // clamped up
    }

    #[test]
    fn parses_headers() {
        let a = args(&[
            "--url",
            "https://x",
            "--header",
            "Authorization: Bearer abc",
        ])
        .unwrap();
        assert_eq!(a.headers.get("authorization").unwrap(), "Bearer abc");
        assert!(args(&["--url", "https://x", "--header", "no-colon"]).is_err());
    }

    #[test]
    fn auth_script_seeds_tokens_user_and_escapes() {
        let s = auth_init_script("tok'1", Some("ref2")).unwrap();
        assert!(s.contains("access_token") && s.contains("auth_token"));
        assert!(s.contains("tok\\'1")); // single quote escaped
                                        // resolves + seeds the required user object
        assert!(s.contains("/api/v1/users/me") && s.contains("auth_user"));
        // refresh token seeded when present
        assert!(s.contains("refresh_token") && s.contains("ref2"));
        assert!(auth_init_script("", None).is_none());
    }

    #[test]
    fn auth_script_omits_refresh_when_absent() {
        let s = auth_init_script("tok", None).unwrap();
        assert!(!s.contains("refresh_token"));
        assert!(s.contains("auth_user")); // user resolution still present
    }

    #[test]
    fn nav_toolbar_wires_back_forward_reload() {
        assert!(NAV_TOOLBAR_SCRIPT.contains("history.back()"));
        assert!(NAV_TOOLBAR_SCRIPT.contains("history.forward()"));
        assert!(NAV_TOOLBAR_SCRIPT.contains("location.reload()"));
        // injected after the DOM exists, fixed in the top-right corner
        assert!(NAV_TOOLBAR_SCRIPT.contains("DOMContentLoaded"));
        assert!(
            NAV_TOOLBAR_SCRIPT.contains("position:fixed")
                && NAV_TOOLBAR_SCRIPT.contains("right:10px")
        );
    }
}