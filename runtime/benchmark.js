(() => {
  "use strict";

  const element = (id) => document.getElementById(id);
  const model = element("model-name");
  const start = element("start-button");
  const cancel = element("cancel-button");
  const status = element("benchmark-status");
  const cpu = element("cpu-usage");
  const ram = element("ram-usage");
  const gpu = element("gpu-telemetry");
  const results = element("benchmark-results");
  const csrf = document.querySelector('meta[name="daia-csrf-token"]');

  if (!model || !start || !cancel || !status || !cpu ||
      !ram || !gpu || !results || !csrf || !csrf.content) {
    return;
  }

  let running = false;
  let busy = false;

  function formatNumber(value, digits = 1) {
    return typeof value === "number" && Number.isFinite(value)
      ? value.toFixed(digits)
      : "unavailable";
  }

  function formatBytes(value) {
    return typeof value === "number" && Number.isFinite(value)
      ? (value / (1024 ** 3)).toFixed(2) + " GiB"
      : "unavailable";
  }

  function updateButtons() {
    start.disabled = busy || running || !model.value.trim();
    cancel.disabled = busy || !running;
    model.disabled = busy || running;
  }

  function displayTelemetry(sample) {
    cpu.textContent =
      "CPU: " + formatNumber(sample.cpu_utilization_percent) + "%";
    ram.textContent = "RAM: " +
      formatBytes(sample.ram_used_bytes) + " / " +
      formatBytes(sample.ram_total_bytes);

    gpu.replaceChildren();
    if (!Array.isArray(sample.gpus) || sample.gpus.length === 0) {
      gpu.textContent = "GPU telemetry: unavailable";
      return;
    }

    for (const device of sample.gpus) {
      const line = document.createElement("p");
      line.textContent =
        "GPU " + String(device.identifier ?? "unknown") +
        ": utilization " + formatNumber(device.utilization_percent) +
        "%, memory " + formatBytes(device.memory_used_bytes) +
        " / " + formatBytes(device.memory_total_bytes) +
        ", temperature " + formatNumber(device.temperature_celsius) +
        " °C, power " + formatNumber(device.power_watts) + " W";
      gpu.appendChild(line);
    }
  }

  function displayResults(event) {
    results.replaceChildren();
    const runs = Array.isArray(event.runs) ? event.runs : [];
    const measured = runs.filter((run) => run.kind === "measured");

    const heading = document.createElement("p");
    heading.textContent = "Measured runs: " + measured.length +
      "; hardware samples: " + String(event.hardware_sample_count ?? "unknown");
    results.appendChild(heading);

    for (const run of measured) {
      const line = document.createElement("p");
      const tokens = run.eval_count;
      const duration = run.eval_duration_ns;
      const rate = typeof tokens === "number" &&
        typeof duration === "number" && duration > 0
        ? tokens * 1e9 / duration : null;

      line.textContent =
        "Generated tokens: " + String(tokens ?? "unavailable") +
        "; generation speed: " + formatNumber(rate, 2) + " tokens/s";
      results.appendChild(line);
    }
  }

  async function refresh() {
    try {
      const response = await fetch("/api/benchmark/status", {
        cache: "no-store"
      });
      if (!response.ok) throw new Error("Status HTTP " + response.status);

      const snapshot = await response.json();
      running = snapshot.running === true;
      status.textContent = "Benchmark state: " + String(snapshot.state);

      if (Array.isArray(snapshot.events)) {
        const samples = snapshot.events.filter(
          (event) => event.event === "hardware_sample"
        );
        if (samples.length) {
          displayTelemetry(samples[samples.length - 1]);
        }

        const completed = snapshot.events.find(
          (event) => event.event === "completed"
        );
        if (completed) displayResults(completed);

        const failed = snapshot.events.find(
          (event) => event.event === "failed"
        );
        if (failed) {
          status.textContent = "Benchmark failed: " +
            String(failed.message ?? "unknown error");
        }
      }
    } catch (error) {
      running = false;
      status.textContent = "Status unavailable: " + String(error.message);
      start.disabled = true;
      cancel.disabled = true;
      return;
    }
    updateButtons();
  }

  async function post(path, body) {
    busy = true;
    updateButtons();

    try {
      const headers = { "X-DAIA-CSRF-Token": csrf.content };
      if (body !== null) headers["Content-Type"] = "application/json";

      const options = { method: "POST", headers };
      if (body !== null) options.body = JSON.stringify(body);

      const response = await fetch(path, options);
      if (!response.ok) {
        throw new Error("HTTP " + response.status);
      }
      await refresh();
    } catch (error) {
      status.textContent = "Request failed: " + String(error.message);
    } finally {
      busy = false;
      updateButtons();
    }
  }

  model.addEventListener("input", updateButtons);
  start.addEventListener("click", () => {
    const name = model.value.trim();
    if (name && !running && !busy) {
      post("/api/benchmark/start", { model_name: name });
    }
  });
  cancel.addEventListener("click", () => {
    if (running && !busy) post("/api/benchmark/cancel", null);
  });

  updateButtons();
  refresh();
  setInterval(refresh, 1000);
})();
