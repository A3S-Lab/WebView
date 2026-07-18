pub(super) const ISLAND_STYLE: &str = r#"
    :root {
      color-scheme: dark;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    * { box-sizing: border-box; }
    html, body {
      width: 100%;
      height: 100%;
      margin: 0;
      overflow: hidden;
      background: transparent;
    }
    body { user-select: none; -webkit-user-select: none; }
    button { font-family: inherit; }
    #island {
      position: absolute;
      top: 0;
      left: 50%;
      width: 296px;
      height: 44px;
      padding: 1px;
      transform: translateX(-50%);
      overflow: hidden;
      color: rgba(255,255,255,.96);
      background: rgba(255,255,255,.105);
      border-radius: 24px;
      box-shadow: 0 8px 28px rgba(0,0,0,.34);
      cursor: default;
      transition: width 220ms cubic-bezier(.2,.84,.2,1),
                  height 220ms cubic-bezier(.2,.84,.2,1),
                  border-radius 220ms ease;
      contain: layout paint;
      isolation: isolate;
    }
    #island.active-work {
      padding: 1.5px;
      background: linear-gradient(
        115deg,
        #38d8ff,
        #5865ff,
        #bd4dff,
        #ff4f9a,
        #ffb340,
        #39e6b1,
        #38d8ff
      );
      background-size: 320% 320%;
      box-shadow:
        0 8px 28px rgba(0,0,0,.36),
        0 0 12px rgba(77,181,255,.42),
        0 0 22px rgba(177,74,255,.27);
      animation: neon-shift 5.6s linear infinite, neon-breathe 2.35s ease-in-out infinite;
    }
    #island.has-attention:not(.active-work) {
      padding: 1.5px;
      background: linear-gradient(120deg, rgba(255,195,82,.9), rgba(255,119,91,.72));
      box-shadow:
        0 8px 28px rgba(0,0,0,.36),
        0 0 15px rgba(255,177,71,.3);
    }
    @keyframes neon-shift {
      0% { background-position: 0% 50%; }
      50% { background-position: 100% 50%; }
      100% { background-position: 0% 50%; }
    }
    @keyframes neon-breathe {
      0%, 100% {
        box-shadow:
          0 8px 28px rgba(0,0,0,.36),
          0 0 9px rgba(77,181,255,.32),
          0 0 16px rgba(177,74,255,.2);
      }
      50% {
        box-shadow:
          0 8px 30px rgba(0,0,0,.38),
          0 0 18px rgba(77,181,255,.68),
          0 0 30px rgba(224,73,255,.46);
      }
    }
    @keyframes attention-pulse {
      0%, 100% { box-shadow: 0 0 0 0 rgba(255,190,76,0); }
      50% { box-shadow: 0 0 0 3px rgba(255,190,76,.13); }
    }
    #island.expanded { width: 100%; height: 100%; border-radius: 28px; }
    .surface {
      width: 100%;
      height: 100%;
      overflow: hidden;
      background: rgba(3,3,5,.985);
      border-radius: inherit;
      box-shadow: inset 0 1px rgba(255,255,255,.035);
    }
    #island.has-attention .surface {
      box-shadow:
        inset 0 1px rgba(255,255,255,.045),
        inset 0 0 0 1px rgba(255,196,91,.075);
    }
    .summary {
      height: 41px;
      display: grid;
      grid-template-columns: 30px minmax(0,1fr) auto;
      align-items: center;
      column-gap: 8px;
      padding: 0 12px 0 9px;
      cursor: pointer;
    }
    .summary-copy { min-width: 0; line-height: 1.08; }
    .headline, .detail, .agent, .task, .workspace {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .headline {
      color: #f7f7f8;
      font-size: 12.5px;
      font-weight: 680;
      letter-spacing: -.01em;
    }
    .detail {
      margin-top: 3px;
      color: #92929d;
      font-size: 10px;
      font-weight: 520;
    }
    .summary-tail {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 6px;
    }
    .compact-attention {
      display: none;
      min-width: 17px;
      height: 17px;
      padding: 0 5px;
      border: 1px solid rgba(255,199,92,.34);
      border-radius: 9px;
      color: #ffd67d;
      background: rgba(177,112,25,.17);
      font-size: 8.75px;
      font-weight: 750;
      line-height: 15px;
      text-align: center;
      font-variant-numeric: tabular-nums;
    }
    .compact-attention.visible { display: block; }
    .chevron {
      width: 13px;
      color: #7d7d88;
      font-size: 14px;
      text-align: right;
      transform: rotate(0);
      transition: transform 180ms ease;
    }
    #island.expanded .chevron { transform: rotate(180deg); }
    .panel {
      height: calc(100% - 41px);
      display: flex;
      flex-direction: column;
      padding: 3px 9px 10px;
      opacity: 0;
      transform: translateY(-5px);
      transition: opacity 130ms ease, transform 180ms ease;
      pointer-events: none;
    }
    #island.expanded .panel {
      opacity: 1;
      transform: translateY(0);
      pointer-events: auto;
      transition-delay: 65ms;
    }
    .rule {
      flex: none;
      height: 1px;
      margin: 0 6px 7px;
      background: rgba(255,255,255,.075);
    }
    .panel-title {
      flex: none;
      min-height: 22px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0 7px 6px;
    }
    .panel-copy {
      min-width: 0;
      display: flex;
      align-items: baseline;
      gap: 8px;
    }
    .panel-copy strong {
      flex: none;
      color: #a1a1ab;
      font-size: 10.5px;
      letter-spacing: .055em;
      text-transform: uppercase;
    }
    .panel-copy span {
      overflow: hidden;
      color: #666671;
      font-size: 8.75px;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .panel-actions {
      flex: none;
      display: flex;
      align-items: center;
      gap: 8px;
    }
    .badge { display: none; color: #ffd166; font-size: 9px; }
    .badge.visible { display: block; }
    .island-power {
      height: 20px;
      padding: 0 8px;
      border: 1px solid rgba(255,104,119,.24);
      border-radius: 10px;
      color: #ffb2b9;
      background: rgba(199,55,70,.11);
      font-size: 8.75px;
      font-weight: 650;
      line-height: 18px;
      cursor: pointer;
      -webkit-appearance: none;
      appearance: none;
    }
    .island-power:hover:not(:disabled) {
      border-color: rgba(255,126,139,.42);
      background: rgba(199,55,70,.19);
    }
    .island-power:disabled { cursor: default; opacity: .55; }
    .filters {
      flex: none;
      display: flex;
      align-items: center;
      gap: 5px;
      min-height: 31px;
      padding: 2px 7px 7px;
    }
    .filter {
      height: 22px;
      display: inline-flex;
      align-items: center;
      gap: 5px;
      padding: 0 8px;
      border: 1px solid rgba(255,255,255,.075);
      border-radius: 11px;
      color: #82828d;
      background: rgba(255,255,255,.025);
      font-size: 8.75px;
      font-weight: 620;
      line-height: 20px;
      cursor: pointer;
      -webkit-appearance: none;
      appearance: none;
    }
    .filter b {
      min-width: 11px;
      color: #696974;
      font-size: 8px;
      font-weight: 720;
      font-variant-numeric: tabular-nums;
    }
    .filter:hover {
      color: #bcbcc5;
      border-color: rgba(255,255,255,.14);
      background: rgba(255,255,255,.055);
    }
    .filter.active {
      color: #e7e7ec;
      border-color: rgba(126,147,255,.29);
      background: rgba(91,108,207,.17);
    }
    .filter.active b { color: #b8c3ff; }
    #island.has-attention .attention-filter {
      animation: attention-pulse 2.4s ease-in-out infinite;
    }
    #island.has-attention .attention-filter:not(.active) {
      color: #e8bd68;
      border-color: rgba(255,190,76,.2);
      background: rgba(177,112,25,.08);
    }
    #activities {
      flex: 1;
      min-height: 0;
      overflow-x: hidden;
      overflow-y: auto;
      padding: 0 2px 2px;
      overscroll-behavior: contain;
      scrollbar-width: none;
    }
    #activities::-webkit-scrollbar { display: none; }
    .activity {
      position: relative;
      min-height: 49px;
      display: grid;
      grid-template-columns: 30px minmax(0,1fr) minmax(136px,auto);
      align-items: center;
      gap: 8px;
      padding: 6px 7px;
      border: 1px solid transparent;
      border-radius: 12px;
    }
    .activity:hover {
      border-color: rgba(255,255,255,.045);
      background: rgba(255,255,255,.035);
    }
    .activity + .activity { margin-top: 2px; }
    .activity.needs-attention {
      border-color: rgba(255,194,83,.075);
      background: rgba(151,94,18,.065);
    }
    .activity.context {
      border-color: rgba(112,146,255,.07);
      background: rgba(62,78,143,.055);
    }
    .activity.child {
      width: calc(100% - 17px);
      margin-left: 17px;
    }
    .activity.child::before {
      content: "";
      position: absolute;
      top: -4px;
      left: -11px;
      width: 8px;
      height: 27px;
      border-bottom: 1px solid rgba(255,255,255,.1);
      border-left: 1px solid rgba(255,255,255,.1);
      border-radius: 0 0 0 6px;
    }
    .copy { min-width: 0; line-height: 1.12; }
    .agent-line {
      min-width: 0;
      display: flex;
      align-items: center;
      gap: 6px;
    }
    .agent {
      min-width: 0;
      color: #efeff2;
      font-size: 11px;
      font-weight: 670;
    }
    .context-label {
      flex: none;
      padding: 1px 5px;
      border: 1px solid rgba(124,151,255,.17);
      border-radius: 7px;
      color: #8499e8;
      font-size: 7.5px;
      font-weight: 650;
      letter-spacing: .02em;
    }
    .task {
      margin-top: 4px;
      color: #9696a0;
      font-size: 9.75px;
    }
    .child-progress {
      display: flex;
      align-items: center;
      gap: 7px;
      margin-top: 5px;
    }
    .progress-label {
      flex: none;
      color: #747480;
      font-size: 8px;
      font-variant-numeric: tabular-nums;
    }
    .progress-rail {
      width: 54px;
      height: 3px;
      overflow: hidden;
      border-radius: 2px;
      background: rgba(255,255,255,.08);
    }
    .progress-fill {
      width: 0;
      height: 100%;
      border-radius: inherit;
      background: linear-gradient(90deg, #7295ff, #62d5b1);
      transition: width 180ms ease;
    }
    .row-meta {
      min-width: 136px;
      max-width: 196px;
      text-align: right;
    }
    .status-line {
      min-height: 13px;
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 6px;
      white-space: nowrap;
    }
    .status { font-size: 9.5px; font-weight: 650; }
    .duration {
      min-width: 34px;
      color: #8b8b96;
      font-size: 9px;
      font-variant-numeric: tabular-nums;
    }
    .workspace { margin-top: 3px; color: #6f6f79; font-size: 8.75px; }
    .controls {
      min-height: 0;
      display: flex;
      justify-content: flex-end;
      gap: 4px;
      margin-top: 4px;
    }
    .control {
      min-width: 35px;
      height: 18px;
      padding: 0 7px;
      border: 1px solid rgba(255,255,255,.13);
      border-radius: 9px;
      color: #d9d9df;
      background: rgba(255,255,255,.065);
      font-size: 8.5px;
      font-weight: 650;
      line-height: 16px;
      text-align: center;
      cursor: pointer;
      -webkit-appearance: none;
      appearance: none;
    }
    .control:hover:not(:disabled) {
      border-color: rgba(255,255,255,.25);
      background: rgba(255,255,255,.12);
    }
    .control.allow {
      color: #b8f6d4;
      border-color: rgba(70,211,138,.28);
      background: rgba(36,162,99,.13);
    }
    .control.always {
      color: #c6d9ff;
      border-color: rgba(99,148,255,.28);
      background: rgba(67,110,211,.14);
    }
    .control.destructive {
      color: #ffb2b9;
      border-color: rgba(255,104,119,.25);
      background: rgba(199,55,70,.12);
    }
    .control:disabled { cursor: default; opacity: .48; }
    .empty-state {
      height: 100%;
      min-height: 110px;
      display: grid;
      place-content: center;
      justify-items: center;
      padding: 24px;
      color: #71717c;
      text-align: center;
    }
    .empty-icon {
      width: 28px;
      height: 28px;
      display: grid;
      place-items: center;
      margin-bottom: 8px;
      border: 1px solid rgba(255,255,255,.08);
      border-radius: 50%;
      color: #858590;
      background: rgba(255,255,255,.025);
      font-size: 12px;
    }
    .empty-title { color: #a0a0aa; font-size: 10px; font-weight: 650; }
    .empty-detail { margin-top: 4px; color: #62626c; font-size: 8.75px; }
    .status.planning { color: #8bb9ff; }
    .status.working { color: #73d99c; }
    .status.attention { color: #ffd166; }
    .status.idle { color: #888893; }
    .status.success { color: #79e3a7; }
    .status.danger { color: #ff757f; }
    .status.cancelled { color: #b0a7bd; }
    .status.inferred { color: #9da5b4; }

    /* Original robot geometry; vendor identity is color-only, never a copied logo. */
    .robot {
      --brand-a: #8a72ff;
      --brand-b: #52c8ff;
      --brand-ink: #e9f5ff;
      position: relative;
      width: 28px;
      height: 26px;
      filter: drop-shadow(0 2px 5px rgba(0,0,0,.32));
    }
    .robot.a3s { --brand-a: #796cff; --brand-b: #42d5ff; --brand-ink: #eef7ff; }
    .robot.open_ai { --brand-a: #0d8068; --brand-b: #20c997; --brand-ink: #e8fff7; }
    .robot.anthropic { --brand-a: #a9563f; --brand-b: #df8c68; --brand-ink: #fff3ec; }
    .robot.google { --brand-a: #4285f4; --brand-b: #eab43f; --brand-ink: #f2f7ff; }
    .robot.cursor { --brand-a: #777d9d; --brand-b: #d4d7e8; --brand-ink: #14151b; }
    .robot.moonshot { --brand-a: #096dd9; --brand-b: #4ba6ff; --brand-ink: #f2f8ff; }
    .robot.tencent { --brand-a: #0a927c; --brand-b: #19d1ad; --brand-ink: #effffb; }
    .robot.alibaba { --brand-a: #6a42df; --brand-b: #f08b38; --brand-ink: #fff8ee; }
    .robot.deep_seek { --brand-a: #315bea; --brand-b: #648cff; --brand-ink: #f2f5ff; }
    .robot.mistral { --brand-a: #e85d2a; --brand-b: #ffb13b; --brand-ink: #fff8ea; }
    .robot .antenna {
      position: absolute;
      top: 0;
      left: 13px;
      width: 2px;
      height: 5px;
      border-radius: 2px;
      background: var(--brand-b);
    }
    .robot .antenna::before {
      content: "";
      position: absolute;
      top: -2px;
      left: -2px;
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: var(--brand-b);
      box-shadow: 0 0 6px var(--brand-b);
    }
    .robot .ear {
      position: absolute;
      top: 10px;
      width: 3px;
      height: 9px;
      border-radius: 3px;
      background: var(--brand-a);
      opacity: .8;
    }
    .robot .ear.left { left: 0; }
    .robot .ear.right { right: 0; }
    .robot .head {
      position: absolute;
      top: 5px;
      left: 2px;
      width: 24px;
      height: 19px;
      border: 1px solid rgba(255,255,255,.2);
      border-radius: 8px 8px 9px 9px;
      background: linear-gradient(145deg, var(--brand-b), var(--brand-a));
      box-shadow: inset 0 1px rgba(255,255,255,.25);
    }
    .robot .face {
      position: absolute;
      inset: 4px 3px 3px;
      border-radius: 5px 5px 6px 6px;
      background: rgba(4,6,10,.74);
    }
    .robot .eye {
      position: absolute;
      top: 4px;
      width: 4px;
      height: 4px;
      border-radius: 50%;
      background: var(--brand-ink);
      box-shadow: 0 0 4px var(--brand-ink);
    }
    .robot .eye.left { left: 3px; }
    .robot .eye.right { right: 3px; }
    .robot .mouth {
      position: absolute;
      left: 50%;
      bottom: 3px;
      width: 7px;
      height: 2px;
      transform: translateX(-50%);
      border-radius: 2px;
      background: var(--brand-ink);
      opacity: .78;
    }
    .robot .pip {
      position: absolute;
      right: -2px;
      bottom: 0;
      width: 7px;
      height: 7px;
      border: 1.5px solid #08080a;
      border-radius: 50%;
      background: #8f96a5;
    }
    .robot .pip.working {
      background: #51df91;
      box-shadow: 0 0 5px rgba(81,223,145,.8);
    }
    .robot .pip.planning {
      background: #70aaff;
      box-shadow: 0 0 5px rgba(112,170,255,.8);
    }
    .robot .pip.attention {
      background: #ffd166;
      box-shadow: 0 0 5px rgba(255,209,102,.75);
    }
    .robot .pip.success { background: #60df98; }
    .robot .pip.danger { background: #ff6572; }
    .robot .pip.cancelled { background: #9e94ae; }
    .robot .pip.idle, .robot .pip.inferred { background: #858b99; }

    @media (prefers-reduced-motion: reduce) {
      #island, .panel, .chevron, .progress-fill, .attention-filter {
        transition: none !important;
        animation: none !important;
      }
      #island.active-work {
        background-position: 48% 50%;
        box-shadow:
          0 8px 28px rgba(0,0,0,.36),
          0 0 12px rgba(77,181,255,.42),
          0 0 20px rgba(177,74,255,.25);
      }
    }
    html.webview-backgrounded #island,
    html.webview-backgrounded .panel,
    html.webview-backgrounded .chevron,
    html.webview-backgrounded .progress-fill,
    html.webview-backgrounded .attention-filter {
      transition: none !important;
      animation: none !important;
    }
"#;
