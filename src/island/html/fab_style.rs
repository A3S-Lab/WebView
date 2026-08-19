pub(super) const FAB_STYLE: &str = r#"
    #island.fab-mode {
      --expanded-width: 720px;
      --expanded-height: 560px;
      color-scheme: dark;
      --background: #07080c;
      --foreground: #eef1f8;
      --card: #10131a;
      --card-foreground: #eef1f8;
      --popover: #151923;
      --popover-foreground: #eef1f8;
      --primary: #5878d4;
      --primary-foreground: #f7f9ff;
      --secondary: #171b24;
      --secondary-foreground: #e7ebf4;
      --muted: #171b24;
      --muted-foreground: #a6aebe;
      --accent: #1b2231;
      --accent-foreground: #f4f6fb;
      --destructive: #e06f79;
      --border: #282d38;
      --input: #343b4b;
      --ring: #88a8ff;
      --a3s-bg: var(--background);
      --a3s-panel: var(--card);
      --a3s-panel-soft: var(--muted);
      --a3s-panel-strong: var(--accent);
      --a3s-ink: var(--foreground);
      --a3s-muted: var(--muted-foreground);
      --a3s-faint: #8e97a9;
      --a3s-line: var(--border);
      --a3s-line-strong: var(--input);
      --a3s-action: var(--primary);
      --a3s-action-hover: #6688e3;
      --a3s-action-ink: var(--primary-foreground);
    }
    #island.fab-mode.expanded .surface { color: #eef1f8; }
    #island.fab-mode .suggestion-header {
      min-height: 64px;
      padding: 15px 18px 11px;
    }
    #island.fab-mode .workspace-header { background: transparent; }
    #surface-health:not([hidden]) { display: inline-flex; }
    .reviewer-layout {
      flex: 1;
      min-height: 0;
      overflow: hidden;
      border-top: 1px solid rgba(255,255,255,.06);
      --settings-navigation-size: 8.75rem;
      --settings-section-gap: 0;
    }
    .reviewer-layout > aside {
      position: static;
      height: 100%;
      padding: 12px 9px;
      border-inline-end: 1px solid rgba(255,255,255,.075);
      background: #090b10;
    }
    .reviewer-layout > aside nav > ul { gap: 3px; }
    .reviewer-layout > aside a {
      min-height: 36px;
      justify-content: space-between;
      padding: 0 10px;
      border-radius: 10px;
      color: #aeb6c7;
      font-size: 10.5px;
      font-weight: 620;
      text-decoration: none;
    }
    .reviewer-layout > aside a:hover { color: #e7ebf4; background: #151923; }
    .reviewer-layout > aside a[aria-current="page"] {
      color: #f4f6fb;
      background: #1b2231;
      box-shadow: inset 0 0 0 1px rgba(135,159,222,.16);
    }
    #nav-unread {
      min-width: 18px;
      height: 18px;
      padding-inline: 5px;
      border: 0;
      color: #17100a;
      background: #ffc766;
      font-size: 9px;
    }
    .reviewer-layout > main.reviewer-views {
      position: relative;
      display: block;
      min-height: 0;
      height: 100%;
      overflow: hidden;
      background: #07090d;
    }
    .reviewer-view {
      width: 100%;
      height: 100%;
      min-height: 0;
    }
    #island.fab-mode .reviewer-layout > main > .reviewer-view {
      gap: 0;
      padding: 0;
      border: 0;
      border-radius: 0;
      background: #07090d;
    }
    .reviewer-view[hidden] { display: none !important; }
    .reviewer-view.suggestion-workbench {
      display: grid;
      grid-template-columns: 164px minmax(0,1fr);
    }
    .reviewer-view .suggestion-sessions { padding: 9px 7px 12px; }
    .reviewer-view .suggestion-detail { padding: 17px 18px 16px; }
    .suggestion-editor.approval-request {
      grid-template-rows: auto minmax(0,1fr) auto;
      gap: 12px;
      overflow: visible;
      border: 0;
      border-radius: 0;
      background: transparent;
      box-shadow: none;
    }
    .suggestion-editor.approval-request > header,
    .suggestion-editor.approval-request > section,
    .suggestion-editor.approval-request > footer {
      padding: 0;
      border: 0;
      background: transparent;
    }
    .suggestion-editor.approval-request > section {
      display: grid;
      min-height: 0;
      grid-template-rows: auto minmax(0,1fr);
      gap: 10px;
    }
    .suggestion-context h2 {
      margin: 0;
      overflow: hidden;
      color: #f0f2f7;
      font-size: 13px;
      font-weight: 690;
      line-height: 1.25;
      letter-spacing: -.01em;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .suggestion-context p {
      margin: 0;
      overflow: hidden;
      color: #9da5b5;
      font-size: 10px;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .suggestion-draft-label.field { min-height: 0; gap: 6px; }
    .suggestion-draft-label.field > span { color: #c8ceda; font-size: 10px; font-weight: 650; }
    .suggestion-draft.input { min-height: 0; height: 100%; font-size: 11px; }
    .suggestion-actions .btn { font-size: 10px; }

    .settings-view {
      display: block;
      overflow-x: hidden;
      overflow-y: auto;
      padding: 18px 20px 22px;
      scrollbar-width: thin;
      scrollbar-color: #343a49 transparent;
      user-select: text;
      -webkit-user-select: text;
    }
    #island.fab-mode .reviewer-layout > main > .settings-view { display: block; }
    .settings-view-header {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 16px;
      margin-bottom: 16px;
    }
    .settings-view-header > div { min-width: 0; }
    .settings-view-header h2 {
      margin: 0;
      color: #f3f5fa;
      font-size: 16px;
      font-weight: 720;
      line-height: 1.2;
      letter-spacing: -.02em;
    }
    .settings-view-header p {
      max-width: 58ch;
      margin: 5px 0 0;
      color: #a6aebe;
      font-size: 10.5px;
      line-height: 1.45;
    }
    .settings-view .badge,
    .suggestion-header .badge {
      border-color: rgba(255,255,255,.1);
      color: #c5cbd8;
      background: #171b24;
    }
    .channel-list { display: grid; gap: 10px; }
    .channel-card.card,
    .reviewer-form.card {
      border: 0;
      border-radius: 14px;
      color: #e9ecf3;
      background: #10131a;
      box-shadow: 0 9px 24px rgba(0,0,0,.22);
    }
    .channel-card.card > header,
    .channel-card.card > section,
    .channel-card.card > footer,
    .reviewer-form.card > header,
    .reviewer-form.card > section,
    .reviewer-form.card > footer { padding: 13px 15px; }
    .channel-card.card > header,
    .reviewer-form.card > header { border-bottom: 1px solid rgba(255,255,255,.07); }
    .channel-card.card > footer,
    .reviewer-form.card > footer { border-top: 1px solid rgba(255,255,255,.07); }
    .channel-card-header {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 12px;
    }
    .channel-card h3,
    .reviewer-form h3 { margin: 0; color: #eef1f7; font-size: 12px; font-weight: 690; }
    .channel-card p,
    .reviewer-form header p { margin: 4px 0 0; color: #9da5b5; font-size: 9.5px; line-height: 1.4; }
    .channel-facts { display: flex; flex-wrap: wrap; gap: 6px 14px; color: #abb3c2; font-size: 9.5px; }
    .pairing-surface { display: grid; grid-template-columns: auto minmax(0,1fr); gap: 15px; align-items: center; }
    .pairing-qr {
      width: 152px;
      height: 152px;
      border-radius: 10px;
      background: #fff;
      image-rendering: pixelated;
    }
    .pairing-copy { display: grid; gap: 8px; }
    .pairing-copy strong { color: #eff2f8; font-size: 11px; }
    .pairing-copy p { margin: 0; color: #a8b0c0; font-size: 10px; line-height: 1.45; }
    .verification-form { display: grid; grid-template-columns: minmax(0,1fr) auto; gap: 8px; }
    .channel-card .btn,
    .reviewer-form .btn { min-height: 32px; font-size: 10px; }
    .channel-empty {
      display: grid;
      place-content: center;
      min-height: 190px;
      padding: 28px;
      color: #a5adbd;
      text-align: center;
    }
    .channel-empty strong { color: #e4e8f0; font-size: 12px; }
    .channel-empty span { margin-top: 5px; font-size: 10px; }

    .reviewer-form { margin-bottom: 11px; }
    .reviewer-form .form-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0,1fr));
      gap: 12px;
    }
    .reviewer-form .field { min-width: 0; gap: 6px; color: #c7cedb; font-size: 10px; }
    .reviewer-form .field > span:first-child { font-weight: 620; }
    .reviewer-form .input:not([type="checkbox"]) {
      min-height: 34px;
      border-color: rgba(157,171,207,.18);
      color: #edf0f6;
      background: #090b10;
      font-size: 10.5px;
    }
    .reviewer-form .input::placeholder { color: #737d90; }
    .switch-field { grid-column: 1 / -1; }
    .switch-field > span,
    .disclosure-fields label > span { display: grid; gap: 2px; }
    .switch-field strong,
    .disclosure-fields strong { color: #e4e8f0; font-size: 10.5px; }
    .switch-field small,
    .disclosure-fields small { color: #939cad; font-size: 9px; line-height: 1.35; }
    .disclosure-fields {
      display: grid;
      grid-template-columns: repeat(3, minmax(0,1fr));
      gap: 9px;
      margin: 0;
      padding: 0 15px 14px;
      border: 0;
    }
    .disclosure-fields legend { padding-top: 13px; color: #c7cedb; font-size: 10px; font-weight: 650; }
    .disclosure-fields label { display: grid; grid-template-columns: auto minmax(0,1fr); gap: 8px; align-items: start; }
    .reviewer-form footer { display: flex; justify-content: space-between; gap: 12px; }
    .reviewer-form footer [role="status"] { color: #aab3c4; font-size: 9.5px; }
    .reviewer-form footer [role="status"].error { color: #ffabb3; }
    .secret-form section { display: block; }

    @media (max-width: 48rem) {
      .reviewer-layout { grid-template-columns: minmax(0,1fr); }
      .reviewer-layout > aside {
        height: auto;
        padding: 7px 10px;
        border-inline-end: 0;
        border-bottom: 1px solid rgba(255,255,255,.075);
      }
      .reviewer-layout > aside nav > ul { display: flex; overflow-x: auto; }
      .reviewer-layout > aside li { flex: none; }
      .reviewer-layout > aside a { min-height: 32px; }
      .reviewer-layout > main.reviewer-views { height: calc(100% - 47px); }
      .reviewer-form .form-grid,
      .disclosure-fields { grid-template-columns: minmax(0,1fr); }
    }
"#;
