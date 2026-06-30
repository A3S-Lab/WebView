# a3s-webview

A tiny native WebView window helper for the [a3s code](https://github.com/A3S-Lab/Cli) TUI.

The TUI is a terminal app and can't embed a WebView in its text grid. When
书安OS's progressive API returns a `viewUrl` that is a *partial* page meant for a
sized popup (not a full browser tab), the TUI spawns this helper: it opens one
native window at the requested size, loads the URL authenticated, and runs until
the window is closed. Plain links still open in the user's browser.

## Install

```bash
brew install a3s-lab/tap/a3s-webview
# or build from source:
cargo install --git https://github.com/A3S-Lab/WebView
```

`brew install a3s-lab/tap/a3s` pulls this in automatically on macOS (it's a
`depends_on` of the `a3s` formula).

## Usage

```text
a3s-webview --url <http(s)://…> [--width N] [--height N] [--title T]
            [--header 'Name: Value']… [--token-env NAME] [--no-auth]
```

- `--url` (required) — `http://` or `https://` only.
- `--width` / `--height` — window size in logical px (clamped to 240–4000 × 180–3000).
- `--title` — window title (default `a3s · OS`).
- `--header 'Name: Value'` — extra request header, repeatable.
- `--token-env NAME` — env var holding the session token (default `A3S_OS_TOKEN`).
- `--no-auth` — don't seed any auth token.

## Auth

The 书安OS web app authenticates from a token in `localStorage` (`access_token` /
`auth_token`), not a cookie — so a freshly opened WebView would land on the login
page. Before navigation the helper seeds those keys via a wry initialization
script, reading the token from the `A3S_OS_TOKEN` env var the TUI exports (so it
never appears in `argv` / `ps`).

## Platform support

| Platform | Backend | Notes |
|---|---|---|
| macOS | WKWebView | System framework — no extra deps. Shipped via Homebrew. |
| Linux | WebKitGTK | Needs `libwebkit2gtk-4.1` installed; build from source. |
| Windows | WebView2 | Runtime ships with Windows 10+; build from source. |

Where the helper isn't installed, the a3s TUI degrades gracefully to opening the
link in the system browser.

## License

MIT — see [LICENSE](LICENSE).
