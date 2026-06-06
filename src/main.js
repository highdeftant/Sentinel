const bytesFormatter = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 1,
});

const state = {
  snapshot: null,
  selectedKey: null,
  selectedListener: null,
  loadedDetailKey: null,
  refreshRequestId: 0,
  detailRequestId: 0,
  filters: {
    query: "",
    scope: "all",
    protocol: "all",
    managedOnly: false,
  },
};

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function listenerKey(listener) {
  return [
    listener.protocol,
    listener.local_address,
    listener.port,
    listener.pid ?? "none",
  ].join(":");
}

function formatBytes(bytes) {
  if (typeof bytes !== "number" || Number.isNaN(bytes)) {
    return "—";
  }

  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${bytesFormatter.format(value)} ${units[unitIndex]}`;
}

function formatSeconds(totalSeconds) {
  if (typeof totalSeconds !== "number" || Number.isNaN(totalSeconds)) {
    return "—";
  }

  const days = Math.floor(totalSeconds / 86400);
  const hours = Math.floor((totalSeconds % 86400) / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);

  const parts = [];
  if (days > 0) parts.push(`${days}d`);
  if (hours > 0 || days > 0) parts.push(`${hours}h`);
  parts.push(`${minutes}m`);
  return parts.join(" ");
}

function formatLoad(loadAverages) {
  if (!Array.isArray(loadAverages) || loadAverages.length !== 3) {
    return "—";
  }

  return loadAverages.map((value) => value.toFixed(2)).join(" / ");
}

function thermalClass(celsius) {
  if (celsius >= 80) return "danger";
  if (celsius >= 65) return "warning";
  return "ok";
}

function normalizedListenerSearch(listener) {
  return [
    listener.protocol,
    listener.port,
    listener.port_kind ?? "",
    listener.local_address,
    listener.scope,
    listener.exposure_severity ?? "",
    listener.process ?? "",
    listener.pid ?? "",
    listener.unit ?? "",
    listener.unit_scope ?? "",
  ]
    .join(" ")
    .toLowerCase();
}

function getFilteredListeners(listeners) {
  if (!Array.isArray(listeners)) {
    return [];
  }

  const { query, scope, protocol, managedOnly } = state.filters;
  const normalizedQuery = query.trim().toLowerCase();

  return listeners.filter((listener) => {
    if (scope !== "all" && listener.scope !== scope) {
      return false;
    }

    if (protocol !== "all" && listener.protocol !== protocol) {
      return false;
    }

    if (managedOnly && !listener.unit) {
      return false;
    }

    if (!normalizedQuery) {
      return true;
    }

    return normalizedListenerSearch(listener).includes(normalizedQuery);
  });
}

async function invoke(command, payload = {}) {
  const module = await import("@tauri-apps/api/core");
  return module.invoke(command, payload);
}

function renderSummary(summary) {
  const summaryGrid = document.querySelector("#summary-grid");
  const cards = [
    {
      label: "CPU usage",
      value: summary.cpu_usage_percent == null ? "warming up" : `${summary.cpu_usage_percent.toFixed(1)}%`,
    },
    {
      label: "Memory used",
      value: `${formatBytes(summary.memory_used_bytes)} / ${formatBytes(summary.memory_total_bytes)}`,
    },
    {
      label: "Memory available",
      value: formatBytes(summary.memory_available_bytes),
    },
    {
      label: "Load avg",
      value: formatLoad(summary.load_averages),
    },
    {
      label: "Uptime",
      value: formatSeconds(summary.uptime_seconds),
    },
    {
      label: "Listeners",
      value: `${summary.listener_count}`,
    },
  ];

  summaryGrid.innerHTML = cards
    .map(
      (card) => `
        <article class="metric-card">
          <span class="label">${escapeHtml(card.label)}</span>
          <strong>${escapeHtml(card.value)}</strong>
        </article>
      `,
    )
    .join("");
}

function renderTemperatures(temperatures) {
  const container = document.querySelector("#temperature-list");
  if (!Array.isArray(temperatures) || temperatures.length === 0) {
    container.innerHTML = '<p class="empty-state">No thermal sensors were readable.</p>';
    return;
  }

  container.innerHTML = temperatures
    .map(
      (reading) => `
        <article class="temperature-card ${thermalClass(reading.celsius)}">
          <span class="label">${escapeHtml(reading.label)}</span>
          <strong>${escapeHtml(`${reading.celsius.toFixed(1)}°C`)}</strong>
        </article>
      `,
    )
    .join("");
}

function renderListeners(listeners) {
  const tbody = document.querySelector("#listeners-body");
  const allListeners = Array.isArray(listeners) ? listeners : [];
  const filteredListeners = getFilteredListeners(allListeners);
  const dangerCount = filteredListeners.filter((listener) => listener.exposure_severity === "danger").length;
  const warningCount = filteredListeners.filter((listener) => listener.exposure_severity === "warning").length;
  const listenerCountLabel = [`${filteredListeners.length} / ${allListeners.length}`];

  if (dangerCount > 0) {
    listenerCountLabel.push(`${dangerCount} danger`);
  }

  if (warningCount > 0) {
    listenerCountLabel.push(`${warningCount} warning`);
  }

  document.querySelector("#listener-count").textContent = listenerCountLabel.join(" · ");

  if (allListeners.length === 0) {
    tbody.innerHTML = '<tr><td colspan="10" class="empty-row">No listeners detected.</td></tr>';
    return;
  }

  if (filteredListeners.length === 0) {
    tbody.innerHTML =
      '<tr><td colspan="10" class="empty-row">No listeners match the active filters.</td></tr>';
    return;
  }

  tbody.innerHTML = filteredListeners
    .map((listener) => {
      const key = listenerKey(listener);
      const selected = key === state.selectedKey ? "selected" : "";
      const processLabel = listener.process ?? "—";
      const pidLabel = listener.pid ?? "—";
      const unitLabel = listener.unit ?? "—";
      const unitScope = listener.unit_scope ?? "none";
      const portKind = listener.port_kind ?? "unknown";
      const severity = listener.exposure_severity ?? "unknown";

      return `
        <tr class="listener-row ${selected}" data-listener-key="${escapeHtml(key)}">
          <td>${escapeHtml(listener.protocol)}</td>
          <td>${escapeHtml(listener.port)}</td>
          <td><span class="chip ${escapeHtml(portKind)}">${escapeHtml(portKind)}</span></td>
          <td><code>${escapeHtml(listener.local_address)}</code></td>
          <td><span class="chip ${escapeHtml(listener.scope)}">${escapeHtml(listener.scope)}</span></td>
          <td><span class="chip ${escapeHtml(severity)}">${escapeHtml(severity)}</span></td>
          <td>${escapeHtml(processLabel)}</td>
          <td>${escapeHtml(pidLabel)}</td>
          <td>${escapeHtml(unitLabel)}</td>
          <td><span class="chip muted">${escapeHtml(unitScope)}</span></td>
        </tr>
      `;
    })
    .join("");

  tbody.querySelectorAll("tr[data-listener-key]").forEach((row) => {
    row.addEventListener("click", () => {
      const listener = filteredListeners.find((item) => listenerKey(item) === row.dataset.listenerKey);
      if (listener) {
        selectListener(listener, true);
      }
    });
  });
}

function renderWarnings(warnings) {
  const warningsList = document.querySelector("#warnings-list");
  if (!Array.isArray(warnings) || warnings.length === 0) {
    warningsList.innerHTML = '<li class="ok-line">No collector warnings.</li>';
    return;
  }

  warningsList.innerHTML = warnings
    .map((warning) => `<li>${escapeHtml(warning)}</li>`)
    .join("");
}

function setStatus(label, mode) {
  const status = document.querySelector("#summary-status");
  status.textContent = label;
  status.className = `chip ${mode}`;
}

function renderDetailPlaceholder(message) {
  document.querySelector("#detail-summary").innerHTML = `<p class="empty-state">${escapeHtml(message)}</p>`;
  document.querySelector("#detail-health").innerHTML = '<p class="empty-state">No probe loaded.</p>';
  document.querySelector("#detail-status").innerHTML = '<p class="empty-state">No status loaded.</p>';
  document.querySelector("#detail-logs").innerHTML = '<p class="empty-state">No logs loaded.</p>';
  document.querySelector("#detail-warnings").innerHTML = '<li class="empty-state">No detail warnings.</li>';
  document.querySelector("#detail-unit").textContent = "—";
  document.querySelector("#detail-pid").textContent = "—";
  state.loadedDetailKey = null;
}

function renderDetailLoading(listener) {
  const name = listener.process ?? listener.unit ?? `${listener.protocol}/${listener.port}`;
  document.querySelector("#detail-summary").innerHTML = `<p class="empty-state">Loading details for ${escapeHtml(name)}…</p>`;
  document.querySelector("#detail-health").innerHTML = '<p class="empty-state">Running active probe…</p>';
  document.querySelector("#detail-status").innerHTML = '<p class="empty-state">Loading systemctl status…</p>';
  document.querySelector("#detail-logs").innerHTML = '<p class="empty-state">Loading journal…</p>';
  document.querySelector("#detail-unit").textContent = listener.unit ?? "—";
  document.querySelector("#detail-pid").textContent = listener.pid ?? "—";
}

function renderServiceDetails(details) {
  const resolvedUnit = details.unit_state?.unit ?? details.resolved_unit ?? "unmanaged";
  document.querySelector("#detail-unit").textContent = resolvedUnit;
  document.querySelector("#detail-pid").textContent = `${details.pid}`;

  const summaryRows = [
    ["Process", details.process_name ?? "—"],
    ["Command", details.command_line ?? "—"],
    ["Bind", state.selectedListener?.local_address ?? "—"],
    ["Port", state.selectedListener?.port ?? "—"],
    ["Port class", state.selectedListener?.port_kind ?? "—"],
    ["Exposure", state.selectedListener?.exposure_severity ?? "—"],
    ["Scope", state.selectedListener?.scope ?? "—"],
    ["Cgroup", details.cgroup_path ?? "—"],
    ["Resolved unit", details.resolved_unit ?? "—"],
    ["Unit scope", details.resolved_unit_scope ?? "—"],
    ["Description", details.unit_state?.description ?? "—"],
    ["Load", details.unit_state?.load_state ?? "—"],
    ["Active", details.unit_state?.active_state ?? "—"],
    ["Sub", details.unit_state?.sub_state ?? "—"],
    ["Unit file", details.unit_state?.unit_file_state ?? "—"],
    ["Fragment", details.unit_state?.fragment_path ?? "—"],
  ];

  document.querySelector("#detail-summary").innerHTML = summaryRows
    .map(
      ([label, value]) => `
        <div class="detail-row">
          <span class="label">${escapeHtml(label)}</span>
          <span class="detail-value">${escapeHtml(value)}</span>
        </div>
      `,
    )
    .join("");

  if (Array.isArray(details.status_lines) && details.status_lines.length > 0) {
    document.querySelector("#detail-status").innerHTML = `
      <pre>${escapeHtml(details.status_lines.join("\n"))}</pre>
    `;
  } else {
    document.querySelector("#detail-status").innerHTML = '<p class="empty-state">No systemctl status output returned.</p>';
  }

  if (Array.isArray(details.recent_logs) && details.recent_logs.length > 0) {
    document.querySelector("#detail-logs").innerHTML = `
      <pre>${escapeHtml(details.recent_logs.join("\n"))}</pre>
    `;
  } else {
    document.querySelector("#detail-logs").innerHTML = '<p class="empty-state">No recent logs returned.</p>';
  }

  if (Array.isArray(details.warnings) && details.warnings.length > 0) {
    document.querySelector("#detail-warnings").innerHTML = details.warnings
      .map((warning) => `<li>${escapeHtml(warning)}</li>`)
      .join("");
  } else {
    document.querySelector("#detail-warnings").innerHTML = '<li class="ok-line">No detail warnings.</li>';
  }

  state.loadedDetailKey = state.selectedKey;
}

function renderServiceHealth(health) {
  if (!health || typeof health !== "object") {
    document.querySelector("#detail-health").innerHTML = '<p class="empty-state">No probe data returned.</p>';
    return;
  }

  const status = health.status ?? "unknown";
  const latency =
    typeof health.latency_ms === "number" && Number.isFinite(health.latency_ms)
      ? `${health.latency_ms}ms`
      : "—";
  const target = `${health.target ?? "—"}:${health.port ?? "—"}`;
  const warnings = Array.isArray(health.warnings) ? health.warnings : [];

  const warningMarkup =
    warnings.length > 0
      ? `<ul class="warning-list">${warnings.map((warning) => `<li>${escapeHtml(warning)}</li>`).join("")}</ul>`
      : '<span class="ok-line">No probe warnings.</span>';

  const healthRows = [
    ["Status", `<span class="chip ${escapeHtml(status)}">${escapeHtml(status)}</span>`],
    ["Probe", escapeHtml(health.check_kind ?? "—")],
    ["Target", escapeHtml(target)],
    ["Latency", escapeHtml(latency)],
    ["Message", escapeHtml(health.message ?? "—")],
    ["Warnings", warningMarkup],
  ];

  document.querySelector("#detail-health").innerHTML = healthRows
    .map(
      ([label, value]) => `
        <div class="detail-row">
          <span class="label">${escapeHtml(label)}</span>
          <span class="detail-value">${value}</span>
        </div>
      `,
    )
    .join("");
}

async function loadServiceDetails(listener) {
  const requestId = ++state.detailRequestId;

  if (!listener?.pid) {
    renderDetailPlaceholder("Selected listener has no PID metadata. Details unavailable.");
    return;
  }

  renderDetailLoading(listener);

  try {
    const [detailResult, healthResult] = await Promise.allSettled([
      invoke("service_details", {
        pid: listener.pid,
        processName: listener.process ?? null,
      }),
      invoke("service_health", {
        protocol: listener.protocol,
        localAddress: listener.local_address,
        port: listener.port,
      }),
    ]);

    if (requestId !== state.detailRequestId || state.selectedKey !== listenerKey(listener)) {
      return;
    }

    if (detailResult.status === "rejected") {
      const message = detailResult.reason instanceof Error ? detailResult.reason.message : String(detailResult.reason);
      renderDetailPlaceholder(`service_details failed: ${message}`);
      return;
    }

    renderServiceDetails(detailResult.value);

    if (healthResult.status === "fulfilled") {
      renderServiceHealth(healthResult.value);
      return;
    }

    const healthMessage =
      healthResult.reason instanceof Error ? healthResult.reason.message : String(healthResult.reason);
    renderServiceHealth({
      status: "warning",
      check_kind: "probe",
      target: listener.local_address,
      port: listener.port,
      latency_ms: null,
      message: `service_health failed: ${healthMessage}`,
      warnings: [],
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);

    if (requestId === state.detailRequestId && state.selectedKey === listenerKey(listener)) {
      renderDetailPlaceholder(`service_details failed: ${message}`);
    }
  }
}

function selectListener(listener, loadDetails = false) {
  const previousKey = state.selectedKey;
  state.selectedKey = listenerKey(listener);
  state.selectedListener = listener;

  if (previousKey !== state.selectedKey) {
    state.loadedDetailKey = null;
  }

  if (state.snapshot?.listeners) {
    renderListeners(state.snapshot.listeners);
  }

  document.querySelector("#detail-unit").textContent = listener.unit ?? "—";
  document.querySelector("#detail-pid").textContent = listener.pid ?? "—";

  if (loadDetails) {
    void loadServiceDetails(listener);
  }
}

function chooseSelectedListener(listeners) {
  const filteredListeners = getFilteredListeners(listeners);

  if (!Array.isArray(listeners) || listeners.length === 0) {
    state.selectedKey = null;
    state.selectedListener = null;
    state.loadedDetailKey = null;
    renderDetailPlaceholder("No listener selected.");
    return;
  }

  if (filteredListeners.length === 0) {
    state.selectedKey = null;
    state.selectedListener = null;
    state.loadedDetailKey = null;
    renderDetailPlaceholder("No listener matches the active filters.");
    return;
  }

  const retained = filteredListeners.find((listener) => listenerKey(listener) === state.selectedKey);
  if (!retained && state.selectedKey) {
    state.selectedListener = null;
    state.loadedDetailKey = null;
    document.querySelector("#detail-unit").textContent = "filtered out";
    document.querySelector("#detail-pid").textContent = "—";
    document.querySelector("#detail-summary").innerHTML =
      '<p class="empty-state">The selected listener is hidden by the active filters.</p>';
    document.querySelector("#detail-status").innerHTML =
      '<p class="empty-state">Clear or relax filters to reload detail.</p>';
    document.querySelector("#detail-logs").innerHTML =
      '<p class="empty-state">Clear or relax filters to reload detail.</p>';
    document.querySelector("#detail-warnings").innerHTML = '<li class="empty-state">No detail warnings.</li>';
    return;
  }

  const nextListener = retained ?? filteredListeners[0];
  const shouldLoadDetails = !retained || state.loadedDetailKey !== listenerKey(nextListener);
  selectListener(nextListener, shouldLoadDetails);
}

function updateFilterState() {
  state.filters.query = document.querySelector("#listener-search")?.value ?? "";
  state.filters.scope = document.querySelector("#scope-filter")?.value ?? "all";
  state.filters.protocol = document.querySelector("#protocol-filter")?.value ?? "all";
  state.filters.managedOnly = Boolean(document.querySelector("#managed-only-filter")?.checked);

  if (state.snapshot?.listeners) {
    renderListeners(state.snapshot.listeners);
    chooseSelectedListener(state.snapshot.listeners);
  }
}

async function refreshDashboard() {
  const requestId = ++state.refreshRequestId;
  setStatus("refreshing", "warning");

  try {
    const snapshot = await invoke("snapshot");

    if (requestId !== state.refreshRequestId) {
      return;
    }

    state.snapshot = snapshot;

    renderSummary(snapshot.summary);
    renderTemperatures(snapshot.temperatures);
    renderListeners(snapshot.listeners);
    renderWarnings(snapshot.warnings);
    chooseSelectedListener(snapshot.listeners);

    document.querySelector("#last-updated").textContent = new Date(
      snapshot.generated_at_unix_ms,
    ).toLocaleString();

    setStatus("live", "ok");
  } catch (error) {
    if (requestId !== state.refreshRequestId) {
      return;
    }

    const message = error instanceof Error ? error.message : String(error);
    renderWarnings([`snapshot command failed: ${message}`]);
    renderDetailPlaceholder(`snapshot failed: ${message}`);
    setStatus("error", "danger");
  }
}

document.querySelector("#refresh-button")?.addEventListener("click", () => {
  void refreshDashboard();
});

document.querySelector("#refresh-detail-button")?.addEventListener("click", () => {
  if (state.selectedListener) {
    void loadServiceDetails(state.selectedListener);
  }
});

document.querySelector("#listener-search")?.addEventListener("input", updateFilterState);
document.querySelector("#scope-filter")?.addEventListener("change", updateFilterState);
document.querySelector("#protocol-filter")?.addEventListener("change", updateFilterState);
document.querySelector("#managed-only-filter")?.addEventListener("change", updateFilterState);

renderDetailPlaceholder("Select a listener to inspect its systemd and journal context.");
void refreshDashboard();
