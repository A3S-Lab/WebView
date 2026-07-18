pub(super) const ISLAND_SCRIPT: &str = r#"
  (() => {
    'use strict';
    const root = document.getElementById('island');
    const summary = document.getElementById('summary');
    const panel = document.getElementById('panel');
    const summaryRobot = document.getElementById('summary-robot');
    const headline = document.getElementById('headline');
    const detail = document.getElementById('detail');
    const compactAttention = document.getElementById('compact-attention');
    const panelSummary = document.getElementById('panel-summary');
    const activities = document.getElementById('activities');
    const degraded = document.getElementById('degraded');
    const turnOff = document.getElementById('turn-off');
    const filterButtons = Array.from(document.querySelectorAll('.filter'));
    const countNodes = {
      all: document.getElementById('count-all'),
      needs_attention: document.getElementById('count-needs-attention'),
      running: document.getElementById('count-running'),
      recent: document.getElementById('count-recent')
    };
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
    const tones = [
      'planning',
      'working',
      'attention',
      'idle',
      'success',
      'danger',
      'cancelled',
      'inferred'
    ];
    const vendors = [
      'a3s',
      'open_ai',
      'anthropic',
      'google',
      'cursor',
      'moonshot',
      'tencent',
      'alibaba',
      'deep_seek',
      'mistral',
      'other'
    ];
    const filters = ['all', 'needs_attention', 'running', 'recent'];
    const seenAttentionKeys = new Set();
    const attentionKeyOrder = [];
    const maxRememberedAttentionKeys = 1024;
    let selectedFilter = 'all';
    let expanded = false;
    let expandPending = false;
    let collapsePending = false;
    let attentionExpandQueued = false;
    let collapseTimer = 0;
    let neonTimer = 0;
    let elapsedTimer = 0;
    let model = null;

    const text = (node, value, fallback = '') => {
      node.textContent = typeof value === 'string' && value.length ? value : fallback;
    };
    const titledText = (node, value, fallback = '') => {
      text(node, value, fallback);
      if (typeof value === 'string' && value.length) {
        node.title = value;
      } else {
        node.removeAttribute('title');
      }
    };
    const post = message => {
      try { window.ipc.postMessage(message); } catch (_) {}
    };
    const toneName = tone => tones.includes(tone) ? tone : 'inferred';
    const vendorName = vendor => vendors.includes(vendor) ? vendor : 'other';
    const finiteCount = value => Number.isFinite(value) && value >= 0
      ? Math.floor(value)
      : 0;
    const plural = (count, noun) => `${count} ${noun}${count === 1 ? '' : 's'}`;
    const completeCollapse = () => {
      clearTimeout(collapseTimer);
      collapseTimer = 0;
      post('collapse-complete');
    };

    function robotNode(vendor, tone) {
      const robot = document.createElement('div');
      robot.className = `robot ${vendorName(vendor)}`;
      const antenna = document.createElement('span');
      antenna.className = 'antenna';
      const leftEar = document.createElement('span');
      leftEar.className = 'ear left';
      const rightEar = document.createElement('span');
      rightEar.className = 'ear right';
      const head = document.createElement('span');
      head.className = 'head';
      const face = document.createElement('span');
      face.className = 'face';
      const leftEye = document.createElement('i');
      leftEye.className = 'eye left';
      const rightEye = document.createElement('i');
      rightEye.className = 'eye right';
      const mouth = document.createElement('i');
      mouth.className = 'mouth';
      const pip = document.createElement('i');
      pip.className = `pip ${toneName(tone)}`;
      face.append(leftEye, rightEye, mouth);
      head.append(face);
      robot.append(antenna, leftEar, rightEar, head, pip);
      return robot;
    }

    function formatDuration(startedAt, finishedAt, now) {
      if (!Number.isFinite(startedAt) || startedAt <= 0) return '—';
      const end = Number.isFinite(finishedAt) && finishedAt >= startedAt ? finishedAt : now;
      let seconds = Math.max(0, Math.floor((end - startedAt) / 1000));
      if (seconds < 60) return `${seconds}s`;
      const minutes = Math.floor(seconds / 60);
      seconds %= 60;
      if (minutes < 60) return `${minutes}m ${String(seconds).padStart(2, '0')}s`;
      const hours = Math.floor(minutes / 60);
      const remainingMinutes = minutes % 60;
      if (hours < 24) return `${hours}h ${String(remainingMinutes).padStart(2, '0')}m`;
      const days = Math.floor(hours / 24);
      return `${days}d ${hours % 24}h`;
    }

    function updateDurations() {
      const now = Date.now();
      document.querySelectorAll('.duration').forEach(node => {
        const started = Number(node.dataset.started);
        const finished = node.dataset.finished ? Number(node.dataset.finished) : NaN;
        node.textContent = formatDuration(started, finished, now);
      });
      document.querySelectorAll('.control').forEach(button => {
        if (button.dataset.pending === 'true') return;
        const expired = Number(button.dataset.expires) < now;
        button.disabled = expired;
        if (expired) button.textContent = 'Expired';
      });
    }

    function stopNeonTimer() {
      if (neonTimer) {
        clearInterval(neonTimer);
        neonTimer = 0;
      }
    }

    function paintHiddenNeon() {
      if (!root.classList.contains('active-work') || reducedMotion.matches) return;
      const phase = (Date.now() % 2800) / 2800;
      const wave = .5 + .5 * Math.sin(phase * Math.PI * 2);
      const position = Math.round(phase * 100);
      root.style.backgroundPosition = `${position}% 50%`;
      root.style.boxShadow =
        `0 8px 28px rgba(0,0,0,.36), 0 0 ${10 + wave * 9}px rgba(77,181,255,${.3 + wave * .35}), 0 0 ${17 + wave * 13}px rgba(177,74,255,${.18 + wave * .28})`;
    }

    function syncNeon() {
      stopNeonTimer();
      root.style.removeProperty('background-position');
      root.style.removeProperty('box-shadow');
      if (!root.classList.contains('active-work') || reducedMotion.matches) return;
      if (document.hidden) {
        paintHiddenNeon();
        neonTimer = window.setInterval(paintHiddenNeon, 180);
      }
    }

    const syncDocumentVisibility = () => {
      document.documentElement.classList.toggle('webview-backgrounded', document.hidden);
      if (document.hidden && collapsePending) completeCollapse();
      syncNeon();
    };

    function normalizedMetrics(data) {
      const source = data && data.metrics && typeof data.metrics === 'object'
        ? data.metrics
        : {};
      return {
        total: finiteCount(source.total),
        needs_attention: finiteCount(source.needs_attention),
        running: finiteCount(source.running),
        recent: finiteCount(source.recent),
        inferred: finiteCount(source.inferred)
      };
    }

    function syncMetrics(metrics) {
      countNodes.all.textContent = String(metrics.total);
      countNodes.needs_attention.textContent = String(metrics.needs_attention);
      countNodes.running.textContent = String(metrics.running);
      countNodes.recent.textContent = String(metrics.recent);
      const summaryParts = [plural(metrics.total, 'agent')];
      if (metrics.inferred > 0) summaryParts.push(`${metrics.inferred} detected`);
      panelSummary.textContent = summaryParts.join(' · ');
      compactAttention.textContent = String(metrics.needs_attention);
      compactAttention.classList.toggle('visible', metrics.needs_attention > 0);
      compactAttention.setAttribute(
        'aria-label',
        plural(metrics.needs_attention, 'agent') + ' need you'
      );
      root.classList.toggle('has-attention', metrics.needs_attention > 0);
    }

    function itemMatches(item, filter) {
      if (filter === 'all') return true;
      return Array.isArray(item.categories) && item.categories.includes(filter);
    }

    function orderedActivities(items) {
      const byId = new Map();
      const children = new Map();
      const positions = new Map(items.map((item, index) => [item, index]));
      items.forEach(item => {
        if (typeof item.id === 'string' && !byId.has(item.id)) byId.set(item.id, item);
      });
      items.forEach(item => {
        if (typeof item.parent_id !== 'string' || !byId.has(item.parent_id)) return;
        const group = children.get(item.parent_id) || [];
        group.push(item);
        children.set(item.parent_id, group);
      });
      const ordered = [];
      const visited = new Set();
      const visit = item => {
        if (visited.has(item)) return;
        visited.add(item);
        ordered.push(item);
        const descendants = children.get(item.id) || [];
        descendants.forEach(visit);
      };
      const priorityCache = new Map();
      const priorityStack = new Set();
      const groupPriority = item => {
        if (priorityCache.has(item)) return priorityCache.get(item);
        if (priorityStack.has(item)) return positions.get(item);
        priorityStack.add(item);
        let priority = positions.get(item);
        (children.get(item.id) || []).forEach(child => {
          priority = Math.min(priority, groupPriority(child));
        });
        priorityStack.delete(item);
        priorityCache.set(item, priority);
        return priority;
      };
      const roots = items
        .filter(item => typeof item.parent_id !== 'string' || !byId.has(item.parent_id))
        .sort((left, right) => groupPriority(left) - groupPriority(right));
      roots.forEach(visit);
      items.forEach(visit);
      return ordered;
    }

    function collectVisibleItems(items, filter) {
      const ordered = orderedActivities(items);
      if (filter === 'all') return ordered.map(item => ({ item, context: false }));
      const byId = new Map(items.map(item => [item.id, item]));
      const matches = new Set(items.filter(item => itemMatches(item, filter)));
      const context = new Set();
      const addAncestors = item => {
        const visited = new Set();
        let parentId = item.parent_id;
        while (typeof parentId === 'string' && !visited.has(parentId)) {
          visited.add(parentId);
          const parent = byId.get(parentId);
          if (!parent) break;
          if (!matches.has(parent)) context.add(parent);
          parentId = parent.parent_id;
        }
      };
      matches.forEach(addAncestors);
      return ordered
        .filter(item => matches.has(item) || context.has(item))
        .map(item => ({ item, context: context.has(item) }));
    }

    function emptyCopy(filter) {
      switch (filter) {
        case 'needs_attention':
          return ['You are all caught up', 'No approval, input, or failed task needs you.'];
        case 'running':
          return ['Nothing is running', 'Planning and active work will appear here.'];
        case 'recent':
          return ['No recent outcomes', 'Completed, failed, and cancelled tasks will appear here.'];
        default:
          return ['No agent activity', 'New work will appear automatically.'];
      }
    }

    function emptyState(filter) {
      const [titleCopy, detailCopy] = emptyCopy(filter);
      const empty = document.createElement('div');
      empty.className = 'empty-state';
      const icon = document.createElement('div');
      icon.className = 'empty-icon';
      icon.textContent = filter === 'needs_attention' ? '✓' : '◇';
      const title = document.createElement('div');
      title.className = 'empty-title';
      text(title, titleCopy);
      const detailNode = document.createElement('div');
      detailNode.className = 'empty-detail';
      text(detailNode, detailCopy);
      empty.append(icon, title, detailNode);
      return empty;
    }

    function childProgressNode(progress) {
      if (!progress || !Number.isFinite(progress.total) || progress.total <= 0) return null;
      const total = Math.max(1, Math.floor(progress.total));
      const settled = Math.min(total, Math.max(0, Math.floor(progress.settled || 0)));
      const wrapper = document.createElement('div');
      wrapper.className = 'child-progress';
      const label = document.createElement('span');
      label.className = 'progress-label';
      label.textContent = `${settled}/${total} settled`;
      const rail = document.createElement('span');
      rail.className = 'progress-rail';
      rail.setAttribute('role', 'progressbar');
      rail.setAttribute('aria-label', 'Child agents settled');
      rail.setAttribute('aria-valuemin', '0');
      rail.setAttribute('aria-valuemax', String(total));
      rail.setAttribute('aria-valuenow', String(settled));
      const fill = document.createElement('i');
      fill.className = 'progress-fill';
      fill.style.width = `${Math.round(settled / total * 100)}%`;
      rail.append(fill);
      wrapper.append(label, rail);
      return wrapper;
    }

    function controlButton(item, control, row) {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = `control ${typeof control.tone === 'string' ? control.tone : 'muted'}`;
      button.dataset.action = control.action;
      button.dataset.expires = String(control.expires_at_ms || 0);
      button.dataset.originalLabel = typeof control.label === 'string' ? control.label : 'Action';
      text(button, control.label, 'Action');
      button.addEventListener('click', event => {
        event.stopPropagation();
        if (button.disabled || Number(control.expires_at_ms) < Date.now()) return;
        row.querySelectorAll('.control').forEach(candidate => {
          candidate.disabled = true;
          candidate.dataset.pending = 'true';
        });
        button.textContent = 'Sending…';
        post(JSON.stringify({
          type: 'control',
          activity_id: item.id,
          action: control.action,
          token: control.token,
          target_instance_id: control.target_instance_id
        }));
      });
      return button;
    }

    function activityNode(item, isContext) {
      const row = document.createElement('div');
      const classes = ['activity'];
      if (item.parent_id) classes.push('child');
      if (isContext) classes.push('context');
      if (Array.isArray(item.categories) && item.categories.includes('needs_attention')) {
        classes.push('needs-attention');
      }
      row.className = classes.join(' ');
      row.dataset.activityId = typeof item.id === 'string' ? item.id : '';
      row.setAttribute('role', 'listitem');
      row.append(robotNode(item.vendor, item.tone));

      const copy = document.createElement('div');
      copy.className = 'copy';
      const agentLine = document.createElement('div');
      agentLine.className = 'agent-line';
      const agent = document.createElement('div');
      agent.className = 'agent';
      titledText(agent, item.agent, 'agent');
      agentLine.append(agent);
      if (isContext) {
        const context = document.createElement('span');
        context.className = 'context-label';
        context.textContent = 'Parent context';
        agentLine.append(context);
      }
      const task = document.createElement('div');
      task.className = 'task';
      const taskText = typeof item.task === 'string' && item.task.length ? item.task : item.status;
      const evidence = typeof item.evidence === 'string' && item.evidence.length
        ? `${taskText} · ${item.evidence}`
        : taskText;
      titledText(task, evidence, item.status);
      copy.append(agentLine, task);
      const progress = childProgressNode(item.child_progress);
      if (progress) copy.append(progress);

      const meta = document.createElement('div');
      meta.className = 'row-meta';
      const statusLine = document.createElement('div');
      statusLine.className = 'status-line';
      const status = document.createElement('span');
      status.className = `status ${toneName(item.tone)}`;
      text(status, item.status, 'Unknown');
      const duration = document.createElement('span');
      duration.className = 'duration';
      duration.dataset.started = Number.isFinite(item.started_at_ms)
        ? String(item.started_at_ms)
        : '';
      duration.dataset.finished = Number.isFinite(item.finished_at_ms)
        ? String(item.finished_at_ms)
        : '';
      statusLine.append(status, duration);
      const workspace = document.createElement('div');
      workspace.className = 'workspace';
      titledText(workspace, item.workspace, item.inferred ? 'process evidence' : 'local task');
      meta.append(statusLine, workspace);

      if (Array.isArray(item.controls) && item.controls.length) {
        const controls = document.createElement('div');
        controls.className = 'controls';
        item.controls.forEach(control => controls.append(controlButton(item, control, row)));
        meta.append(controls);
      }
      row.append(copy, meta);
      return row;
    }

    function renderActivities(data) {
      activities.replaceChildren();
      const visible = collectVisibleItems(data.activities, selectedFilter);
      if (!visible.length) {
        activities.append(emptyState(selectedFilter));
      } else {
        visible.forEach(({ item, context }) => {
          activities.append(activityNode(item, context));
        });
      }
      filterButtons.forEach(button => {
        const active = button.dataset.filter === selectedFilter;
        button.classList.toggle('active', active);
        button.setAttribute('aria-pressed', active ? 'true' : 'false');
      });
      updateDurations();
    }

    function rememberAttentionKey(key) {
      if (seenAttentionKeys.has(key)) return false;
      seenAttentionKeys.add(key);
      attentionKeyOrder.push(key);
      while (attentionKeyOrder.length > maxRememberedAttentionKeys) {
        const removed = attentionKeyOrder.shift();
        seenAttentionKeys.delete(removed);
      }
      return true;
    }

    function requestExpand() {
      if (expanded || expandPending) return;
      if (collapsePending) {
        attentionExpandQueued = true;
        return;
      }
      expandPending = true;
      post('expand');
    }

    function handleAttention(data) {
      if (!Array.isArray(data.attention_keys)) return;
      let hasNewRequest = false;
      data.attention_keys.forEach(key => {
        if (typeof key !== 'string' || key.length === 0 || key.length > 80) return;
        hasNewRequest = rememberAttentionKey(key) || hasNewRequest;
      });
      if (!hasNewRequest) return;
      selectedFilter = 'needs_attention';
      requestExpand();
    }

    function render(data) {
      if (!data || !Array.isArray(data.activities)) return;
      model = data;
      const metrics = normalizedMetrics(data);
      titledText(headline, data.headline, 'No active agents');
      titledText(detail, data.detail, 'Waiting for activity');
      degraded.classList.toggle('visible', data.degraded === true);
      root.classList.toggle('active-work', data.active_work === true);
      summaryRobot.replaceChildren(robotNode(data.vendor, data.tone));
      syncMetrics(metrics);
      handleAttention(data);
      renderActivities(data);
      syncNeon();
    }

    function controlResult(result) {
      if (!result || typeof result.activity_id !== 'string') return;
      const row = Array.from(document.querySelectorAll('.activity'))
        .find(candidate => candidate.dataset.activityId === result.activity_id);
      if (!row) return;
      const buttons = Array.from(row.querySelectorAll('.control'));
      const selected = buttons.find(button => button.dataset.action === result.action);
      if (result.accepted === true) {
        buttons.forEach(button => {
          button.disabled = true;
          button.dataset.pending = 'true';
        });
        if (selected) text(selected, result.message, 'Sent');
        return;
      }
      buttons.forEach(button => {
        button.dataset.pending = 'false';
        const expired = Number(button.dataset.expires) < Date.now();
        button.disabled = expired;
        button.textContent = expired ? 'Expired' : button.dataset.originalLabel;
      });
      if (selected && !selected.disabled) text(selected, result.message, 'Retry');
    }

    function disableResult(accepted) {
      if (accepted === true) return;
      turnOff.disabled = false;
      turnOff.textContent = 'Try again';
    }

    function syncPanelAccess() {
      panel.setAttribute('aria-hidden', expanded ? 'false' : 'true');
      if (expanded) {
        panel.removeAttribute('inert');
      } else {
        panel.setAttribute('inert', '');
      }
    }

    function setExpanded(next) {
      clearTimeout(collapseTimer);
      collapseTimer = 0;
      expanded = next === true;
      expandPending = false;
      collapsePending = false;
      root.classList.toggle('expanded', expanded);
      summary.setAttribute('aria-expanded', expanded ? 'true' : 'false');
      syncPanelAccess();
      if (expanded) attentionExpandQueued = false;
      if (model) renderActivities(model);
      if (!expanded && attentionExpandQueued) {
        attentionExpandQueued = false;
        requestExpand();
      }
    }

    function beginCollapse() {
      if (!expanded || collapsePending) return;
      expanded = false;
      collapsePending = true;
      attentionExpandQueued = false;
      root.classList.remove('expanded');
      summary.setAttribute('aria-expanded', 'false');
      syncPanelAccess();
      if (model) renderActivities(model);
      if (document.hidden) {
        completeCollapse();
      } else {
        collapseTimer = window.setTimeout(completeCollapse, 235);
      }
    }

    function finishCollapse() {
      setExpanded(false);
    }

    summary.addEventListener('click', event => {
      event.stopPropagation();
      if (expandPending || collapsePending) return;
      if (expanded) {
        beginCollapse();
      } else {
        requestExpand();
      }
    });
    filterButtons.forEach(button => {
      button.addEventListener('click', event => {
        event.stopPropagation();
        const requested = button.dataset.filter;
        if (!filters.includes(requested) || requested === selectedFilter) return;
        selectedFilter = requested;
        if (model) renderActivities(model);
      });
    });
    turnOff.addEventListener('click', event => {
      event.stopPropagation();
      if (turnOff.disabled) return;
      turnOff.disabled = true;
      turnOff.textContent = 'Closing…';
      post('disable');
    });

    document.addEventListener('visibilitychange', syncDocumentVisibility);
    if (typeof reducedMotion.addEventListener === 'function') {
      reducedMotion.addEventListener('change', syncNeon);
    }
    elapsedTimer = window.setInterval(updateDurations, 1000);
    syncDocumentVisibility();
    window.a3sIsland = {
      update: render,
      controlResult,
      disableResult,
      setExpanded,
      beginCollapse,
      finishCollapse
    };
    post('ready');
  })();
"#;
