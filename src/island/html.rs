#[path = "html/lifecycle.rs"]
mod lifecycle;
#[path = "html/script.rs"]
mod script;
#[path = "html/style.rs"]
mod style;

const DOCUMENT_START: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1,user-scalable=no">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'">
  <style>
"#;

const DOCUMENT_BODY: &str = r#"
  </style>
</head>
<body>
  <main id="island" class="booting" aria-label="A3S agent activity">
    <div class="surface">
      <section class="summary" id="summary" role="button" aria-label="Show agent activity"
               aria-expanded="false">
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
            <span class="compact-running" id="compact-running">0 running</span>
            <span class="metric-separator" aria-hidden="true">·</span>
            <span class="compact-total" id="compact-total">0 total</span>
            <span class="compact-attention" id="compact-attention"
                  aria-label="Agents need you"></span>
            <span class="chevron" aria-hidden="true">⌄</span>
          </div>
        </div>
      </section>
      <section class="panel" id="panel" aria-label="Agent activity details"
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
    </div>
  </main>
  <script>
"#;

const DOCUMENT_END: &str = r#"
  </script>
</body>
</html>"#;

pub(crate) fn island_html() -> String {
    [
        DOCUMENT_START,
        style::ISLAND_STYLE,
        DOCUMENT_BODY,
        script::ISLAND_SCRIPT_START,
        lifecycle::ISLAND_LIFECYCLE_SCRIPT,
        script::ISLAND_SCRIPT_END,
        DOCUMENT_END,
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html() -> String {
        island_html()
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
        assert!(html.contains("width: 560px;\n      height: 303px;"));
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
    fn collapsed_summary_exposes_primary_state_time_and_agent_counts() {
        let html = html();
        for id in [
            "compact-agent",
            "compact-status",
            "compact-duration",
            "compact-running",
            "compact-total",
            "compact-attention",
        ] {
            assert!(html.contains(&format!("id=\"{id}\"")));
        }
        assert!(html.contains("width: 392px"));
        assert!(html.contains("height: 60px"));
        assert!(html.contains("data.primary_agent"));
        assert!(html.contains("data.status"));
        assert!(html.contains("data.primary_started_at_ms"));
        assert!(html.contains("data.primary_finished_at_ms"));
        assert!(html.contains("`${metrics.running} running`"));
        assert!(html.contains("`${metrics.total} total`"));
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
        let html = html();
        assert!(!html.contains("color-mix("));
        assert!(!html.contains("rgba(77,181,255,var("));
        assert!(!html.contains("calc(var(--neon-alpha"));
    }
}
