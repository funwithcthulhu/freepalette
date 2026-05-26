const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const searchInput = document.querySelector("#search");
const resultsList = document.querySelector("#results");
const statusLine = document.querySelector("#status");
const shellConfirmation = document.querySelector("#shell-confirmation");
const shellCommand = document.querySelector("#shell-command");
const confirmShellButton = document.querySelector("#confirm-shell");
const cancelShellButton = document.querySelector("#cancel-shell");

let palette = {
  query: "",
  results: [],
  selectedIndex: null,
  status: { state: "ready" },
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

document.addEventListener("keydown", async (event) => {
  if (pendingShellCommand && event.key === "Escape") {
    event.preventDefault();
    await cancelPendingShellCommand();
  }
});

document.addEventListener("DOMContentLoaded", async () => {
  await listen("palette-updated", async () => {
    await callPalette("palette_snapshot");
  });
  await listen("palette-shown", async () => {
    await callPalette("palette_snapshot");
    focusSearch();
  });
  await callPalette("palette_snapshot");
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

    const content = document.createElement("div");
    const title = document.createElement("p");
    title.className = "title";
    title.textContent = result.title;
    const subtitle = document.createElement("div");
    subtitle.className = "subtitle";
    subtitle.textContent = result.subtitle || result.provider;
    const action = document.createElement("div");
    action.className = "action";
    action.textContent = describeAction(result.action);
    content.append(title, subtitle, action);

    const meta = document.createElement("div");
    meta.className = "meta";
    const provider = document.createElement("span");
    provider.textContent = result.provider;
    meta.append(provider);

    item.append(content, meta);
    return item;
  });
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
