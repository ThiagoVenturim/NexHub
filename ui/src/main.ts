import { agentsSpawn, workspacesList } from "./api";
import { createCanvas, type CanvasController } from "./canvas/canvas";
import { openCommandPalette, type CommandItem } from "./components/commandPalette";
import { createMessageLog, type MessageLogController } from "./components/messageLog";
import { createTopBar, type TopBarController } from "./components/topBar";
import { startFirstRunWizard } from "./components/wizard";
import { clear, el } from "./dom";
import { icons } from "./icons";
import { createSidebar, type SidebarController } from "./sidebar/sidebar";
import "./styles/tokens.css";
import type { Workspace } from "./types";

let currentWorkspace: Workspace | null = null;
let topBar: TopBarController | null = null;
let sidebar: SidebarController | null = null;
let canvas: CanvasController | null = null;
let messageLog: MessageLogController | null = null;

function initTheme(): () => void {
  const saved = localStorage.getItem("nexhub_theme");
  if (saved === "light") {
    document.documentElement.setAttribute("data-theme", "light");
  } else {
    document.documentElement.removeAttribute("data-theme");
  }

  return function toggleTheme() {
    const isLight = document.documentElement.getAttribute("data-theme") === "light";
    if (isLight) {
      document.documentElement.removeAttribute("data-theme");
      localStorage.setItem("nexhub_theme", "dark");
    } else {
      document.documentElement.setAttribute("data-theme", "light");
      localStorage.setItem("nexhub_theme", "light");
    }
  };
}

async function renderApp() {
  const app = document.getElementById("app");
  if (!app) return;

  const toggleTheme = initTheme();

  // Check workspaces
  const wsRes = await workspacesList();
  if (wsRes.workspaces.length === 0) {
    // Show first-run wizard
    startFirstRunWizard({
      container: app,
      onComplete: (newWs) => {
        currentWorkspace = newWs;
        mountWorkspace(newWs, toggleTheme);
      },
    });
    return;
  }

  currentWorkspace = wsRes.workspaces[0];
  mountWorkspace(currentWorkspace, toggleTheme);
}

function mountWorkspace(workspace: Workspace, toggleTheme: () => void) {
  const app = document.getElementById("app");
  if (!app) return;

  clear(app);

  const topbarEl = el("header", { class: "top-bar", id: "app-topbar" });
  const sidebarEl = el("aside", { class: "sidebar", id: "app-sidebar" });
  const canvasEl = el("main", { class: "canvas", id: "app-canvas" });
  const mainContent = el("div", { class: "main-content" }, sidebarEl, canvasEl);
  const messageLogEl = el("footer", { class: "message-log collapsed", id: "app-messagelog" });

  app.appendChild(topbarEl);
  app.appendChild(mainContent);
  app.appendChild(messageLogEl);

  topBar = createTopBar(
    topbarEl,
    workspace.name,
    () => triggerCommandPalette(toggleTheme),
    toggleTheme
  );

  canvas = createCanvas(canvasEl, workspace.id);
  canvas.loadLayout(workspace.layout_json);

  sidebar = createSidebar(
    sidebarEl,
    workspace.id,
    (selectedWs) => {
      currentWorkspace = selectedWs;
      topBar?.setWorkspaceName(selectedWs.name);
      canvas?.dispose();
      canvas = createCanvas(canvasEl, selectedWs.id);
      canvas.loadLayout(selectedWs.layout_json);
      topBar?.updateStatus();
      messageLog?.refresh();
    },
    () => {
      triggerNewFloor();
    }
  );

  messageLog = createMessageLog(messageLogEl);

  // Global Keyboard shortcuts
  window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && (e.key === "k" || e.key === "K")) {
      e.preventDefault();
      triggerCommandPalette(toggleTheme);
    } else if ((e.ctrlKey || e.metaKey) && e.key === "0") {
      e.preventDefault();
      canvas?.fitAllCards();
    } else if ((e.ctrlKey || e.metaKey) && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      const idx = parseInt(e.key, 10) - 1;
      canvas?.focusCardByIndex(idx);
    } else if (e.key === "Escape") {
      if (canvas?.hasMaximizedCard()) {
        e.preventDefault();
        canvas.leaveMaximized();
      }
    }
  });
}

function triggerCommandPalette(toggleTheme: () => void) {
  if (!currentWorkspace) return;

  const commands: CommandItem[] = [
    {
      id: "spawn-agent",
      title: "Spawn Agent...",
      description: "Spawn a new autonomous coding agent or shell",
      icon: icons.terminal,
      shortcut: "Ctrl+N",
      action: () => {
        const name = window.prompt("Agent name:", `agent_${Date.now().toString(36).substring(4)}`);
        if (!name) return;
        const preset = window.prompt("Preset (agy, claude, shell):", "agy") || "agy";
        const role = window.prompt("Role (e.g. builder, reviewer):", "builder") || "builder";
        agentsSpawn({
          workspace: currentWorkspace!.id,
          name,
          preset,
          role,
        }).then((res) => {
          canvas?.addAgentCard(res.agent);
          topBar?.updateStatus();
        }).catch((err) => alert(`Spawn failed: ${String(err)}`));
      },
    },
    {
      id: "new-note",
      title: "New Note Card",
      description: "Create an interactive markdown scratch note",
      icon: icons.note,
      action: () => {
        canvas?.addNoteCard("New note", "# Scratch Note\n\n- Write markdown here...");
      },
    },
    {
      id: "fit-all",
      title: "Fit All Cards",
      description: "Center and zoom canvas to enclose all cards",
      icon: icons.maximize,
      shortcut: "Ctrl+0",
      action: () => {
        canvas?.fitAllCards();
      },
    },
    {
      id: "new-floor",
      title: "New Floor...",
      description: "Create a git worktree and branch for a new floor",
      icon: icons.folder,
      action: () => {
        triggerNewFloor();
      },
    },
    {
      id: "toggle-theme",
      title: "Toggle Theme",
      description: "Switch between dark and light themes",
      icon: icons.sun,
      action: () => {
        toggleTheme();
      },
    },
  ];

  openCommandPalette(commands);
}

function triggerNewFloor() {
  const name = window.prompt("Floor name (e.g. semantic, core, bus):");
  if (!name || !name.trim()) return;
  sidebar?.refresh();
}

renderApp();
