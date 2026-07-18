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
  <main id="island" aria-label="A3S agent activity">
    <div class="surface">
      <section class="summary" id="summary" role="button" aria-label="Show agent activity"
               aria-expanded="false">
        <div id="summary-robot" aria-hidden="true"></div>
        <div class="summary-copy">
          <div class="headline" id="headline">A3S agents</div>
          <div class="detail" id="detail">Connecting…</div>
        </div>
        <div class="summary-tail">
          <span class="compact-attention" id="compact-attention" aria-label="Agents need you"></span>
          <span class="chevron" aria-hidden="true">⌄</span>
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
        script::ISLAND_SCRIPT,
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
    fn hidden_webview_directly_paints_neon_and_completes_resize() {
        let html = html();
        assert!(html.contains("webview-backgrounded"));
        assert!(html.contains("if (document.hidden && collapsePending)"));
        assert!(html.contains("window.setInterval(paintHiddenNeon, 180)"));
        assert!(html.contains("root.style.boxShadow"));
        assert!(html.contains("addEventListener('visibilitychange'"));
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
