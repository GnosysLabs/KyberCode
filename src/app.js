(() => {
  const lockup = document.getElementById("lockup");
  const error = document.getElementById("error");

  window.__KYBER__ = {
    setStatus() {
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
        error.textContent = String(text ?? "Kyber Code failed to start.");
      }
    },
  };
})();
