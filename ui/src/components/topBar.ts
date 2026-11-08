import { status } from "../api";
import { clear, el } from "../dom";
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
  clear(container);
  container.className = "top-bar";

  let workspaceName = initialWorkspaceName;
  let statusBadgeEl: HTMLElement;
  let wsNameEl: HTMLElement;

  function render() {
    clear(container);

    wsNameEl = el("span", {
      class: "workspace-name",
      id: "topbar-ws-name",
      text: workspaceName,
    });

    const topBarLeft = el(
      "div",
      { class: "top-bar-left" },
      el("span", { class: "brand", text: "NexHub" }),
      wsNameEl
    );

    const searchInput = el("input", {
      class: "search-input",
      attrs: {
        type: "text",
        placeholder: "Search agents, notes, commands... (Ctrl+K)",
        readonly: "true",
      },
    });

    const searchBox = el(
      "div",
      {
        class: "search-box",
        id: "topbar-search-box",
        onClick: () => {
          onOpenPalette();
        },
      },
      el("span", { class: "search-icon", html: icons.search }),
      searchInput,
      el("kbd", { class: "search-kbd", text: "Ctrl+K" })
    );

    const topBarCenter = el("div", { class: "top-bar-center" }, searchBox);

    const themeBtn = el("button", {
      class: "topbar-btn theme-toggle-btn",
      id: "topbar-theme-btn",
      title: "Toggle theme (Dark/Light)",
      html: icons.sun,
      onClick: () => {
        onToggleTheme();
      },
    });

    statusBadgeEl = el("div", {
      class: "status-badge",
      id: "app-status",
      text: "status: loading...",
    });

    const topBarRight = el("div", { class: "top-bar-right" }, themeBtn, statusBadgeEl);

    container.appendChild(topBarLeft);
    container.appendChild(topBarCenter);
    container.appendChild(topBarRight);
  }

  async function updateStatus() {
    try {
      const res = await status();
      if (statusBadgeEl) {
        statusBadgeEl.textContent = `status: ${res.agents_running} running`;
      }
    } catch {
      if (statusBadgeEl) {
        statusBadgeEl.textContent = "status: offline";
      }
    }
  }

  render();
  updateStatus();

  return {
    element: container,
    setWorkspaceName: (name: string) => {
      workspaceName = name;
      if (wsNameEl) {
        wsNameEl.textContent = name;
      }
    },
    updateStatus,
  };
}
