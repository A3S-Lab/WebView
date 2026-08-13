const version = "a3s.workspace.v1";
const localResourceId = "smoke:document";
const localGeneration = 1;
const remoteResourceId = "smoke:remote";
const remoteGeneration = 2;
const phase = document.querySelector("#phase");
const trace = document.querySelector("#trace");
const slot = document.querySelector("#slot");
const overlay = document.querySelector("#overlay");
const restore = document.querySelector("#restore");
let opened = false;
let remoteOpened = false;
let overlayTimer = null;
let restoreTimer = null;

function record(message, pass = true) {
  const item = document.createElement("li");
  item.textContent = message;
  item.dataset.pass = String(pass);
  trace.append(item);
}

function bounds() {
  const rect = slot.getBoundingClientRect();
  return {
    x: Math.round(rect.x),
    y: Math.round(rect.y),
    width: Math.round(rect.width),
    height: Math.round(rect.height),
  };
}

function post(command) {
  window.a3sWorkspaceHost.postMessage({ version, ...command });
}

function report(step) {
  return fetch(`/smoke-log?step=${encodeURIComponent(step)}`, {
    cache: "no-store",
  });
}

function open() {
  if (opened) return;
  opened = true;
  record("host bridge ready");
  post({
    type: "workspace.open",
    resourceId: localResourceId,
    generation: localGeneration,
    title: "Workspace smoke document",
    target: { kind: "local_app", path: "/child.html" },
    bounds: bounds(),
    policy: { bridge: "typed", allowedOrigins: [] },
  });
  post({
    type: "workspace.post_message",
    resourceId: localResourceId,
    generation: localGeneration,
    payload: {
      type: "artifact.replace",
      revision: 3,
      title: "Queued before ready",
      content: "The native FIFO delivered this revision after the typed ready handshake.",
    },
  });
}

function openRemote() {
  if (remoteOpened) return;
  remoteOpened = true;
  post({
    type: "workspace.open",
    resourceId: remoteResourceId,
    generation: remoteGeneration,
    title: "Remote workspace smoke",
    target: { kind: "remote", url: "http://127.0.0.1:4319/remote.html" },
    bounds: bounds(),
    policy: { bridge: "none", allowedOrigins: [] },
  });
  record("second-origin remote workspace opened");
}

window.addEventListener("a3s-workspace-event", (event) => {
  const detail = event.detail;
  if (detail?.type === "workspace.host_ready") {
    phase.textContent = "native host ready";
    open();
    return;
  }
  if (detail?.type === "workspace.lifecycle") {
    phase.textContent = detail.phase;
    record(`${detail.resourceId} · ${detail.phase}`, detail.phase !== "error");
    if (
      detail.resourceId === localResourceId &&
      detail.generation === localGeneration &&
      detail.phase === "ready" &&
      !overlayTimer
    ) {
      overlayTimer = window.setTimeout(() => {
        post({
          type: "workspace.occlusion",
          resourceId: localResourceId,
          generation: localGeneration,
          occluded: true,
        });
        overlay.dataset.open = "true";
        restore.focus();
        record("trusted overlay opened");
        restoreTimer = window.setTimeout(restoreWorkspace, 1_000);
      }, 900);
    }
    if (
      detail.resourceId === remoteResourceId &&
      detail.generation === remoteGeneration &&
      detail.phase === "ready"
    ) {
      void report("remote-ready").catch(() => {});
    }
    return;
  }
  if (detail?.type === "workspace.view_message") {
    record(`view bridge · ${detail.payload?.type ?? "message"}`);
    if (detail.payload?.type === "smoke.retained") {
      const step = detail.payload.retained ? "state-retained" : "state-lost";
      void report(step)
        .catch(() => {})
        .finally(() => {
          if (detail.payload.retained) openRemote();
        });
    }
    return;
  }
  if (detail?.type === "workspace.host_error") {
    phase.textContent = "host error";
    record(detail.message, false);
  }
});

const observer = new ResizeObserver(() => {
  if (!opened) return;
  post({
    type: "workspace.bounds",
    resourceId: remoteOpened ? remoteResourceId : localResourceId,
    generation: remoteOpened ? remoteGeneration : localGeneration,
    bounds: bounds(),
  });
});
observer.observe(slot);

function restoreWorkspace() {
  overlay.dataset.open = "false";
  post({
    type: "workspace.occlusion",
    resourceId: localResourceId,
    generation: localGeneration,
    occluded: false,
  });
  post({
    type: "workspace.post_message",
    resourceId: localResourceId,
    generation: localGeneration,
    payload: { type: "smoke.verify_retained" },
  });
  record("mounted workspace restored");
}

restore.addEventListener("click", () => {
  if (restoreTimer) window.clearTimeout(restoreTimer);
  restoreWorkspace();
});

if (window.a3sWorkspaceHost?.native) open();
