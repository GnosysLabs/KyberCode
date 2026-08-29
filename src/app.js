(() => {
  const lockup = document.getElementById("lockup");
  const status = document.getElementById("status");
  const error = document.getElementById("error");

  window.__KYBER__ = {
    setStatus(text) {
      if (status) status.textContent = String(text ?? "");
      if (lockup) lockup.hidden = false;
      if (error) {
        error.hidden = true;
        error.textContent = "";
      }
    },
    setError(text) {
      if (lockup) lockup.hidden = true;
      if (error) {
        error.hidden = false;
        error.textContent = String(text ?? "DeepSeek Harness failed to start.");
      }
    },
  };
})();
