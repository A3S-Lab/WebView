function report(step) {
  return fetch(`/smoke-log?step=${encodeURIComponent(step)}`, {
    cache: "no-store",
  });
}

void report("remote-same-origin-navigation")
  .catch(() => {})
  .finally(() => {
    window.setTimeout(() => {
      window.location.assign("http://127.0.0.1:4318/forbidden.html");
    }, 250);
    window.setTimeout(() => {
      if (
        window.location.origin === "http://127.0.0.1:4319" &&
        window.location.pathname === "/remote-next.html"
      ) {
        document.querySelector("#result").textContent =
          "Cross-origin navigation was blocked by the native host.";
        void report("remote-cross-origin-navigation-blocked").catch(() => {});
      }
    }, 750);
  });
