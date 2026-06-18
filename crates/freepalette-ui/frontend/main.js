const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const searchInput = document.querySelector("#search");
const resultsList = document.querySelector("#results");
const statusLine = document.querySelector("#status");
const shellConfirmation = document.querySelector("#shell-confirmation");
const shellCommand = document.querySelector("#shell-command");
const confirmShellButton = document.querySelector("#confirm-shell");
const cancelShellButton = document.querySelector("#cancel-shell");
const settingsToggle = document.querySelector("#settings-toggle");
const settingsPanel = document.querySelector("#settings-panel");
const settingsClose = document.querySelector("#settings-close");
const settingsProviders = document.querySelector("#settings-providers");
const clipboardCapture = document.querySelector("#clipboard-capture");
const settingsClipboardCount = document.querySelector("#settings-clipboard-count");
const settingsRecentCount = document.querySelector("#settings-recent-count");
const settingsHotkey = document.querySelector("#settings-hotkey");
const settingsStatePath = document.querySelector("#settings-state-path");
const settingsDaemonConnection = document.querySelector(
  "#settings-daemon-connection",
);
const clipboardRecordButton = document.querySelector("#clipboard-record");
const clipboardClearButton = document.querySelector("#clipboard-clear");
const settingsReloadButton = document.querySelector("#settings-reload");

let palette = {
  query: "",
  results: [],
  selectedIndex: null,
  status: { state: "ready" },
};
let settings = {
  providerIds: [],
  providers: {
    apps: false,
    calculator: false,
    shell: false,
    clipboard: false,
  },
  clipboardCaptureEnabled: false,
  clipboardHistoryLen: 0,
  recentResultCount: 0,
  hotkeySummary: "",
  localStatePath: null,
  daemonConnection: "",
};
let pendingShellCommand = null;

searchInput.addEventListener("input", async () => {
  pendingShellCommand = null;
  await callPalette("search_palette", { query: searchInput.value });
});

searchInput.addEventListener("keydown", async (event) => {
  if (pendingShellCommand && event.key === "Enter") {
    event.preventDefault();
    await confirmPendingShellCommand();
  } else if (pendingShellCommand && event.key === "Escape") {
    event.preventDefault();
    await cancelPendingShellCommand();
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    await callPalette("move_selection", { direction: "next" });
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    await callPalette("move_selection", { direction: "previous" });
  } else if (event.key === "Enter") {
    event.preventDefault();
    const response = await invoke("execute_selected");
    await handleExecutionResponse(response);
  } else if (event.key === "Escape") {
    event.preventDefault();
    await invoke("close_palette_window");
  }
});

confirmShellButton.addEventListener("click", async () => {
  await confirmPendingShellCommand();
});

cancelShellButton.addEventListener("click", async () => {
  await cancelPendingShellCommand();
});

settingsToggle.addEventListener("click", async () => {
  await showSettings();
});

settingsClose.addEventListener("click", () => {
  hideSettings();
});

settingsPanel.addEventListener("click", (event) => {
  if (event.target === settingsPanel) {
    hideSettings();
  }
});

clipboardRecordButton.addEventListener("click", async () => {
  await callPalette("record_current_clipboard");
  await refreshSettings();
});

clipboardClearButton.addEventListener("click", async () => {
  await callPalette("clear_clipboard_history");
  await refreshSettings();
});

settingsReloadButton.addEventListener("click", async () => {
  await callPalette("reload_config");
  await refreshSettings();
});

clipboardCapture.addEventListener("change", async () => {
  await callPalette("set_clipboard_capture", {
    enabled: clipboardCapture.checked,
  });
  await refreshSettings();
});

document.addEventListener("keydown", async (event) => {
  if (pendingShellCommand && event.key === "Escape") {
    event.preventDefault();
    await cancelPendingShellCommand();
  } else if (!settingsPanel.hidden && event.key === "Escape") {
    event.preventDefault();
    hideSettings();
  }
});

document.addEventListener("DOMContentLoaded", async () => {
  await listen("palette-updated", async () => {
    await callPalette("palette_snapshot");
  });
  await listen("palette-shown", async () => {
    await callPalette("palette_snapshot");
    hideSettings();
    focusSearch();
  });
  await callPalette("palette_snapshot");
  await refreshSettings();
  focusSearch();
});

async function callPalette(command, args = {}) {
  try {
    palette = await invoke(command, args);
    pendingShellCommand = null;
    render();
  } catch (error) {
    statusLine.textContent = String(error);
    statusLine.className = "status error";
  }
}

async function refreshSettings() {
  try {
    settings = await invoke("settings_snapshot");
    renderSettings();
  } catch (error) {
    statusLine.textContent = String(error);
    statusLine.className = "status error";
  }
}

async function handleExecutionResponse(response) {
  palette = response.palette;
  render();

  if (response.execution.state === "needs-shell-confirmation") {
    pendingShellCommand = response.execution.command;
    render();
    return;
  }

  pendingShellCommand = null;

  if (
    response.execution.state === "completed" &&
    response.execution.hide_palette
  ) {
    await invoke("close_palette_window");
  }

  await refreshSettings();
}

async function confirmPendingShellCommand() {
  if (!pendingShellCommand) {
    return;
  }

  pendingShellCommand = null;
  const confirmedResponse = await invoke("execute_confirmed_shell");
  await handleExecutionResponse(confirmedResponse);
}

async function cancelPendingShellCommand() {
  if (!pendingShellCommand) {
    return;
  }

  pendingShellCommand = null;
  await callPalette("cancel_shell_confirmation");
}

