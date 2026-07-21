pub(super) const ISLAND_SCRIPT: &str = r#"
  (() => {
    'use strict';
    const root = document.getElementById('island');
    const summary = document.getElementById('summary');
    const panel = document.getElementById('panel');
    const summaryRobot = document.getElementById('summary-robot');
    const headline = document.getElementById('headline');
    const detail = document.getElementById('detail');
    const compactAgent = document.getElementById('compact-agent');
    const compactStatus = document.getElementById('compact-status');
    const compactDuration = document.getElementById('compact-duration');
    const compactRunning = document.getElementById('compact-running');
    const compactTotal = document.getElementById('compact-total');
    const compactAttention = document.getElementById('compact-attention');
    const panelSummary = document.getElementById('panel-summary');
    const activities = document.getElementById('activities');
    const degraded = document.getElementById('degraded');
    const turnOff = document.getElementById('turn-off');
    const dragHandle = document.getElementById('drag-handle');
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
    const replyDrafts = new Map();
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

    function setScreenProfile(profile) {
      const dimension = (value, fallback, minimum, maximum) =>
        Number.isFinite(value) ? Math.min(maximum, Math.max(minimum, value)) : fallback;
      const collapsedWidth = dimension(profile && profile.collapsedWidth, 392, 240, 1400);
      const expandedWidth = dimension(
        profile && profile.expandedWidth,
        Math.max(560, collapsedWidth),
        collapsedWidth,
        1400
      );
      const notchLeft = dimension(profile && profile.notchLeft, 0, 0, collapsedWidth);
      const notchWidth = dimension(
        profile && profile.notchWidth,
        0,
        0,
        Math.max(0, collapsedWidth - notchLeft)
      );
      const notchHeight = dimension(profile && profile.notchHeight, 0, 0, 120);
      root.style.setProperty('--collapsed-width', `${collapsedWidth}px`);
      root.style.setProperty('--expanded-width', `${expandedWidth}px`);
      root.style.setProperty('--notch-left', `${notchLeft}px`);
      root.style.setProperty('--notch-width', `${notchWidth}px`);
      root.style.setProperty('--notch-height', `${notchHeight}px`);
      root.classList.toggle('notched', profile && profile.notched === true);
      root.classList.add('screen-ready');
    }

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
        if (!node.dataset.started) {
          if (!node.textContent) node.textContent = '—';
          return;
        }
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
      document.querySelectorAll('.reply-composer').forEach(composer => {
        if (composer.dataset.pending === 'true') return;
        const expired = Number(composer.dataset.expires) < now;
        const input = composer.querySelector('.reply-input');
        const button = composer.querySelector('.reply-send');
        if (!input || !button) return;
        input.disabled = expired;
        button.disabled = expired || input.value.trim().length === 0;
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
        `0 6px 18px rgba(0,0,0,.36), 0 0 ${7 + wave * 4}px rgba(77,181,255,${.38 + wave * .38}), 0 0 ${14 + wave * 4}px rgba(88,101,255,${.24 + wave * .26}), 0 0 ${20 + wave * 4}px rgba(224,73,255,${.12 + wave * .13})`;
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
      compactRunning.textContent = `${metrics.running} running`;
      compactRunning.setAttribute('aria-label', plural(metrics.running, 'agent') + ' running');
      compactTotal.textContent = `${metrics.total} total`;
      compactTotal.setAttribute('aria-label', plural(metrics.total, 'agent') + ' total');
      const summaryParts = [plural(metrics.total, 'agent')];
      if (metrics.inferred > 0) summaryParts.push(`${metrics.inferred} detected`);
      panelSummary.textContent = summaryParts.join(' · ');
      compactAttention.textContent = `! ${metrics.needs_attention}`;
      compactAttention.classList.toggle('visible', metrics.needs_attention > 0);
      compactAttention.setAttribute(
        'aria-label',
        plural(metrics.needs_attention, 'agent') + ' need you'
      );
      root.classList.toggle('has-attention', metrics.needs_attention > 0);
    }

    function syncCompactPrimary(data) {
      const tone = toneName(data.tone);
      titledText(compactAgent, data.primary_agent, 'Agent');
      titledText(compactStatus, data.status, 'Idle');
      compactStatus.className = `compact-status ${tone}`;

      if (Number.isFinite(data.primary_started_at_ms) && data.primary_started_at_ms > 0) {
        compactDuration.dataset.started = String(data.primary_started_at_ms);
        if (
          Number.isFinite(data.primary_finished_at_ms)
          && data.primary_finished_at_ms >= data.primary_started_at_ms
        ) {
          compactDuration.dataset.finished = String(data.primary_finished_at_ms);
        } else {
          delete compactDuration.dataset.finished;
        }
        compactDuration.textContent = formatDuration(
          data.primary_started_at_ms,
          data.primary_finished_at_ms,
          Date.now()
        );
      } else {
        delete compactDuration.dataset.started;
        delete compactDuration.dataset.finished;
        compactDuration.textContent = data.active_work === true ? 'Live' : '—';
      }
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

    function markRowPending(row) {
      row.querySelectorAll('.control').forEach(button => {
        button.disabled = true;
        button.dataset.pending = 'true';
      });
      const composer = row.querySelector('.reply-composer');
      if (!composer) return;
      composer.dataset.pending = 'true';
      const input = composer.querySelector('.reply-input');
      const button = composer.querySelector('.reply-send');
      if (input) input.disabled = true;
      if (button) button.disabled = true;
    }

    function restoreRowActions(row) {
      const now = Date.now();
      row.querySelectorAll('.control').forEach(button => {
        button.dataset.pending = 'false';
        const expired = Number(button.dataset.expires) < now;
        button.disabled = expired;
        button.textContent = expired ? 'Expired' : button.dataset.originalLabel;
      });
      const composer = row.querySelector('.reply-composer');
      if (!composer) return;
      composer.dataset.pending = 'false';
      const input = composer.querySelector('.reply-input');
      const button = composer.querySelector('.reply-send');
      if (!input || !button) return;
      const expired = Number(composer.dataset.expires) < now;
      input.disabled = expired;
      button.disabled = expired || input.value.trim().length === 0;
      button.textContent = expired ? 'Expired' : 'Send';
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
        markRowPending(row);
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

    function replyComposer(item, control, row) {
      const composer = document.createElement('div');
      composer.className = 'reply-composer';
      composer.dataset.action = 'reply';
      composer.dataset.expires = String(control.expires_at_ms || 0);
      composer.dataset.pending = 'false';
      const input = document.createElement('textarea');
      input.className = 'reply-input';
      input.rows = 1;
      input.maxLength = 1000;
      input.setAttribute('aria-label', `Reply to ${item.agent || 'agent'}`);
      input.placeholder = `Reply to ${item.agent || 'agent'}…`;
      input.value = replyDrafts.get(item.id) || '';
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'reply-send';
      button.textContent = 'Send';

      const syncButton = () => {
        const expired = Number(control.expires_at_ms) < Date.now();
        button.disabled = expired || input.value.trim().length === 0;
      };
      const send = event => {
        event.stopPropagation();
        const value = input.value.trim();
        if (!value || button.disabled || Number(control.expires_at_ms) < Date.now()) return;
        markRowPending(row);
        button.textContent = 'Sending…';
        post(JSON.stringify({
          type: 'control',
          activity_id: item.id,
          action: 'reply',
          message: value,
          token: control.token,
          target_instance_id: control.target_instance_id
        }));
      };
      input.addEventListener('input', () => {
        replyDrafts.set(item.id, input.value);
        syncButton();
      });
      input.addEventListener('click', event => event.stopPropagation());
      input.addEventListener('keydown', event => {
        event.stopPropagation();
        if (event.key === 'Enter' && !event.shiftKey) {
          event.preventDefault();
          send(event);
        }
      });
      button.addEventListener('click', send);
      syncButton();
      composer.append(input, button);
      return composer;
    }

    function focusedReply() {
      const input = document.activeElement;
      if (!input || !input.classList || !input.classList.contains('reply-input')) return null;
      const row = input.closest('.activity');
      if (!row) return null;
      return {
        activityId: row.dataset.activityId,
        start: input.selectionStart,
        end: input.selectionEnd
      };
    }

    function restoreReplyFocus(focus) {
      if (!focus) return;
      const row = Array.from(document.querySelectorAll('.activity'))
        .find(candidate => candidate.dataset.activityId === focus.activityId);
      const input = row && row.querySelector('.reply-input');
      if (!input || input.disabled) return;
      try {
        input.focus({ preventScroll: true });
      } catch (_) {
        input.focus();
      }
      if (Number.isFinite(focus.start) && Number.isFinite(focus.end)) {
        input.setSelectionRange(focus.start, focus.end);
      }
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
      let reasonText = typeof item.reason === 'string' ? item.reason : '';
      if (!reasonText && item.state === 'waiting_approval') {
        reasonText = 'This operation is paused until you choose an approval action.';
      } else if (!reasonText && item.state === 'waiting_input') {
        reasonText = 'The agent is paused until you send the requested input.';
      }
      if (reasonText) {
        const reason = document.createElement('div');
        reason.className = 'attention-reason';
        text(reason, reasonText);
        copy.append(reason);
      }
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

      const itemControls = Array.isArray(item.controls) ? item.controls : [];
      const buttons = itemControls.filter(control => control.action !== 'reply');
      if (buttons.length) {
        const controls = document.createElement('div');
        controls.className = 'controls';
        buttons.forEach(control => controls.append(controlButton(item, control, row)));
        meta.append(controls);
      }
      row.append(copy, meta);
      const reply = itemControls.find(control => control.action === 'reply');
      if (reply) row.append(replyComposer(item, reply, row));
      return row;
    }

    function renderActivities(data) {
      const focus = focusedReply();
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
      restoreReplyFocus(focus);
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
      syncCompactPrimary(data);
      syncMetrics(metrics);
      summary.setAttribute(
        'aria-label',
        `${data.status || 'Idle'}. ${metrics.running} running, ${metrics.total} total. Show agent activity`
      );
      handleAttention(data);
      renderActivities(data);
      syncNeon();
    }

    function controlResult(result) {
      if (!result || typeof result.activity_id !== 'string') return;
      const row = Array.from(document.querySelectorAll('.activity'))
        .find(candidate => candidate.dataset.activityId === result.activity_id);
      if (!row) return;
      if (result.action === 'reply') {
        const composer = row.querySelector('.reply-composer');
        const input = composer && composer.querySelector('.reply-input');
        const button = composer && composer.querySelector('.reply-send');
        if (!composer || !input || !button) return;
        if (result.accepted === true) {
          replyDrafts.delete(result.activity_id);
          input.value = '';
          input.disabled = true;
          button.disabled = true;
          text(button, result.message, 'Queued');
          return;
        }
        restoreRowActions(row);
        if (!button.disabled) button.textContent = 'Try again';
        return;
      }
      const buttons = Array.from(row.querySelectorAll('.control'));
      const selected = buttons.find(button => button.dataset.action === result.action);
      if (result.accepted === true) {
        if (selected) text(selected, result.message, 'Sent');
        return;
      }
      restoreRowActions(row);
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
    dragHandle.addEventListener('mousedown', event => {
      event.stopPropagation();
      if (event.button !== 0) return;
      event.preventDefault();
      post('drag-window');
    });
    dragHandle.addEventListener('touchstart', event => {
      event.stopPropagation();
      event.preventDefault();
      post('drag-window');
    }, { passive: false });
    dragHandle.addEventListener('click', event => {
      event.preventDefault();
      event.stopPropagation();
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
      setScreenProfile,
      setExpanded,
      beginCollapse,
      finishCollapse
    };
    post('ready');
  })();
"#;
