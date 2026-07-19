pub(super) const ISLAND_LIFECYCLE_SCRIPT: &str = r#"
    function flushPendingActivityRender() {
      if (!pendingActivityRender || !model || closing) return;
      pendingActivityRender = false;
      renderActivities(model);
    }

    function finishResize() {
      clearTimeout(resizeStartTimer);
      resizeStartTimer = 0;
      if (resizeFrame) window.cancelAnimationFrame(resizeFrame);
      resizeFrame = 0;
      clearTimeout(resizeTimer);
      resizeTimer = 0;
      const completion = resizeCompletion;
      resizeCompletion = null;
      root.classList.remove('resizing');
      flushPendingActivityRender();
      if (completion) completion();
    }

    function beginResize(completion) {
      clearTimeout(resizeStartTimer);
      resizeStartTimer = 0;
      if (resizeFrame) window.cancelAnimationFrame(resizeFrame);
      resizeFrame = 0;
      clearTimeout(resizeTimer);
      resizeTimer = 0;
      resizeCompletion = typeof completion === 'function' ? completion : null;
      root.classList.add('resizing');
    }

    function armResizeCompletion() {
      if (reducedMotion.matches) {
        finishResize();
        return;
      }
      resizeTimer = window.setTimeout(finishResize, resizeFallbackMs);
    }

    function requestPresent() {
      if (presentPosted || presentTimer || presentFrame || closing) return;
      const present = () => {
        if (presentPosted || closing) return;
        presentPosted = true;
        clearTimeout(presentTimer);
        presentTimer = 0;
        if (presentFrame) window.cancelAnimationFrame(presentFrame);
        presentFrame = 0;
        post('present');
      };
      // Finish the initial DOM and layout work while the native window is
      // hidden. The timer is required because non-activating WKWebViews may be
      // classified as hidden before their first ordered frame.
      root.getBoundingClientRect();
      presentTimer = window.setTimeout(present, presentFallbackMs);
      presentFrame = window.requestAnimationFrame(() => {
        presentFrame = window.requestAnimationFrame(present);
      });
    }

    function finishOpen() {
      if (!opening && opened) return;
      clearTimeout(openTimer);
      openTimer = 0;
      opening = false;
      opened = true;
      root.classList.remove('booting');
      if (expandAfterOpen) {
        expandAfterOpen = false;
        requestExpand();
      }
    }

    function beginOpen() {
      if (closing || opening || opened) return;
      opening = true;
      const paint = () => {
        if (!opening || closing) return;
        clearTimeout(openTimer);
        openTimer = 0;
        if (openFrame) window.cancelAnimationFrame(openFrame);
        openFrame = 0;
        root.classList.remove('booting');
        if (reducedMotion.matches) {
          finishOpen();
        } else {
          openTimer = window.setTimeout(finishOpen, openFallbackMs);
        }
      };
      if (reducedMotion.matches) {
        paint();
        return;
      }
      // The native window has just been ordered front. Two display frames give
      // WKWebView one committed booting frame before the compositor transition.
      openTimer = window.setTimeout(paint, presentFallbackMs);
      openFrame = window.requestAnimationFrame(() => {
        openFrame = window.requestAnimationFrame(paint);
      });
    }

    function completeClose() {
      if (!closing || closeCompletePosted) return;
      closeCompletePosted = true;
      clearTimeout(closeTimer);
      closeTimer = 0;
      post('close-complete');
    }

    function freezeResizeForClose() {
      if (!root.classList.contains('resizing')) return;
      const bounds = root.getBoundingClientRect();
      const style = window.getComputedStyle(root);
      root.style.width = `${bounds.width}px`;
      root.style.height = `${bounds.height}px`;
      root.style.borderRadius = style.borderRadius;
      root.getBoundingClientRect();
    }

    function beginClose() {
      if (closing) return;
      freezeResizeForClose();
      closing = true;
      closeCompletePosted = false;
      clearTimeout(presentTimer);
      presentTimer = 0;
      if (presentFrame) window.cancelAnimationFrame(presentFrame);
      presentFrame = 0;
      clearTimeout(openTimer);
      openTimer = 0;
      if (openFrame) window.cancelAnimationFrame(openFrame);
      openFrame = 0;
      opening = false;
      opened = false;
      expandAfterOpen = false;
      clearTimeout(resizeStartTimer);
      resizeStartTimer = 0;
      if (resizeFrame) window.cancelAnimationFrame(resizeFrame);
      resizeFrame = 0;
      clearTimeout(resizeTimer);
      resizeTimer = 0;
      resizeCompletion = null;
      pendingActivityRender = false;
      expanded = false;
      expandPending = false;
      collapsePending = false;
      collapseCompletePosted = false;
      attentionExpandQueued = false;
      // Keep the current compact or expanded geometry stable while closing.
      // Combining a full layout collapse with the compositor fade is both more
      // expensive and visually truncates an expanded island.
      root.classList.remove('booting', 'resizing');
      root.classList.add('closing');
      summary.setAttribute('aria-expanded', 'false');
      syncPanelAccess();
      if (reducedMotion.matches) {
        completeClose();
      } else {
        closeTimer = window.setTimeout(completeClose, closeFallbackMs);
      }
    }

    function syncPanelAccess() {
      const accessible = expanded && !closing;
      panel.setAttribute('aria-hidden', accessible ? 'false' : 'true');
      if (accessible) {
        panel.removeAttribute('inert');
      } else {
        panel.setAttribute('inert', '');
      }
    }

    function setExpanded(next) {
      if (closing) return;
      if (next !== true) {
        finishCollapse();
        return;
      }
      collapsePending = false;
      collapseCompletePosted = false;
      attentionExpandQueued = false;
      beginResize(null);
      const start = () => {
        if (!expandPending || closing) return;
        clearTimeout(resizeStartTimer);
        resizeStartTimer = 0;
        if (resizeFrame) window.cancelAnimationFrame(resizeFrame);
        resizeFrame = 0;
        expanded = true;
        expandPending = false;
        root.classList.add('expanded');
        summary.setAttribute('aria-expanded', 'true');
        syncPanelAccess();
        armResizeCompletion();
      };
      if (reducedMotion.matches) {
        start();
        return;
      }
      // Rust has already enlarged the transparent native host. Start the only
      // visible geometry transition after WKWebView has committed those bounds.
      resizeStartTimer = window.setTimeout(start, resizeStartFallbackMs);
      resizeFrame = window.requestAnimationFrame(start);
    }

    function beginCollapse() {
      if (closing || !expanded || collapsePending) return;
      expanded = false;
      collapsePending = true;
      collapseCompletePosted = false;
      attentionExpandQueued = false;
      beginResize(completeCollapse);
      root.classList.remove('expanded');
      summary.setAttribute('aria-expanded', 'false');
      syncPanelAccess();
      armResizeCompletion();
    }

    function finishCollapse() {
      finishResize();
      expanded = false;
      expandPending = false;
      collapsePending = false;
      collapseCompletePosted = false;
      root.classList.remove('expanded');
      summary.setAttribute('aria-expanded', 'false');
      syncPanelAccess();
      if (attentionExpandQueued) {
        attentionExpandQueued = false;
        requestExpand();
      }
    }
"#;
