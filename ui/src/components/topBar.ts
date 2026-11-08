import { status } from "../api";
import { icons } from "../icons";

export interface TopBarController {
  element: HTMLElement;
  setWorkspaceName: (name: string) => void;
  updateStatus: () => Promise<void>;
}

export function createTopBar(
  container: HTMLElement,
  initialWorkspaceName: string,
  onOpenPalette: () => void,
  onToggleTheme: () => void
): TopBarController {
  container.className = "top-bar";

  let workspaceName = initialWorkspaceName;

  function render() {
    container.innerHTML = `
      <div class="top-bar-left">
        <span class="brand">NexHub</span>
        <span class="workspace-name" id="topbar-ws-name">${workspaceName}</span>
      </div>
      <div class="top-bar-center">
        <div class="search-box" id="topbar-search-box">
          <span class="search-icon">${icons.search}</span>
          <input class="search-input" type="text" placeholder="Search agents, notes, commands... (Ctrl+K)" readonly />
          <kbd class="search-kbd">Ctrl+K</kbd>
        </div>
      </div>
      <div class="top-bar-right">
        <button class="topbar-btn theme-toggle-btn" title="Toggle theme (Dark/Light)" id="topbar-theme-btn">
          ${icons.sun}
        </button>
        <div class="status-badge" id="app-status">status: loading...</div>
      </div>
    `;

    container.querySelector("#topbar-search-box")?.addEventListener("click", () => {
      onOpenPalette();
    });

    container.querySelector("#topbar-theme-btn")?.addEventListener("click", () => {
      onToggleTheme();
    });
  }

  async function updateStatus() {
    try {
      const res = await status();
      const statusEl = container.querySelector("#app-status");
      if (statusEl) {
        statusEl.textContent = `status: ${res.agents_running} running`;
      }
    } catch {
      const statusEl = container.querySelector("#app-status");
      if (statusEl) {
        statusEl.textContent = "status: offline";
      }
    }
  }

  render();
  updateStatus();

  return {
    element: container,
    setWorkspaceName: (name: string) => {
      workspaceName = name;
      const wsEl = container.querySelector("#topbar-ws-name");
      if (wsEl) wsEl.textContent = name;
    },
    updateStatus,
  };
}
