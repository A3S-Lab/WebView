function report(step) {
  return fetch(`/smoke-log?step=${encodeURIComponent(step)}`, {
    cache: "no-store",
  });
}

void report("remote-loaded")
  .catch(() => {})
  .finally(() => {
    window.setTimeout(() => window.location.assign("/remote-next.html"), 250);
  });
