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
  private per-user directory. The shared advisory lock admits only one island
  process for the current user, even when multiple TUI publishers request it. A
  fresh snapshot without an exact non-idle A3S lifecycle or recognized
  coding-agent process closes the helper; fresh process rows trigger and keep
  the island alive.

## Agent Island UI

The island is implemented by this standalone Tao/Wry helper. It embeds offline
HTML, CSS, and JavaScript in the platform WebView; it does not render through
`a3s-tui`, the A3S GUI crate, React, or Next.js. The compact `392 × 60` pill
expands to a bounded, scrollable `560 × 360` detail surface. Its transparent
native window adds 48 logical pixels of horizontal and 32 pixels of vertical
bleed on every side (`488 × 124` collapsed and `656 × 424` expanded). The
wide aura fades to transparent inside that bleed instead of exposing the
native window's rectangular clipping boundary. The collapsed surface shows
the primary agent, task context, state, live elapsed time, running/total
counts, and needs-you count. Automatic attention expansion does not steal
keyboard focus; the expanded window becomes focusable so a user can operate
controls and type a reply directly in the island.

The user preference defaults to enabled and is persisted by A3S Code.
`/island on|off|status` controls it from the TUI. The expanded view also offers
`Turn off`, which writes the same private opt-out marker before the helper
exits; `/island on` restores the surface.

Each row uses original robot geometry with a color palette derived from the
agent vendor, not a copied logo or mascot. Exact lifecycle rows show agent,
task, state, workspace, and elapsed time; terminal durations freeze at the
reported finish time. An approval row displays its sanitized, bounded reason
and uses larger `Allow` / `Always` / `Deny` buttons. Live grants add only the
controls the owning TUI can currently execute: approval actions, `Stop` for the
parent stream, `Cancel` for a running child, and a direct reply composer on an
eligible exact parent row. Inferred process rows never receive controls.

The detail view is an attention workbench with live `All`, `Needs you`,
`Running`, and `Recent` counts. `Needs you` covers approval waits, input waits,
and failures; `Recent` covers completed, failed, and cancelled outcomes, so a
failure intentionally belongs to both. Filtering a child retains its parent as
labeled context, and each parent reports settled direct children against its
total. Process-only evidence remains explicitly inferred under `All` and also
appears under `Running` because a recognized process is live running evidence;
it never masquerades as authoritative lifecycle state.

Each previously unseen actionable approval or input identity selects
`Needs you` and expands the island once without activating it. Continued
heartbeats with the same control token do not reopen it after a manual collapse,
and a later new request does. Manual filter choices remain stable until a new
attention identity arrives.

Any exact `planning` / `working` row or recognized coding-agent process enables
the animated, multilayer multicolor neon border. Reduced-motion mode keeps a
static color border, and backgrounded WebViews receive a low-frequency direct
repaint so WebKit timer throttling does not freeze the breathing effect.

Lifecycle motion is synchronized with the rendered surface. The compact island
waits for its first rendered snapshot before easing in, activity rows are not
rebuilt on an expand or collapse frame, and transition completion rather than a
fixed delay drives the native resize handshake. A background classification
pauses only continuous neon work, not the bounded open, resize, and close
transitions. On macOS the child WKWebView participates in the native window
resize transaction; reduced-motion preferences still make every lifecycle
change immediate.

Control clicks and replies are authorized against the latest sanitized snapshot
and written as bounded, versioned requests to a private sibling queue. Replies
are limited to 1,000 characters / 4 KiB; `Enter` sends and `Shift+Enter` inserts
a newline. During approval, a reply queues a normal follow-up and does not
implicitly approve or deny. On Unix the queue is `0700` and request files are
`0600`. The target TUI revalidates the short-lived, one-shot grant against its
current session, activity, and tool/task context before using the existing
submission, approval, interruption, or child-cancellation path.

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
| Windows | WebView2 | Runtime ships with supported Windows releases; the island stays outside the taskbar and Alt-Tab list, remains non-activating while collapsed, and accepts focus when expanded for direct interaction. |

Release workflows build all CLI targets. Where the helper is not installed,
RemoteUI degrades to the system browser and Agent Island startup is skipped.

## License

MIT — see [LICENSE](LICENSE).
