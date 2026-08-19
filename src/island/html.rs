#[path = "html/a3s_ui.rs"]
mod a3s_ui;
#[path = "html/fab_settings.rs"]
mod fab_settings;
#[path = "html/fab_style.rs"]
mod fab_style;
#[path = "html/lifecycle.rs"]
mod lifecycle;
#[path = "html/script.rs"]
mod script;
#[path = "html/style.rs"]
mod style;

use super::window::IslandPresentation;

const DOCUMENT_START: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1,user-scalable=no">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'">
  <style>
"#;

const DOCUMENT_BODY_START: &str = r#"
  </style>
</head>
<body>
"#;

const DOCUMENT_MAIN_ISLAND: &str = r#"  <main id="island" class="booting" aria-label="A3S agent activity">
"#;

const DOCUMENT_MAIN_FAB: &str = r#"  <main id="island" class="booting fab-mode" aria-label="Coding Reviewer suggestions">
"#;

const DOCUMENT_BODY: &str = r##"
    <div class="surface">
      <section class="summary" id="summary" role="button" aria-label="Show agent activity"
               aria-expanded="false" tabindex="0">
        <div class="fab-mark" aria-hidden="true">
          <svg viewBox="0 0 28 28" focusable="false" aria-hidden="true">
            <path d="M7.5 8.5h5l3 5-3 5h-5l-3-5 3-5Z"></path>
            <path d="M15.5 6.5h5l3 5-3 5h-2.5"></path>
            <circle cx="9.5" cy="13.5" r="1.35"></circle>
            <circle cx="18.5" cy="11.5" r="1.35"></circle>
          </svg>
        </div>
        <span class="fab-badge" id="fab-badge" aria-label="No new suggestions"></span>
        <div id="summary-robot" aria-hidden="true"></div>
        <div class="summary-copy">
          <div class="headline" id="headline">A3S agents</div>
          <div class="summary-context">
            <span class="compact-agent" id="compact-agent">Agent</span>
            <span class="context-separator" aria-hidden="true">·</span>
            <span class="detail" id="detail">Connecting…</span>
          </div>
        </div>
        <div class="summary-tail">
          <div class="compact-primary">
            <span class="compact-status inferred" id="compact-status">Connecting</span>
            <span class="compact-duration duration" id="compact-duration">—</span>
          </div>
          <div class="compact-overview">
            <span class="compact-stats" id="compact-stats"
                  aria-label="No agent metrics"></span>
            <span class="chevron" aria-hidden="true">⌄</span>
          </div>
        </div>
      </section>
      <button class="drag-handle" id="drag-handle" type="button"
              aria-label="Move Agent Island" title="Drag to move Agent Island">
        <span aria-hidden="true"></span>
      </button>
      <section class="panel agent-panel" id="panel" aria-label="Agent activity details"
               aria-hidden="true" inert>
        <div class="rule"></div>
        <header class="panel-title">
          <div class="panel-copy">
            <strong>Agent activity</strong>
            <span id="panel-summary">Connecting</span>
          </div>
          <div class="panel-actions">
            <span class="badge" id="degraded">Partial data</span>
            <button class="island-power" id="turn-off" type="button"
                    aria-label="Turn off Agent Island" title="Turn off Agent Island">Turn off</button>
          </div>
        </header>
        <nav class="filters" aria-label="Filter agent activity">
          <button class="filter active" type="button" data-filter="all" aria-pressed="true">
            <span>All</span><b id="count-all">0</b>
          </button>
          <button class="filter attention-filter" type="button" data-filter="needs_attention"
                  aria-pressed="false">
            <span>Needs you</span><b id="count-needs-attention">0</b>
          </button>
          <button class="filter" type="button" data-filter="running" aria-pressed="false">
            <span>Running</span><b id="count-running">0</b>
          </button>
          <button class="filter" type="button" data-filter="recent" aria-pressed="false">
            <span>Recent</span><b id="count-recent">0</b>
          </button>
        </nav>
        <div id="activities" role="list"></div>
      </section>
      <section class="suggestion-panel" id="suggestion-panel"
               aria-label="Coding Reviewer suggestions" aria-hidden="true" inert>
        <header class="suggestion-header workspace-header">
          <div class="suggestion-heading">
            <strong>Coding Reviewer</strong>
            <span id="suggestion-summary">Waiting for sessions</span>
          </div>
          <div class="suggestion-header-actions">
            <span class="badge" id="surface-health" data-variant="secondary" hidden>Degraded</span>
            <button class="btn global-suggestion-toggle" id="global-suggestion-toggle"
                    type="button" role="switch" aria-checked="true">
              <span class="toggle-track" aria-hidden="true"><i></i></span>
              <span id="global-suggestion-label">Suggestions on</span>
            </button>
            <button class="btn island-power" id="hide-fab" type="button" data-variant="ghost"
                    aria-label="Hide Coding Reviewer button"
                    title="Hide Coding Reviewer button">Hide FAB</button>
          </div>
        </header>
        <div class="reviewer-layout settings-layout" style="--settings-navigation-size:8.75rem">
          <aside>
            <nav aria-label="Coding Reviewer sections">
              <ul>
                <li><a href="#reviewer-suggestions" data-reviewer-view="suggestions"
                       aria-current="page">Suggestions <span class="badge" id="nav-unread" hidden>0</span></a></li>
                <li><a href="#reviewer-channels" data-reviewer-view="channels">IM channels</a></li>
                <li><a href="#reviewer-model" data-reviewer-view="model">Reviewer model</a></li>
              </ul>
            </nav>
          </aside>
          <main class="reviewer-views">
            <section class="reviewer-view suggestion-workbench" id="reviewer-suggestions"
                     data-reviewer-panel="suggestions" aria-label="Programming suggestions">
              <nav class="suggestion-sessions" id="suggestion-sessions"
                   aria-label="Codex sessions"></nav>
              <section class="suggestion-detail" id="suggestion-detail" aria-live="polite">
                <div class="suggestion-empty" id="suggestion-empty">
                  <svg viewBox="0 0 32 32" focusable="false" aria-hidden="true">
                    <path d="M8 10.5h7l4 6-4 6H8l-4-6 4-6Z"></path>
                    <path d="M19 8h6l3 4.5-3 4.5h-3"></path>
                  </svg>
                  <strong>Select a Codex session</strong>
                  <span>Suggestions stay quiet until there is actionable evidence.</span>
                </div>
                <section class="suggestion-editor approval-request" id="suggestion-editor"
                         data-state="pending" aria-labelledby="suggestion-title" hidden>
                  <header class="suggestion-context">
                    <div>
                      <h2 id="suggestion-title">Programming suggestion</h2>
                      <p id="suggestion-meta">Managed Codex session</p>
                    </div>
                    <span class="badge delivery-state" id="delivery-state"
                          data-variant="secondary">Approval required</span>
                  </header>
                  <section>
                    <p class="suggestion-reason" id="suggestion-reason"></p>
                    <label class="field suggestion-draft-label" for="suggestion-draft">
                      <span>Edit before sending</span>
                      <textarea class="input suggestion-draft" id="suggestion-draft" rows="7"
                                maxlength="1000" spellcheck="true"></textarea>
                    </label>
                  </section>
                  <footer class="suggestion-footer">
                    <span class="suggestion-result" id="suggestion-result" role="status"></span>
                    <div class="suggestion-actions">
                      <button class="btn suggestion-copy" id="suggestion-copy" type="button"
                              data-size="sm" data-variant="ghost">Copy</button>
                      <button class="btn suggestion-dismiss" id="suggestion-dismiss"
                              type="button" data-size="sm" data-variant="outline">Dismiss</button>
                      <button class="btn suggestion-send" id="suggestion-send"
                              type="button" data-size="sm">Send to Codex</button>
                    </div>
                  </footer>
                </section>
              </section>
            </section>

            <section class="reviewer-view settings-view" id="reviewer-channels"
                     data-reviewer-panel="channels" aria-labelledby="channels-title" hidden>
              <header class="settings-view-header">
                <div><h2 id="channels-title">IM channels</h2><p>Connect chat channels to query and manage every local Codex session.</p></div>
                <span class="badge" id="channel-summary" data-variant="secondary">Not configured</span>
              </header>
              <div class="channel-list" id="channel-list" aria-live="polite"></div>
            </section>

            <section class="reviewer-view settings-view" id="reviewer-model"
                     data-reviewer-panel="model" aria-labelledby="model-title" hidden>
              <header class="settings-view-header">
                <div><h2 id="model-title">Reviewer model</h2><p>Configure proposal-only cognition and the evidence it may inspect.</p></div>
                <span class="badge" id="restart-required" data-variant="secondary" hidden>Restart required</span>
              </header>
              <form class="card reviewer-form" id="llm-settings-form" data-size="sm" novalidate>
                <header><h3>Model connection</h3><p>Non-secret settings are revisioned in A3S ORM.</p></header>
                <section class="form-grid">
                  <label class="field switch-field" data-orientation="horizontal">
                    <input class="input" id="llm-enabled" type="checkbox" role="switch">
                    <span><strong>Enable cognitive review</strong><small>Deterministic checks remain active when disabled.</small></span>
                  </label>
                  <label class="field"><span>Provider</span><input class="input" id="llm-provider" maxlength="64" autocomplete="off" placeholder="openai"></label>
                  <label class="field"><span>Model</span><input class="input" id="llm-model" maxlength="256" autocomplete="off" placeholder="gpt-5"></label>
                  <label class="field"><span>Base URL</span><input class="input" id="llm-base-url" type="url" maxlength="2048" autocomplete="off" placeholder="https://api.openai.com/v1"></label>
                  <label class="field"><span>Keychain reference</span><input class="input" id="llm-api-key-ref" maxlength="256" autocomplete="off" placeholder="reviewer/openai"></label>
                  <label class="field"><span>Evidence detail</span><select class="input" id="llm-evidence"><option value="metadata">Metadata only</option><option value="redacted_error">Metadata and redacted errors</option></select></label>
                </section>
                <fieldset class="disclosure-fields">
                  <legend>Project context disclosure</legend>
                  <label><input class="input" id="share-habits" type="checkbox"><span><strong>Coding habits</strong><small>Share learned user preferences.</small></span></label>
                  <label><input class="input" id="share-knowledge" type="checkbox"><span><strong>Knowledge graph</strong><small>Share bounded project facts.</small></span></label>
                  <label><input class="input" id="share-transitions" type="checkbox"><span><strong>Recent transitions</strong><small>Requires knowledge graph disclosure.</small></span></label>
                </fieldset>
                <footer><span id="llm-settings-result" role="status"></span><button class="btn" id="save-llm-settings" type="submit" data-size="sm">Save settings</button></footer>
              </form>
              <form class="card reviewer-form secret-form" id="llm-secret-form" data-size="sm" novalidate>
                <header><h3>API key</h3><p>The value travels through a private same-user socket directly to Keychain.</p></header>
                <section><label class="field" for="llm-api-key"><span>Replacement key</span><input class="input" id="llm-api-key" type="password" maxlength="16384" autocomplete="new-password" spellcheck="false"></label></section>
                <footer><span id="llm-secret-result" role="status"></span><button class="btn" id="save-llm-secret" type="submit" data-size="sm">Replace API key</button></footer>
              </form>
            </section>
          </main>
        </div>
      </section>
    </div>
  </main>
  <script>
