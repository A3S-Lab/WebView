//! `a3s-webview` — a tiny native WebView window helper for the a3s code TUI.
//!
//! The TUI is a terminal app and can't embed a WebView in its text grid, so when
//! 书安OS's progressive API returns a `viewUrl` that is a *partial* page meant for
//! a sized popup (not a full browser tab), the TUI spawns this helper:
//!
//! ```text
//! a3s-webview --url https://os.example.com/embed/x --width 720 --height 520 --title "..."
//! ```
//!
//! It opens one native window at the requested size, loads the URL, and runs
//! until the window is closed.
//!
//! ## Auth
//! The 书安OS web app authenticates from a token in `localStorage` (`access_token`
//! / `auth_token`; see apps/web `auth-headers.ts`), not a cookie — so a freshly
//! opened WebView would land on the login page. Before navigation we seed those
//! keys via a wry initialization script, reading the token from the
//! `A3S_OS_TOKEN` env var the TUI already exports (so it never appears in argv /
//! `ps`). Override the env name with `--token-env`, disable with `--no-auth`.
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
    let mut title = String::from("a3s · OS");
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

/// JS run at document-start (before the page's own scripts) that seeds the OS
/// session token into localStorage so the SPA loads authenticated. `token` is
/// escaped into a JS string literal. Returns `None` for an empty token.
fn auth_init_script(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    let esc = token.replace('\\', "\\\\").replace('\'', "\\'");
    Some(format!(
        "try{{localStorage.setItem('access_token','{esc}');\
         localStorage.setItem('auth_token','{esc}');}}catch(e){{}}"
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
        std::env::var(&args.token_env)
            .ok()
            .and_then(|t| auth_init_script(&t))
    };

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(&args.title)
        .with_inner_size(LogicalSize::new(args.width, args.height))
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
    fn auth_script_seeds_both_keys_and_escapes() {
        let s = auth_init_script("tok'1").unwrap();
        assert!(s.contains("access_token") && s.contains("auth_token"));
        assert!(s.contains("tok\\'1")); // single quote escaped
        assert!(auth_init_script("").is_none());
    }
}
