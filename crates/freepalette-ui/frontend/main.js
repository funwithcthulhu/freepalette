const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const searchInput = document.querySelector("#search");
const resultsList = document.querySelector("#results");
const statusLine = document.querySelector("#status");

let palette = {
  query: "",
  results: [],
  selectedIndex: null,
  status: { state: "ready" },
};

searchInput.addEventListener("input", async () => {
  await callPalette("search_palette", { query: searchInput.value });
});

searchInput.addEventListener("keydown", async (event) => {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    await callPalette("move_selection", { direction: "next" });
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    await callPalette("move_selection", { direction: "previous" });
  } else if (event.key === "Enter") {
    event.preventDefault();
    const response = await invoke("execute_selected");
    palette = response.palette;
    render();
    if (
      response.execution.state === "completed" &&
      response.execution.hide_palette
    ) {
      await invoke("close_palette_window");
    }
  } else if (event.key === "Escape") {
    event.preventDefault();
    await invoke("close_palette_window");
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
    render();
  } catch (error) {
    statusLine.textContent = String(error);
    statusLine.className = "status error";
  }
}

function render() {
  searchInput.value = palette.query;
  resultsList.replaceChildren(...resultElements(palette.results));
  renderStatus(palette.status);
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
    const score = document.createElement("span");
    score.className = "score";
    score.textContent = ranked.score;
    meta.append(provider, score);

    item.append(content, meta);
    return item;
  });
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
    return `shell command blocked: ${action.command}`;
  }
  if (action.type === "copy-text") {
    return "copy text";
  }
  if (action.type === "noop") {
    return action.message;
  }

  return action.type;
}
