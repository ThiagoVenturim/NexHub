import {
  floorsCreate,
  floorsList,
  floorsMerge,
  floorsRemove,
  groupsList,
  routinesAdd,
  routinesList,
  routinesRemove,
  workspacesList,
} from "../api";
import { icons } from "../icons";
import type { Floor, Group, Routine, Workspace } from "../types";

export interface SidebarController {
  element: HTMLElement;
  refresh: () => Promise<void>;
}

export function createSidebar(
  container: HTMLElement,
  currentWorkspaceId: string,
  onSelectWorkspace: (workspace: Workspace) => void,
  onNewFloorRequest?: () => void
): SidebarController {
  container.innerHTML = "";
  container.className = "sidebar";

  let groups: Group[] = [];
  let workspaces: Workspace[] = [];
  let floors: Floor[] = [];
  let routines: Routine[] = [];

  const collapsedGroups = new Set<string>();

  async function loadData() {
    try {
      const [groupsRes, workspacesRes, floorsRes, routinesRes] = await Promise.all([
        groupsList(),
        workspacesList(),
        floorsList(currentWorkspaceId),
        routinesList(),
      ]);

      groups = groupsRes.groups;
      workspaces = workspacesRes.workspaces;
      floors = floorsRes.floors;
      routines = routinesRes.routines;
      render();
    } catch (err) {
      console.error("Failed to load sidebar data:", err);
    }
  }

  function render() {
    container.innerHTML = "";

    // 1. Groups & Workspaces section
    const groupsSection = document.createElement("div");
    groupsSection.className = "sidebar-section";

    const groupsHeader = document.createElement("div");
    groupsHeader.className = "sidebar-section-header";
    groupsHeader.innerHTML = `
      <span class="sidebar-section-title">Workspaces</span>
      <button class="sidebar-add-btn add-workspace-btn" title="Add workspace">${icons.plus}</button>
    `;
    groupsSection.appendChild(groupsHeader);

    // Group items
    const groupsListEl = document.createElement("div");
    groupsListEl.className = "sidebar-groups-list";

    for (const group of groups) {
      const groupEl = document.createElement("div");
      groupEl.className = "sidebar-group";

      const isCollapsed = collapsedGroups.has(group.id);
      const groupHeader = document.createElement("div");
      groupHeader.className = "sidebar-group-header";
      groupHeader.innerHTML = `
        <span class="group-collapse-icon">${isCollapsed ? icons.chevronRight : icons.chevronDown}</span>
        <span class="group-name">${group.name}</span>
      `;

      groupHeader.addEventListener("click", () => {
        if (collapsedGroups.has(group.id)) {
          collapsedGroups.delete(group.id);
        } else {
          collapsedGroups.add(group.id);
        }
        render();
      });

      groupEl.appendChild(groupHeader);

      if (!isCollapsed) {
        const groupWorkspaces = workspaces.filter((w) => w.group_id === group.id);
        const wsList = document.createElement("ul");
        wsList.className = "sidebar-list";

        for (const ws of groupWorkspaces) {
          const wsItem = document.createElement("li");
          wsItem.className = `sidebar-item workspace-item ${ws.id === currentWorkspaceId ? "active" : ""}`;
          wsItem.innerHTML = `
            <span class="sidebar-item-icon">${icons.folder}</span>
            <span class="sidebar-item-label">${ws.name}</span>
          `;

          wsItem.addEventListener("click", () => {
            onSelectWorkspace(ws);
          });

          // Right click context menu
          wsItem.addEventListener("contextmenu", (e) => {
            e.preventDefault();
            openWorkspaceContextMenu(e.clientX, e.clientY, ws);
          });

          wsList.appendChild(wsItem);
        }

        groupEl.appendChild(wsList);
      }

      groupsListEl.appendChild(groupEl);
    }

    groupsSection.appendChild(groupsListEl);
    container.appendChild(groupsSection);

    // 2. Floors section
    const floorsSection = document.createElement("div");
    floorsSection.className = "sidebar-section";

    const floorsHeader = document.createElement("div");
    floorsHeader.className = "sidebar-section-header";
    floorsHeader.innerHTML = `
      <span class="sidebar-section-title">Floors</span>
      <button class="sidebar-add-btn add-floor-btn" title="New floor">${icons.plus}</button>
    `;

    floorsHeader.querySelector(".add-floor-btn")?.addEventListener("click", () => {
      if (onNewFloorRequest) {
        onNewFloorRequest();
      } else {
        openNewFloorPrompt();
      }
    });

    floorsSection.appendChild(floorsHeader);

    const floorsListEl = document.createElement("ul");
    floorsListEl.className = "sidebar-list";

    for (const fl of floors) {
      const flItem = document.createElement("li");
      flItem.className = "sidebar-item floor-item";
      flItem.innerHTML = `
        <span class="floor-status-dot status-${fl.status}"></span>
        <span class="sidebar-item-label">${fl.name}</span>
        <span class="floor-badge badge-${fl.status}">${fl.status}</span>
      `;

      flItem.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        openFloorContextMenu(e.clientX, e.clientY, fl);
      });

      floorsListEl.appendChild(flItem);
    }

    if (floors.length === 0) {
      const emptyEl = document.createElement("li");
      emptyEl.className = "sidebar-item-empty";
      emptyEl.textContent = "No floors active";
      floorsListEl.appendChild(emptyEl);
    }

    floorsSection.appendChild(floorsListEl);
    container.appendChild(floorsSection);

    // 3. Routines section
    const routinesSection = document.createElement("div");
    routinesSection.className = "sidebar-section";

    const routinesHeader = document.createElement("div");
    routinesHeader.className = "sidebar-section-header";
    routinesHeader.innerHTML = `
      <span class="sidebar-section-title">Routines</span>
      <button class="sidebar-add-btn add-routine-btn" title="Add routine">${icons.plus}</button>
    `;

    routinesHeader.querySelector(".add-routine-btn")?.addEventListener("click", () => {
      openNewRoutinePrompt();
    });

    routinesSection.appendChild(routinesHeader);

    const routinesListEl = document.createElement("ul");
    routinesListEl.className = "sidebar-list";

    for (const rt of routines) {
      const rtItem = document.createElement("li");
      rtItem.className = "sidebar-item routine-item";
      const nextRun = rt.next_run_at ? new Date(rt.next_run_at).toLocaleTimeString() : rt.schedule;

      rtItem.innerHTML = `
        <span class="routine-info">
          <span class="sidebar-item-label">${rt.name}</span>
          <span class="routine-meta">${nextRun}</span>
        </span>
        <label class="toggle-switch">
          <input type="checkbox" ${rt.enabled ? "checked" : ""} class="routine-toggle" />
          <span class="slider"></span>
        </label>
      `;

      const toggle = rtItem.querySelector(".routine-toggle") as HTMLInputElement;
      toggle?.addEventListener("change", (e) => {
        rt.enabled = (e.target as HTMLInputElement).checked ? 1 : 0;
      });

      rtItem.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        openRoutineContextMenu(e.clientX, e.clientY, rt);
      });

      routinesListEl.appendChild(rtItem);
    }

    if (routines.length === 0) {
      const emptyEl = document.createElement("li");
      emptyEl.className = "sidebar-item-empty";
      emptyEl.textContent = "No routines scheduled";
      routinesListEl.appendChild(emptyEl);
    }

    routinesSection.appendChild(routinesListEl);
    container.appendChild(routinesSection);
  }

  function openWorkspaceContextMenu(x: number, y: number, ws: Workspace) {
    const existing = document.getElementById("sidebar-context-menu");
    if (existing) existing.remove();

    const menu = document.createElement("div");
    menu.id = "sidebar-context-menu";
    menu.className = "context-menu";
    menu.style.left = `${x}px`;
    menu.style.top = `${y}px`;

    menu.innerHTML = `
      <div class="context-menu-item" data-action="rename">Rename workspace...</div>
      <div class="context-menu-item" data-action="move">Move to group...</div>
      <div class="context-menu-item danger" data-action="remove">Remove workspace</div>
    `;

    document.body.appendChild(menu);

    const closeMenu = () => menu.remove();
    setTimeout(() => {
      window.addEventListener("click", closeMenu, { once: true });
    }, 10);

    menu.addEventListener("click", (e) => {
      const action = (e.target as HTMLElement).dataset.action;
      if (!action) return;
      closeMenu();

      if (action === "rename") {
        const newName = window.prompt("New workspace name:", ws.name);
        if (newName && newName.trim()) {
          ws.name = newName.trim();
          render();
        }
      } else if (action === "move") {
        const groupName = window.prompt("Target group name (OSS, Pessoais):", "OSS");
        const found = groups.find((g) => g.name.toLowerCase() === groupName?.toLowerCase());
        if (found) {
          ws.group_id = found.id;
          render();
        }
      } else if (action === "remove") {
        if (window.confirm(`Remove workspace "${ws.name}" from NexHub? (Files will not be deleted)`)) {
          workspaces = workspaces.filter((w) => w.id !== ws.id);
          render();
        }
      }
    });
  }

  function openFloorContextMenu(x: number, y: number, fl: Floor) {
    const existing = document.getElementById("sidebar-context-menu");
    if (existing) existing.remove();

    const menu = document.createElement("div");
    menu.id = "sidebar-context-menu";
    menu.className = "context-menu";
    menu.style.left = `${x}px`;
    menu.style.top = `${y}px`;

    menu.innerHTML = `
      <div class="context-menu-item" data-action="merge">Merge into develop</div>
      <div class="context-menu-item danger" data-action="remove">Remove floor</div>
    `;

    document.body.appendChild(menu);

    const closeMenu = () => menu.remove();
    setTimeout(() => {
      window.addEventListener("click", closeMenu, { once: true });
    }, 10);

    menu.addEventListener("click", async (e) => {
      const action = (e.target as HTMLElement).dataset.action;
      if (!action) return;
      closeMenu();

      if (action === "merge") {
        try {
          await floorsMerge(currentWorkspaceId, fl.name, "develop");
          alert(`Floor "${fl.name}" merged successfully into develop.`);
          await loadData();
        } catch (err) {
          alert(`Merge failed: ${String(err)}`);
        }
      } else if (action === "remove") {
        if (window.confirm(`Remove floor "${fl.name}"?`)) {
          try {
            await floorsRemove(currentWorkspaceId, fl.name, false);
            await loadData();
          } catch (err) {
            alert(`Remove floor failed: ${String(err)}`);
          }
        }
      }
    });
  }

  function openRoutineContextMenu(x: number, y: number, rt: Routine) {
    const existing = document.getElementById("sidebar-context-menu");
    if (existing) existing.remove();

    const menu = document.createElement("div");
    menu.id = "sidebar-context-menu";
    menu.className = "context-menu";
    menu.style.left = `${x}px`;
    menu.style.top = `${y}px`;

    menu.innerHTML = `
      <div class="context-menu-item danger" data-action="delete">Delete routine</div>
    `;

    document.body.appendChild(menu);

    const closeMenu = () => menu.remove();
    setTimeout(() => {
      window.addEventListener("click", closeMenu, { once: true });
    }, 10);

    menu.addEventListener("click", async (e) => {
      const action = (e.target as HTMLElement).dataset.action;
      if (!action) return;
      closeMenu();

      if (action === "delete") {
        try {
          await routinesRemove(rt.name);
          await loadData();
        } catch (err) {
          alert(`Failed to remove routine: ${String(err)}`);
        }
      }
    });
  }

  function openNewFloorPrompt() {
    const name = window.prompt("New floor name (e.g. semantic, core, bus):");
    if (!name || !name.trim()) return;

    floorsCreate(currentWorkspaceId, name.trim(), "origin/develop")
      .then(() => loadData())
      .catch((err) => alert(`Failed to create floor: ${String(err)}`));
  }

  function openNewRoutinePrompt() {
    const name = window.prompt("Routine name (e.g. watchdog):");
    if (!name) return;
    const schedule = window.prompt("Schedule (e.g. every 15m, daily 09:00):", "every 15m") || "every 15m";
    const command = window.prompt("Command to run:", "git status") || "git status";

    routinesAdd({
      name,
      schedule,
      command,
      body: "",
    })
      .then(() => loadData())
      .catch((err) => alert(`Failed to add routine: ${String(err)}`));
  }

  loadData();

  return {
    element: container,
    refresh: loadData,
  };
}
