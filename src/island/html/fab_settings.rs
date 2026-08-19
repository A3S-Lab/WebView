pub(super) const FAB_SETTINGS_SCRIPT: &str = r#"
    const reviewerLinks = Array.from(document.querySelectorAll('[data-reviewer-view]'));
    const reviewerPanels = Array.from(document.querySelectorAll('[data-reviewer-panel]'));
    const surfaceHealth = document.getElementById('surface-health');
    const navUnread = document.getElementById('nav-unread');
    const channelSummary = document.getElementById('channel-summary');
    const channelList = document.getElementById('channel-list');
    const restartRequired = document.getElementById('restart-required');
    const llmSettingsForm = document.getElementById('llm-settings-form');
    const llmSecretForm = document.getElementById('llm-secret-form');
    const llmEnabled = document.getElementById('llm-enabled');
    const llmProvider = document.getElementById('llm-provider');
    const llmModel = document.getElementById('llm-model');
    const llmBaseUrl = document.getElementById('llm-base-url');
    const llmApiKeyRef = document.getElementById('llm-api-key-ref');
    const llmEvidence = document.getElementById('llm-evidence');
    const shareHabits = document.getElementById('share-habits');
    const shareKnowledge = document.getElementById('share-knowledge');
    const shareTransitions = document.getElementById('share-transitions');
    const llmSettingsResult = document.getElementById('llm-settings-result');
    const saveLlmSettings = document.getElementById('save-llm-settings');
    const llmApiKey = document.getElementById('llm-api-key');
    const llmSecretResult = document.getElementById('llm-secret-result');
    const saveLlmSecret = document.getElementById('save-llm-secret');
    const reviewerViews = ['suggestions', 'channels', 'model'];
    const settingActions = [
      'start_channel_pairing',
      'advance_channel_pairing',
      'save_llm_configuration',
      'set_llm_api_key'
    ];
    let activeReviewerView = 'suggestions';
    let settingsPending = null;
    let llmFormDirty = false;
    let renderedLlmRevision = null;
    let expectedLlmRevision = null;

    function settingsControl(owner, action) {
      const actions = owner && Array.isArray(owner.actions) ? owner.actions : [];
      return actions.find(control => control.action === action) || null;
    }

    function settingsTokenExists(data, token) {
      if (!token || !data || !data.settings) return false;
      const channels = Array.isArray(data.settings.channels) ? data.settings.channels : [];
      const channelControls = channels.flatMap(channel => {
        const direct = Array.isArray(channel.actions) ? channel.actions : [];
        const pairing = channel.pairing && Array.isArray(channel.pairing.actions)
          ? channel.pairing.actions
          : [];
        return direct.concat(pairing);
      });
      const llmControls = data.settings.llm && Array.isArray(data.settings.llm.actions)
        ? data.settings.llm.actions
        : [];
      return channelControls.concat(llmControls).some(control => control.token === token);
    }

    function setFormResult(node, message, error = false) {
      text(node, message, '');
      node.classList.toggle('error', error === true);
    }

    function postReviewerControl(activityId, control, message, statusNode) {
      if (
        typeof activityId !== 'string'
        || !activityId.length
        || !control
        || Number(control.expires_at_ms) < Date.now()
      ) {
        setFormResult(statusNode, 'Authorization expired. Wait for the next refresh.', true);
        return false;
      }
      settingsPending = {
        activityId,
        action: control.action,
        token: control.token
      };
      setFormResult(statusNode, 'Submitting exact request…');
      const payload = {
        type: 'control',
        activity_id: activityId,
        action: control.action,
        transport: control.transport || 'durable_queue',
        token: control.token,
        target_instance_id: control.target_instance_id
      };
      if (typeof message === 'string') payload.message = message;
      post(JSON.stringify(payload));
      syncReviewerControlAvailability();
      return true;
    }

    function activateReviewerView(view) {
      if (!reviewerViews.includes(view)) return;
      activeReviewerView = view;
      reviewerLinks.forEach(link => {
        if (link.dataset.reviewerView === view) {
          link.setAttribute('aria-current', 'page');
        } else {
          link.removeAttribute('aria-current');
        }
      });
      reviewerPanels.forEach(panel => {
        panel.hidden = panel.dataset.reviewerPanel !== view;
      });
      if (model) renderFabSurface(model);
    }

    function channelStateCopy(state) {
      switch (state) {
        case 'connected': return ['Connected', 'success'];
        case 'pairing': return ['Pairing', 'warning'];
        case 'degraded': return ['Needs attention', 'destructive'];
        default: return ['Not connected', 'secondary'];
      }
    }

    function pairingStateCopy(state) {
      switch (state) {
        case 'waiting_for_scan': return ['Scan the code', 'Open the channel app and scan this code.'];
        case 'scanned': return ['Scan received', 'Waiting for the channel to finish authentication.'];
        case 'verification_required': return ['Verification required', 'Enter the code shown by the channel.'];
        case 'connected': return ['Connected', 'The channel account is ready.'];
        case 'already_connected': return ['Already connected', 'This channel account was already linked.'];
        case 'expired': return ['Pairing expired', 'Start a new pairing session.'];
        case 'failed': return ['Pairing failed', 'Review the channel response and try again.'];
        default: return ['Pairing in progress', 'Waiting for the channel.'];
      }
    }

    function drawQr(canvas, qr) {
      const size = Number(qr && qr.size);
      const rows = qr && Array.isArray(qr.rows) ? qr.rows : [];
      if (!Number.isInteger(size) || size < 1 || size > 177 || rows.length !== size) {
        canvas.hidden = true;
        return;
      }
      const quiet = 4;
      const scale = 4;
      const dimension = (size + quiet * 2) * scale;
      canvas.width = dimension;
      canvas.height = dimension;
      const context = canvas.getContext('2d', { alpha: false });
      if (!context) {
        canvas.hidden = true;
        return;
      }
      context.imageSmoothingEnabled = false;
      context.fillStyle = '#ffffff';
      context.fillRect(0, 0, dimension, dimension);
      context.fillStyle = '#090b10';
      for (let y = 0; y < size; y += 1) {
        const row = rows[y];
        if (typeof row !== 'string' || row.length !== size || /[^01]/.test(row)) {
          canvas.hidden = true;
          return;
        }
        for (let x = 0; x < size; x += 1) {
          if (row[x] === '1') {
            context.fillRect((x + quiet) * scale, (y + quiet) * scale, scale, scale);
          }
        }
      }
      canvas.hidden = false;
    }

    function channelCard(channel) {
      const card = document.createElement('article');
      card.className = 'card channel-card';
      card.dataset.size = 'sm';

      const header = document.createElement('header');
      header.className = 'channel-card-header';
      const titleGroup = document.createElement('div');
      const title = document.createElement('h3');
      text(title, channel.display_name, channel.id || 'IM channel');
      const protocol = document.createElement('p');
      text(protocol, channel.protocol_version, 'Connector protocol');
      titleGroup.append(title, protocol);
      const state = document.createElement('span');
      const [stateLabel, stateVariant] = channelStateCopy(channel.state);
      state.className = 'badge';
      state.dataset.variant = stateVariant;
      text(state, stateLabel);
      header.append(titleGroup, state);
      card.append(header);

      const section = document.createElement('section');
      const facts = document.createElement('div');
      facts.className = 'channel-facts';
      const accounts = document.createElement('span');
      text(accounts, plural(finiteCount(channel.account_count), 'account'));
      const bindings = document.createElement('span');
      text(bindings, plural(finiteCount(channel.binding_count), 'binding'));
      const capabilities = document.createElement('span');
      const capabilityLabels = [];
      if (channel.qr_login === true) capabilityLabels.push('QR login');
      if (channel.text_commands === true) capabilityLabels.push('Chat control');
      text(capabilities, capabilityLabels.join(' · '), 'Status only');
      facts.append(accounts, bindings, capabilities);
      section.append(facts);

      if (channel.pairing) {
        const pairing = document.createElement('div');
        pairing.className = 'pairing-surface';
        const canvas = document.createElement('canvas');
        canvas.className = 'pairing-qr';
        canvas.setAttribute('aria-label', `Pair ${channel.display_name || channel.id}`);
        if (channel.pairing.qr) drawQr(canvas, channel.pairing.qr);
        else canvas.hidden = true;
        const copy = document.createElement('div');
        copy.className = 'pairing-copy';
        const [pairingTitle, pairingDetail] = pairingStateCopy(channel.pairing.state);
        const heading = document.createElement('strong');
        text(heading, pairingTitle);
        const detailNode = document.createElement('p');
        text(detailNode, channel.pairing.failure || pairingDetail);
        copy.append(heading, detailNode);
        const advance = settingsControl(channel.pairing, 'advance_channel_pairing');
        if (channel.pairing.state === 'verification_required' && advance) {
          const form = document.createElement('form');
          form.className = 'verification-form';
          const code = document.createElement('input');
          code.className = 'input';
          code.type = 'text';
          code.maxLength = 64;
          code.autocomplete = 'one-time-code';
          code.placeholder = 'Verification code';
          code.setAttribute('aria-label', `Verification code for ${channel.display_name || channel.id}`);
          const submit = document.createElement('button');
          submit.className = 'btn';
          submit.dataset.size = 'sm';
          submit.type = 'submit';
          submit.textContent = 'Verify';
          submit.disabled = settingsPending !== null || Number(advance.expires_at_ms) < Date.now();
          form.addEventListener('submit', event => {
            event.preventDefault();
            const value = code.value.trim();
            if (!value) {
              code.setAttribute('aria-invalid', 'true');
              return;
            }
            code.removeAttribute('aria-invalid');
            postReviewerControl(channel.activity_id, advance, value, detailNode);
          });
          form.append(code, submit);
          copy.append(form);
        }
        pairing.append(canvas, copy);
        section.append(pairing);
      }
      card.append(section);

      const connect = settingsControl(channel, 'start_channel_pairing');
      if (connect) {
        const footer = document.createElement('footer');
        const button = document.createElement('button');
        button.className = 'btn';
        button.dataset.size = 'sm';
        button.type = 'button';
        button.textContent = 'Connect channel';
        button.disabled = settingsPending !== null || Number(connect.expires_at_ms) < Date.now();
        button.addEventListener('click', () => {
          postReviewerControl(channel.activity_id, connect, undefined, button);
        });
        footer.append(button);
        card.append(footer);
      }
      return card;
    }

    function renderChannels(data) {
      const channels = data.settings && Array.isArray(data.settings.channels)
        ? data.settings.channels
        : [];
      channelList.replaceChildren();
      const connected = channels.filter(channel => channel.state === 'connected').length;
      text(
        channelSummary,
        channels.length ? `${connected}/${channels.length} connected` : 'Not configured'
      );
      if (!channels.length) {
        const empty = document.createElement('div');
        empty.className = 'channel-empty';
        const title = document.createElement('strong');
        title.textContent = data.degraded === true ? 'Channel state unavailable' : 'No channel connectors';
        const detailNode = document.createElement('span');
        detailNode.textContent = data.degraded === true
          ? 'The Reviewer will retry the local projection automatically.'
          : 'Install or configure an IM connector to enable chat control.';
        empty.append(title, detailNode);
        channelList.append(empty);
        return;
      }
      channels.forEach(channel => channelList.append(channelCard(channel)));
    }

    function validReviewerBaseUrl(value) {
      if (!value) return true;
      try {
        const parsed = new URL(value);
        const loopback = parsed.protocol === 'http:'
          && ['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname);
        return (parsed.protocol === 'https:' || loopback)
          && !parsed.username
          && !parsed.password
          && !parsed.search
          && !parsed.hash;
      } catch (_) {
        return false;
      }
    }

    function populateLlmForm(llm) {
      llmEnabled.checked = llm.enabled === true;
      llmProvider.value = typeof llm.provider === 'string' ? llm.provider : '';
      llmModel.value = typeof llm.model === 'string' ? llm.model : '';
      llmBaseUrl.value = typeof llm.base_url === 'string' ? llm.base_url : '';
      llmApiKeyRef.value = typeof llm.api_key_ref === 'string' ? llm.api_key_ref : '';
      llmEvidence.value = llm.evidence === 'redacted_error' ? 'redacted_error' : 'metadata';
      shareHabits.checked = llm.share_project_habits === true;
      shareKnowledge.checked = llm.share_project_knowledge === true;
      shareTransitions.checked = llm.share_project_transitions === true;
      renderedLlmRevision = Number(llm.revision) || 0;
      llmFormDirty = false;
    }

    function currentLlm() {
      return model && model.settings && model.settings.llm ? model.settings.llm : null;
    }

    function syncReviewerControlAvailability() {
      const llm = currentLlm();
      const saveSettingsControl = settingsControl(llm, 'save_llm_configuration');
      const saveSecretControl = settingsControl(llm, 'set_llm_api_key');
      const pending = settingsPending !== null;
      saveLlmSettings.disabled = pending
        || !saveSettingsControl
        || Number(saveSettingsControl.expires_at_ms) < Date.now();
      saveLlmSecret.disabled = pending
        || !saveSecretControl
        || Number(saveSecretControl.expires_at_ms) < Date.now()
        || llmApiKey.value.length === 0;
      Array.from(llmSettingsForm.elements).forEach(element => {
        if (element !== saveLlmSettings) element.disabled = pending || !llm;
      });
      llmApiKey.disabled = pending || !llm || !saveSecretControl;
    }

    function renderLlmSettings(data) {
      const llm = data.settings && data.settings.llm ? data.settings.llm : null;
      restartRequired.hidden = !(llm && llm.restart_required === true);
      if (!llm) {
        saveLlmSettings.disabled = true;
        saveLlmSecret.disabled = true;
        setFormResult(llmSettingsResult, 'Configuration state unavailable.', true);
        return;
      }
      const revision = Number(llm.revision) || 0;
      if (expectedLlmRevision !== null && revision >= expectedLlmRevision) {
        expectedLlmRevision = null;
        settingsPending = null;
        llmFormDirty = false;
        setFormResult(
          llmSettingsResult,
          llm.restart_required === true ? 'Saved. Restart the daemon to activate it.' : 'Saved.'
        );
      }
      const focused = llmSettingsForm.contains(document.activeElement);
      if (!llmFormDirty && !focused && renderedLlmRevision !== revision) {
        populateLlmForm(llm);
      }
      syncReviewerControlAvailability();
    }

    function renderReviewerChrome(data) {
      surfaceHealth.hidden = data.degraded !== true;
      const items = Array.isArray(data.activities) ? data.activities : [];
      const suggestions = items.filter(item => item.kind === 'coding_suggestion');
      const unread = suggestions.filter(item => item.unread === true).length;
      navUnread.hidden = unread === 0;
      text(navUnread, unread > 99 ? '99+' : String(unread));
      fabBadge.classList.toggle('visible', unread > 0);
      text(fabBadge, unread > 99 ? '99+' : String(unread));
      fabBadge.setAttribute(
        'aria-label',
        unread > 0 ? plural(unread, 'new suggestion') : 'No new suggestions'
      );
      root.classList.toggle('has-attention', unread > 0);

      const view = suggestionModel(data);
      const globalEnabled = !view.settings || view.settings.enabled !== false;
      globalSuggestionToggle.setAttribute('aria-checked', globalEnabled ? 'true' : 'false');
      text(globalSuggestionLabel, globalEnabled ? 'Suggestions on' : 'Suggestions paused');
      const globalControl = view.settings && controlFor(
        view.settings,
        globalEnabled ? 'disable_suggestions' : 'enable_suggestions'
      );
      globalSuggestionToggle.dataset.available = globalControl ? 'true' : 'false';
      globalSuggestionToggle.dataset.expires = String(globalControl ? globalControl.expires_at_ms : 0);
      globalSuggestionToggle.disabled = !globalControl || suggestionPending !== null;
      suggestionSummary.textContent = [
        plural(view.sessions.length, 'session'),
        plural(view.suggestions.length, 'pending suggestion')
      ].join(' · ');
    }

    function renderFabSurface(data) {
      if (!isFab) return;
      if (settingsPending && !settingsTokenExists(data, settingsPending.token)) {
        settingsPending = null;
      }
      renderReviewerChrome(data);
      if (activeReviewerView === 'channels') {
        renderChannels(data);
      } else if (activeReviewerView === 'model') {
        renderLlmSettings(data);
      } else {
        renderSuggestionSurface(data);
      }
      syncReviewerControlAvailability();
    }

    function handleReviewerSettingsControlResult(result) {
      if (!isFab || !settingActions.includes(result.action)) return false;
      if (result.action === 'save_llm_configuration') {
        if (result.accepted === true) {
          const llm = currentLlm();
          expectedLlmRevision = (Number(llm && llm.revision) || 0) + 1;
          setFormResult(llmSettingsResult, 'Queued. Waiting for daemon confirmation…');
        } else {
          settingsPending = null;
          setFormResult(llmSettingsResult, result.message || 'Save failed. Review and retry.', true);
        }
      } else if (result.action === 'set_llm_api_key') {
        settingsPending = null;
        if (result.accepted === true) {
          llmApiKey.value = '';
          setFormResult(llmSecretResult, 'API key replaced in Keychain.');
        } else {
          setFormResult(llmSecretResult, result.message || 'API key was not replaced.', true);
        }
      } else if (result.accepted !== true) {
        settingsPending = null;
      }
      if (model) renderFabSurface(model);
      return true;
    }

    reviewerLinks.forEach(link => {
      link.addEventListener('click', event => {
        event.preventDefault();
        event.stopPropagation();
        activateReviewerView(link.dataset.reviewerView);
      });
    });

    Array.from(llmSettingsForm.elements).forEach(element => {
      if (element === saveLlmSettings) return;
      element.addEventListener('input', () => {
        llmFormDirty = true;
        setFormResult(llmSettingsResult, 'Unsaved changes');
      });
      element.addEventListener('change', () => {
        llmFormDirty = true;
        setFormResult(llmSettingsResult, 'Unsaved changes');
      });
    });
    shareTransitions.addEventListener('change', () => {
      if (shareTransitions.checked) shareKnowledge.checked = true;
    });
    llmApiKey.addEventListener('input', syncReviewerControlAvailability);

    llmSettingsForm.addEventListener('submit', event => {
      event.preventDefault();
      const llm = currentLlm();
      const control = settingsControl(llm, 'save_llm_configuration');
      if (!llm || !control) {
        setFormResult(llmSettingsResult, 'Settings authorization is unavailable.', true);
        return;
      }
      const provider = llmProvider.value.trim();
      const modelName = llmModel.value.trim();
      const baseUrl = llmBaseUrl.value.trim();
      const keyReference = llmApiKeyRef.value.trim();
      if (llmEnabled.checked && (!provider || !modelName || !keyReference)) {
        setFormResult(llmSettingsResult, 'Enabled review requires provider, model, and Keychain reference.', true);
        return;
      }
      if (!validReviewerBaseUrl(baseUrl)) {
        llmBaseUrl.setAttribute('aria-invalid', 'true');
        setFormResult(llmSettingsResult, 'Use HTTPS, or loopback HTTP, without credentials or query parameters.', true);
        return;
      }
      llmBaseUrl.removeAttribute('aria-invalid');
      if (shareTransitions.checked && !shareKnowledge.checked) {
        setFormResult(llmSettingsResult, 'Recent transitions require knowledge graph disclosure.', true);
        return;
      }
      const settings = {
        enabled: llmEnabled.checked,
        provider: provider || null,
        model: modelName || null,
        api_key_ref: keyReference || null,
        base_url: baseUrl || null,
        evidence: llmEvidence.value === 'redacted_error' ? 'redacted_error' : 'metadata',
        share_project_habits: shareHabits.checked,
        share_project_knowledge: shareKnowledge.checked,
        share_project_transitions: shareTransitions.checked
      };
      postReviewerControl(llm.activity_id, control, JSON.stringify(settings), llmSettingsResult);
    });

    llmSecretForm.addEventListener('submit', event => {
      event.preventDefault();
      const llm = currentLlm();
      const control = settingsControl(llm, 'set_llm_api_key');
      if (!llm || !control || !llmApiKey.value.length) {
        setFormResult(llmSecretResult, 'Save a Keychain reference before replacing the key.', true);
        return;
      }
      postReviewerControl(llm.activity_id, control, llmApiKey.value, llmSecretResult);
    });
"#;
