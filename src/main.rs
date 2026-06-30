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
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::{
    http::{HeaderMap, HeaderName, HeaderValue},
    WebViewBuilder,
};

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

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("a3s-webview: {e}\n{USAGE}");
            std::process::exit(2);
        }
    };

    let init_script = if args.no_auth {
        None
    } else {
        let refresh = std::env::var("A3S_OS_REFRESH_TOKEN").ok();
        std::env::var(&args.token_env)
            .ok()
            .and_then(|t| auth_init_script(&t, refresh.as_deref()))
    };

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(&args.title)
        .with_inner_size(LogicalSize::new(args.width, args.height))
        // Spawned detached from the TUI — bring the popup to the front.
        .with_focused(true)
        .build(&event_loop)
        .expect("create window");

    let mut builder = WebViewBuilder::new().with_url(&args.url);
    if let Some(script) = &init_script {
        builder = builder.with_initialization_script(script);
    }
    if !args.headers.is_empty() {
        builder = builder.with_headers(args.headers);
    }
    let _webview = builder.build(&window).expect("create webview");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
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
}