function render() {
  searchInput.value = palette.query;
  resultsList.replaceChildren(...resultElements(palette.results));
  renderStatus(palette.status);
  renderShellConfirmation();
  renderSettings();
}

async function showSettings() {
  await refreshSettings();
  settingsPanel.hidden = false;
  settingsClose.focus();
}

function hideSettings() {
  settingsPanel.hidden = true;
  focusSearch();
}

function focusSearch() {
  searchInput.focus();
  searchInput.select();
}

function resultElements(results) {
  if (results.length === 0) {
    const empty = document.createElement("li");
    empty.className = "empty";
    empty.textContent = palette.query.trim() ? "No results" : "";
    return [empty];
  }

  return results.map((ranked, index) => {
    const result = ranked.result;
    const item = document.createElement("li");
    item.className = index === palette.selectedIndex ? "result selected" : "result";
    item.addEventListener("click", async () => {
      pendingShellCommand = null;
      const direction = index < palette.selectedIndex ? "previous" : "next";
      while (palette.selectedIndex !== index) {
        await callPalette("move_selection", { direction });
      }
    });

    const kind = document.createElement("span");
    kind.className = `kind kind-${result.kind || "system"}`;
    kind.textContent = kindLabel(result.kind);

    const content = document.createElement("div");
    content.className = "result-content";
    const title = document.createElement("p");
    title.className = "title";
    title.textContent = result.title;
    const subtitle = document.createElement("div");
    subtitle.className = "subtitle";
    subtitle.textContent = result.subtitle || result.provider;
    const actionValue = primaryAction(result);
    const action = document.createElement("div");
    action.className = "action";
    action.textContent = describeAction(actionValue);
    content.append(title, subtitle, action);

    const meta = document.createElement("div");
    meta.className = "meta";
    const provider = document.createElement("span");
    provider.textContent = result.provider;
    const actionLabel = document.createElement("span");
    actionLabel.className = "action-label";
    actionLabel.textContent = primaryActionLabel(result);
    meta.append(provider, actionLabel);

    item.append(kind, content, meta);
    return item;
  });
}

function primaryActionDescriptor(result) {
  return result.actions?.find((descriptor) => descriptor.primary) || null;
}

function primaryAction(result) {
  return primaryActionDescriptor(result)?.action || result.action;
}

function primaryActionLabel(result) {
  return (
    primaryActionDescriptor(result)?.label ||
    fallbackActionLabel(primaryAction(result))
  );
}

function renderShellConfirmation() {
  if (!pendingShellCommand) {
    shellConfirmation.hidden = true;
    shellCommand.textContent = "";
    return;
  }

  shellCommand.textContent = pendingShellCommand;
  shellConfirmation.hidden = false;
  confirmShellButton.focus();
}

function renderStatus(status) {
  if (!status || status.state === "ready") {
    statusLine.textContent = "";
    statusLine.className = "status";
    return;
  }

  statusLine.textContent = status.message;
  statusLine.className = status.state === "error" ? "status error" : "status";
}

function renderSettings() {
  settingsProviders.replaceChildren(...providerToggleElements());
  clipboardCapture.checked = Boolean(settings.clipboardCaptureEnabled);
  settingsClipboardCount.textContent = String(settings.clipboardHistoryLen);
  settingsRecentCount.textContent = String(settings.recentResultCount);
  settingsHotkey.textContent = settings.hotkeySummary || "global hotkey unavailable";
  settingsStatePath.textContent = settings.localStatePath || "not available";
  settingsDaemonConnection.textContent =
    settings.daemonConnection || "in-process palette state";
}

function providerToggleElements() {
  const providers = ["apps", "calculator", "shell", "clipboard"];
  return providers.map((providerId) => {
    const label = document.createElement("label");
    label.className = "toggle-row";
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = Boolean(settings.providers?.[providerId]);
    input.addEventListener("change", async () => {
      await callPalette("set_provider_enabled", {
        providerId,
        enabled: input.checked,
      });
      await refreshSettings();
    });
    const text = document.createElement("span");
    text.textContent = providerId;
    label.append(input, text);
    return label;
  });
}

function fallbackActionLabel(action) {
  if (!action) {
    return "Action";
  }

  if (action.type === "launch-app") {
    return "Launch";
  }
  if (action.type === "open-path") {
    return "Open";
  }
  if (action.type === "run-shell") {
    return "Run";
  }
  if (action.type === "copy-text") {
    return "Copy";
  }
  if (action.type === "noop") {
    return "Show";
  }

  return "Action";
}

function kindLabel(kind) {
  if (kind === "app") {
    return "A";
  }
  if (kind === "calculator") {
    return "=";
  }
  if (kind === "shell") {
    return ">";
  }
  if (kind === "clipboard") {
    return "C";
  }
  if (kind === "plugin") {
    return "P";
  }

  return "S";
}

function describeAction(action) {
  if (!action) {
    return "";
  }

  if (action.type === "launch-app") {
    return action.args.length === 0
      ? `launch ${action.command}`
      : `launch ${action.command} ${action.args.join(" ")}`;
  }
  if (action.type === "open-path") {
    return `open ${action.path}`;
  }
  if (action.type === "run-shell") {
    return `shell command requires confirmation: ${action.command}`;
  }
  if (action.type === "copy-text") {
    return "copy text";
  }
  if (action.type === "noop") {
    return action.message;
  }

  return action.type;
}
