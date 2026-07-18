# a3s-webview

The native window companion for [A3S Code](https://github.com/A3S-Lab/Cli).
It owns two desktop surfaces that must remain independent of the terminal:

- sized RemoteUI and trusted local-report WebView windows; and
- the transparent, always-on-top Agent Island at the physical screen's top
  center.

Plain links still open in the user's browser. Failure to start either native
surface never prevents the A3S Code TUI from running.

## Install

```bash
brew install a3s-lab/tap/a3s-webview
# or build from source:
cargo install --git https://github.com/A3S-Lab/WebView
```

Published `a3s` release archives and the Homebrew `a3s` formula install this
binary beside the CLI. The standalone formula remains available for source or
Cargo installations of the CLI.

## Usage

```text
a3s-webview --url <http(s)://…|file://…> [--width N] [--height N] [--title T]
            [--header 'Name: Value']… [--token-env NAME] [--no-auth]

a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>
```

- `--url` (RemoteUI mode) — `http://`, `https://`, or a trusted local `file://`
  report.
- `--width` / `--height` — window size in logical px (clamped to 240–4000 × 180–3000).
- `--title` — window title (default `A3S RemoteUI`).
- `--header 'Name: Value'` — extra request header, repeatable.
- `--token-env NAME` — env var holding the session token (default `A3S_OS_TOKEN`).
- `--no-auth` — don't seed any auth token.
- `--agent-island` — render the private system-agent snapshot in a separate,
  frameless OS window. Snapshot and lock paths must be absolute siblings in one
  private per-user directory. The lock admits only one island process. A fresh
  snapshot without any exact non-idle A3S lifecycle closes the helper; inferred
  process rows do not keep an otherwise idle island alive.

## Agent Island UI

The island is implemented by this standalone Tao/Wry helper. It embeds offline
HTML, CSS, and JavaScript in the platform WebView; it does not render through
`a3s-tui`, the A3S GUI crate, React, or Next.js. The compact `296 × 44` pill
expands to a bounded, scrollable `560 × 360` detail view without taking keyboard
focus.

The user preference defaults to enabled and is persisted by A3S Code.
`/island on|off|status` controls it from the TUI. The expanded view also offers
`Turn off`, which writes the same private opt-out marker before the helper
exits; `/island on` restores the surface.

Each row uses original robot geometry with a color palette derived from the
agent vendor, not a copied logo or mascot. Exact lifecycle rows show agent,
task, state, workspace, and elapsed time; terminal durations freeze at the
reported finish time. Live grants add only the controls the owning TUI can
currently execute: `Allow` / `Always` / `Deny` for approval, `Stop` for the
parent stream, and `Cancel` for a running child. Inferred process rows never
receive controls.

The detail view is an attention workbench with live `All`, `Needs you`,
`Running`, and `Recent` counts. `Needs you` covers approval waits, input waits,
and failures; `Recent` covers completed, failed, and cancelled outcomes, so a
failure intentionally belongs to both. Filtering a child retains its parent as
labeled context, and each parent reports settled direct children against its
total. Process-only evidence remains available under `All` but is never counted
as running.

Each previously unseen actionable approval or input identity selects
`Needs you` and expands the island once without activating it. Continued
heartbeats with the same control token do not reopen it after a manual collapse,
and a later new request does. Manual filter choices remain stable until a new
attention identity arrives.

Any exact `planning` or `working` row enables the animated multicolor neon
border. Reduced-motion mode keeps a static color border, and backgrounded
WebViews receive a low-frequency direct repaint so WebKit timer throttling does
not freeze the breathing effect.

Control clicks are authorized against the latest sanitized snapshot and written
as bounded, versioned requests to a private sibling queue. On Unix the queue is
`0700` and request files are `0600`. The target TUI revalidates the short-lived,
one-shot grant against its current session, activity, and tool/task context
before using the existing approval, interruption, or child-cancellation path.

## Auth

The A3S OS web app authenticates from a token in `localStorage` (`access_token` /
`auth_token`), not a cookie — so a freshly opened WebView would land on the login
page. Before navigation the helper seeds those keys via a wry initialization
script, reading the token from the `A3S_OS_TOKEN` env var the TUI exports (so it
never appears in `argv` / `ps`).

## Platform support

| Platform | Backend | Notes |
|---|---|---|
| macOS | WKWebView | System framework; native island joins Spaces and full-screen auxiliary windows without entering the Dock. |
| Linux | WebKitGTK | Needs `libwebkit2gtk-4.1`; X11 supports top-center/keep-above hints. Standard Wayland may ignore global placement. |
| Windows | WebView2 | Runtime ships with supported Windows releases; the island uses a non-activating tool window outside the taskbar and Alt-Tab list. |

Release workflows build all CLI targets. Where the helper is not installed,
RemoteUI degrades to the system browser and Agent Island startup is skipped.

## License

MIT — see [LICENSE](LICENSE).
