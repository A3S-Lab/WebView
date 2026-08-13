const bridge = window.a3sWorkspaceView;
const title = document.querySelector("#title");
const content = document.querySelector("#content");
const revision = document.querySelector("#revision");
const retained = document.querySelector("#retained");
const retainedValue = "Mounted state survived native occlusion.";

function report(step) {
  void fetch(`/smoke-log?step=${encodeURIComponent(step)}`, {
    cache: "no-store",
  }).catch(() => {});
}

function apply(message) {
  const payload = message?.payload;
  if (payload?.type === "smoke.verify_retained") {
    bridge.postMessage({
      type: "smoke.retained",
      retained: retained.value === retainedValue,
    });
    return;
  }
  if (payload?.type !== "artifact.replace") return;
  title.textContent = payload.title;
  content.textContent = payload.content;
  revision.textContent = `generation ${bridge.generation} · revision ${payload.revision}`;
  bridge.postMessage({ type: "smoke.applied", revision: payload.revision });
}

if (!bridge || bridge.version !== "a3s.workspace.v1") {
  report("bridge-missing");
  throw new Error("typed native workspace bridge is unavailable");
}

report("bridge-present");
retained.value = retainedValue;
window.addEventListener("a3s-workspace-message", (event) => apply(event.detail));
window.requestAnimationFrame(() => {
  report("calling-ready");
  bridge.ready();
  for (const message of bridge.consumePending()) apply(message);
});