"##;

const DOCUMENT_END: &str = r#"
  </script>
</body>
</html>"#;

pub(crate) fn island_html(presentation: IslandPresentation) -> String {
    [
        DOCUMENT_START,
        a3s_ui::METADATA,
        a3s_ui::CSS,
        style::ISLAND_STYLE,
        fab_style::FAB_STYLE,
        DOCUMENT_BODY_START,
        if presentation.is_fab() {
            DOCUMENT_MAIN_FAB
        } else {
            DOCUMENT_MAIN_ISLAND
        },
        DOCUMENT_BODY,
        script::ISLAND_SCRIPT_START,
        lifecycle::ISLAND_LIFECYCLE_SCRIPT,
        fab_settings::FAB_SETTINGS_SCRIPT,
        script::ISLAND_SCRIPT_END,
        DOCUMENT_END,
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html() -> String {
        island_html(IslandPresentation::Island)
    }

    fn fab_html() -> String {
        island_html(IslandPresentation::Fab)
    }

    #[test]
    fn renderer_uses_text_nodes_and_keeps_the_bounded_snapshot_scrollable() {
        let html = html();
        assert!(html.contains("node.textContent"));
        assert!(html.contains("node.title = value"));
        assert!(html.contains("orderedActivities"));
        assert!(html.contains("overflow-y: auto"));
        assert!(!html.contains("innerHTML"));
        assert!(!html.contains("slice(0"));
    }

    #[test]
    fn resize_handshake_messages_are_present() {
        let html = html();
        assert!(html.contains("post('expand')"));
        assert!(html.contains("post('collapse-complete')"));
        assert!(html.contains("collapsePending"));
        assert!(html.contains("finishCollapse"));
        assert!(html.contains("window.a3sIsland"));
        assert!(html.contains("syncPanelAccess"));
        assert!(html.contains("aria-hidden=\"true\" inert"));
        assert!(html.contains("post('drag-window')"));
    }

    #[test]
    fn lifecycle_motion_is_paint_ready_event_driven_and_defers_heavy_rows() {
        let html = html();
        assert!(html.contains("class=\"booting\""));
        assert!(html.contains("#island.booting"));
        assert!(html.contains("#island.opening"));
        assert!(html.contains("#island.closing"));
        assert!(html.contains("requestAnimationFrame"));
        assert!(html.contains("transitionend"));
        assert!(html.contains("post('present')"));
        assert!(html.contains("beginOpen"));
        assert!(html.contains("expandAfterOpen"));
        assert!(html.contains("pendingActivityRender"));
        assert!(html.contains("width: 560px;\n      height: 291px;"));
        assert!(html.contains("post('close-complete')"));
        assert!(html.contains("beginClose"));
        assert!(html.contains("freezeResizeForClose"));
        assert!(html.contains("closing && event.propertyName === 'transform'"));
        assert!(!html.contains("window.setTimeout(completeCollapse, 235)"));

        let set_expanded = html
            .split_once("function setExpanded")
            .and_then(|(_, tail)| tail.split_once("function beginCollapse"))
            .map(|(body, _)| body)
            .expect("setExpanded function");
        assert!(!set_expanded.contains("renderActivities("));

        let begin_collapse = html
            .split_once("function beginCollapse")
            .and_then(|(_, tail)| tail.split_once("function finishCollapse"))
            .map(|(body, _)| body)
            .expect("beginCollapse function");
        assert!(!begin_collapse.contains("renderActivities("));

        let begin_close = html
            .split_once("function beginClose")
            .and_then(|(_, tail)| tail.split_once("function syncPanelAccess"))
            .map(|(body, _)| body)
            .expect("beginClose function");
        assert!(!begin_close.contains("classList.remove('expanded')"));
    }

    #[test]
    fn lifecycle_motion_prepares_compositor_before_native_resize() {
        let html = html();
        let request_expand = html
            .split_once("function requestExpand")
            .and_then(|(_, tail)| tail.split_once("function handleAttention"))
            .map(|(body, _)| body)
            .expect("requestExpand function");
        let prepare = request_expand
            .find("beginResize(null)")
            .expect("resize preparation");
        let native_handshake = request_expand
            .find("post('expand')")
            .expect("expand handshake");
        assert!(
            prepare < native_handshake,
            "the expensive effects must pause before the native host grows"
        );

        assert!(html.contains("#island.expanded:not(.resizing) .panel"));
        assert!(html.contains("!root.classList.contains('resizing')"));
        assert!(html.contains("contain: layout paint style;"));
        assert!(html.contains("#island.opening.active-work"));
        assert!(html.contains("#island.closing.active-work"));
        assert!(html.contains("#island.resizing::after"));
        assert!(html.contains("surface.style.borderRadius"));
        assert!(!html.contains("filter: blur(8px) saturate(1.2);"));
        assert!(!html.contains("filter: blur(11px) saturate(1.42);"));
    }

    #[test]
    fn backgrounded_webview_keeps_lifecycle_motion_and_directly_paints_neon() {
        let html = html();
        assert!(html.contains("webview-backgrounded"));
        assert!(html.contains("html.webview-backgrounded #island.active-work"));
        assert!(!html.contains("html.webview-backgrounded .panel"));
        assert!(!html.contains("html.webview-backgrounded .chevron"));
        assert!(!html.contains("document.hidden && collapsePending"));
        assert!(html.contains("resizeFallbackMs"));
        assert!(html.contains("window.setInterval(paintHiddenNeon, 180)"));
        assert!(html.contains("root.style.boxShadow"));
        assert!(html.contains("addEventListener('visibilitychange'"));
    }

    #[test]
    fn glow_has_native_bleed_space_and_only_the_inner_surface_clips() {
        let html = html();
        let island_rule = html
            .split_once("#island {")
            .and_then(|(_, tail)| tail.split_once('}'))
            .map(|(body, _)| body)
            .expect("base island rule");
        assert!(html.contains("top: 32px"));
        assert!(html.contains("overflow: visible"));
        assert!(island_rule.contains("contain: layout;"));
        assert!(!island_rule.contains("contain: layout paint"));
        assert!(html.contains(".surface"));
        assert!(html.contains("overflow: hidden"));
        assert!(html.contains("inset: -30px -46px"));
        assert!(html.contains("-webkit-mask-image: radial-gradient"));
        assert!(html.contains("transparent 78%"));
        assert!(!html.contains("0 0 68px"));
        assert!(!html.contains("0 0 66px"));
    }

    #[test]
    fn collapsed_summary_exposes_prioritized_context_progress_and_counts() {
        let html = html();
        for id in [
            "compact-agent",
            "compact-status",
            "compact-duration",
            "compact-stats",
        ] {
            assert!(html.contains(&format!("id=\"{id}\"")));
        }
        assert!(html.contains("width: 480px"));
        assert!(html.contains("height: 72px"));
        assert!(html.contains("data.primary_agent"));
        assert!(html.contains("data.primary_workspace"));
        assert!(html.contains("data.primary_reason"));
        assert!(html.contains("data.primary_child_progress"));
        assert!(html.contains("data.status"));
        assert!(html.contains("data.primary_started_at_ms"));
        assert!(html.contains("data.primary_finished_at_ms"));
        assert!(html.contains("function compactMetricParts"));
        assert!(html.contains("`${progress.settled}/${progress.total} settled`"));
        assert!(html.contains("`${metrics.recent} recent`"));
        assert!(html.contains("`${metrics.total} total`"));
        assert!(html.contains("compactStats.scrollWidth > compactStats.clientWidth"));
        assert!(html.contains("visibleParts.pop()"));

        let metric_parts = html
            .split_once("function compactMetricParts")
            .and_then(|(_, tail)| tail.split_once("function syncCompactMetrics"))
            .map(|(body, _)| body)
            .expect("compact metric hierarchy");
        let partial = metric_parts
            .find("Partial data")
            .expect("partial-data priority");
        let attention = metric_parts.find("needs you").expect("attention priority");
        let running = metric_parts
            .find("`${metrics.running} running`")
            .expect("running priority");
        let progress = metric_parts
            .find("`${progress.settled}/${progress.total} settled`")
            .expect("progress priority");
        let recent = metric_parts
            .find("`${metrics.recent} recent`")
            .expect("recent fallback");
        let total = metric_parts
            .find("`${metrics.total} total`")
            .expect("total fallback");
        assert!(partial < attention);
        assert!(attention < running);
        assert!(running < progress);
        assert!(progress < recent);
        assert!(recent < total);
    }

    #[test]
    fn inline_controls_are_json_ipc_and_do_not_toggle_the_island() {
        let html = html();
        assert!(html.contains("event.stopPropagation()"));
        assert!(html.contains("type: 'control'"));
        assert!(html.contains("target_instance_id"));
        assert!(html.contains("controlResult"));
        assert!(html.contains("summary.addEventListener('click'"));
        assert!(!html.contains("root.addEventListener('click'"));
    }

    #[test]
    fn macbook_notch_profile_avoids_the_hardware_and_fuses_to_the_top_edge() {
        let html = html();

        assert!(html.contains("setScreenProfile"));
        assert!(html.contains("--notch-left"));
        assert!(html.contains("--notch-width"));
        assert!(html.contains("#island.notched .summary"));
        assert!(html.contains("border-radius: 0 0 var(--island-radius)"));
        assert!(html.contains("root.classList.toggle('notched'"));
        assert!(html.contains("root.classList.add('screen-ready')"));
    }

    #[test]
    fn dedicated_drag_handle_does_not_toggle_summary_actions() {
        let html = html();

        assert!(html.contains("id=\"drag-handle\""));
        assert!(html.contains("aria-label=\"Move Agent Island\""));
        assert!(html.contains("dragHandle.addEventListener('mousedown'"));
        assert!(html.contains("dragHandle.addEventListener('touchstart'"));
        assert!(html.contains("event.preventDefault()"));
        assert!(html.contains("event.stopPropagation()"));
    }

    #[test]
    fn hitl_rows_explain_the_request_and_support_a_real_text_reply() {
        let html = html();
        assert!(html.contains("attention-reason"));
        assert!(html.contains("item.reason"));
        assert!(html.contains("reply-composer"));
        assert!(html.contains("reply-input"));
        assert!(html.contains("action: 'reply'"));
        assert!(html.contains("message: value"));
        assert!(html.contains("event.shiftKey"));
        assert_eq!(html.matches("markRowPending(row);").count(), 2);
        assert!(html.contains("restoreRowActions(row);"));
        assert!(html.contains("min-width: 56px"));
        assert!(html.contains("height: 30px"));
    }

    #[test]
    fn expanded_view_exposes_a_persistent_turn_off_action() {
        let html = html();
        assert!(html.contains("aria-label=\"Turn off Agent Island\""));
        assert!(html.contains("post('disable')"));
        assert!(html.contains("disableResult"));
        assert!(html.contains("turnOff.disabled = true"));
    }

    #[test]
    fn attention_filters_counts_and_empty_states_are_explicit() {
        let html = html();
        for filter in ["all", "needs_attention", "running", "recent"] {
            assert!(html.contains(&format!("data-filter=\"{filter}\"")));
        }
        assert!(html.contains("seenAttentionKeys"));
        assert!(html.contains("attentionExpandQueued"));
        assert!(html.contains("data.attention_keys.forEach"));
        assert!(!html.contains(".some(rememberAttentionKey)"));
        assert!(html.contains("selectedFilter = 'needs_attention'"));
        assert!(html.contains("emptyCopy"));
    }

    #[test]
    fn filtered_children_keep_parent_context_and_progress() {
        let html = html();
        assert!(html.contains("collectVisibleItems"));
        assert!(html.contains("addAncestors"));
        assert!(html.contains("groupPriority"));
        assert!(html.contains("Parent context"));
        assert!(html.contains("item.child_progress"));
        assert!(html.contains("settled"));
    }

    #[test]
    fn robots_statuses_and_terminal_durations_are_explicit() {
        let html = html();
        assert!(html.contains("Original robot geometry"));
        assert!(html.contains("finished_at_ms"));
        assert!(html.contains("formatDuration"));
        assert!(html.contains("item.status"));
        assert!(html.contains("prefers-reduced-motion"));
    }

    #[test]
    fn css_avoids_newer_color_functions_for_embedded_webviews() {
        assert!(!style::ISLAND_STYLE.contains("color-mix("));
        assert!(!style::ISLAND_STYLE.contains("rgba(77,181,255,var("));
        assert!(!style::ISLAND_STYLE.contains("calc(var(--neon-alpha"));
    }

    #[test]
    fn fab_embeds_the_pinned_a3s_ui_operational_components_offline() {
        let html = fab_html();
        assert!(html.contains("@a3s-lab/ui 0.3.0"));
        assert!(html.contains("sha256:25803bd741f763a5b7ed5cb4c753cad0"));
        assert!(a3s_ui::CSS.contains(".settings-layout"));
        assert!(a3s_ui::CSS.contains(".approval-request"));
        assert!(a3s_ui::CSS.contains(".btn"));
    }

    #[test]
    fn fab_scopes_a3s_ui_theme_badges_and_scroll_layout() {
        let html = fab_html();

        assert!(html.contains("#island.fab-mode {"));
        assert!(html.contains("color-scheme: dark"));
        assert!(html.contains("--a3s-panel: var(--card)"));
        assert!(html.contains("#degraded { display: none"));
        assert!(!html.contains(".badge { display: none"));
        assert!(html.contains("#island.fab-mode .reviewer-layout > main > .reviewer-view"));
        assert!(html.contains(
            "#island.fab-mode .reviewer-layout > main > .settings-view { display: block; }"
        ));
    }

    #[test]
    fn fab_is_a_bounded_draggable_suggestion_surface_with_an_unread_badge() {
        let html = fab_html();

        assert!(html.contains("class=\"booting fab-mode\""));
        assert!(html.contains("id=\"fab-badge\""));
        assert!(html.contains("id=\"suggestion-panel\""));
        assert!(html.contains("id=\"global-suggestion-toggle\""));
        assert!(html.contains("id=\"suggestion-sessions\""));
        assert!(html.contains("id=\"suggestion-draft\""));
        assert!(html.contains("id=\"suggestion-send\""));
        assert!(html.contains("id=\"hide-fab\""));
        assert!(html.contains("--collapsed-width: 56px"));
        assert!(html.contains("--expanded-width: 720px"));
        assert!(html.contains("fabBadge.classList.toggle('visible'"));
        assert!(html.contains("if (isFab) return;"));
        assert!(html.contains("post('drag-window')"));
    }

    #[test]
    fn fab_edits_and_submits_the_complete_exact_draft_but_keeps_observed_sessions_copy_only() {
        let html = fab_html();

        assert!(html.contains("controlFor(activeSuggestion, 'approve_suggestion')"));
        assert!(html.contains("controlFor(activeSuggestion, 'dismiss_suggestion')"));
        assert!(html.contains("message: value"));
        assert!(html.contains("payload.message = message"));
        assert!(html.contains("new TextEncoder().encode(value).length > 4096"));
        assert!(html.contains("event.metaKey && !event.ctrlKey"));
        assert!(html.contains("navigator.clipboard.writeText(value)"));
        assert!(html.contains("document.execCommand('copy')"));
        assert!(html.contains("text(deliveryState, approve ? 'Approval required' : 'Copy only')"));
        assert!(html.contains("suggestionSend.dataset.available = canSend ? 'true' : 'false'"));
    }

    #[test]
    fn fab_lifecycle_exposes_only_the_active_panel_and_defers_the_correct_renderer() {
        let html = fab_html();

        assert!(html.contains("const activePanel = isFab ? suggestionPanel : panel"));
        assert!(html.contains("const inactivePanel = isFab ? panel : suggestionPanel"));
        assert!(html.contains("renderSuggestionSurface(model)"));
        assert!(html.contains("globalSuggestionToggle.addEventListener('click'"));
        assert!(html.contains("suggestionDraft.addEventListener('input'"));
        assert!(html.contains("suggestionSend.addEventListener('click'"));
        assert!(html.contains("hideFab.addEventListener('click'"));
        assert!(html.contains("syncSuggestionControlAvailability()"));
    }
}
