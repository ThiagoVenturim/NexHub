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
import { clear, el, sanitizeFloorStatus } from "../dom";
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
  clear(container);
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
    clear(container);

    // 1. Groups & Workspaces section
    const groupsHeader = el(
      "div",
      { class: "sidebar-section-header" },
      el("span", { class: "sidebar-section-title", text: "Workspaces" }),
      el("button", {
        class: "sidebar-add-btn add-workspace-btn",
        title: "Add workspace",
        html: icons.plus,
      })
    );

    const groupsListEl = el("div", { class: "sidebar-groups-list" });

    for (const group of groups) {
      const isCollapsed = collapsedGroups.has(group.id);
      const groupHeader = el(
        "div",
        { class: "sidebar-group-header" },
        el("span", {
          class: "group-collapse-icon",
          html: isCollapsed ? icons.chevronRight : icons.chevronDown,
        }),
        el("span", { class: "group-name", text: group.name })
      );

      groupHeader.addEventListener("click", () => {
        if (collapsedGroups.has(group.id)) {
          collapsedGroups.delete(group.id);
        } else {
          collapsedGroups.add(group.id);
        }
        render();
      });

      const groupEl = el("div", { class: "sidebar-group" }, groupHeader);

      if (!isCollapsed) {
        const groupWorkspaces = workspaces.filter((w) => w.group_id === group.id);
        const wsList = el("ul", { class: "sidebar-list" });

        for (const ws of groupWorkspaces) {
          const wsItem = el(
            "li",
            {
              class: `sidebar-item workspace-item ${ws.id === currentWorkspaceId ? "active" : ""}`,
              onClick: () => {
                onSelectWorkspace(ws);
              },
              onContextMenu: (e) => {
                e.preventDefault();
                openWorkspaceContextMenu(e.clientX, e.clientY, ws);
              },
            },
            el("span", { class: "sidebar-item-icon", html: icons.folder }),
            el("span", { class: "sidebar-item-label", text: ws.name })
          );

          wsList.appendChild(wsItem);
        }

        groupEl.appendChild(wsList);
      }

      groupsListEl.appendChild(groupEl);
    }

    const groupsSection = el(
      "div",
      { class: "sidebar-section" },
      groupsHeader,
      groupsListEl
    );
    container.appendChild(groupsSection);

    // 2. Floors section
    const addFloorBtn = el("button", {
      class: "sidebar-add-btn add-floor-btn",
      title: "New floor",
      html: icons.plus,
      onClick: () => {
        if (onNewFloorRequest) {
          onNewFloorRequest();
        } else {
          openNewFloorPrompt();
        }
      },
    });

    const floorsHeader = el(
      "div",
      { class: "sidebar-section-header" },
      el("span", { class: "sidebar-section-title", text: "Floors" }),
      addFloorBtn
    );

    const floorsListEl = el("ul", { class: "sidebar-list" });

    for (const fl of floors) {
      const safeStatus = sanitizeFloorStatus(fl.status);
      const flItem = el(
        "li",
        {
          class: "sidebar-item floor-item",
          onContextMenu: (e) => {
            e.preventDefault();
            openFloorContextMenu(e.clientX, e.clientY, fl);
          },
        },
        el("span", { class: `floor-status-dot status-${safeStatus}` }),
        el("span", { class: "sidebar-item-label", text: fl.name }),
        el("span", { class: `floor-badge badge-${safeStatus}`, text: fl.status })
      );

      floorsListEl.appendChild(flItem);
    }

    if (floors.length === 0) {
      floorsListEl.appendChild(
        el("li", { class: "sidebar-item-empty", text: "No floors active" })
      );
    }

    const floorsSection = el("div", { class: "sidebar-section" }, floorsHeader, floorsListEl);
    container.appendChild(floorsSection);

    // 3. Routines section
    const addRoutineBtn = el("button", {
      class: "sidebar-add-btn add-routine-btn",
      title: "Add routine",
      html: icons.plus,
      onClick: () => {
        openNewRoutinePrompt();
      },
    });

    const routinesHeader = el(
      "div",
      { class: "sidebar-section-header" },
      el("span", { class: "sidebar-section-title", text: "Routines" }),
      addRoutineBtn
    );

    const routinesListEl = el("ul", { class: "sidebar-list" });

    for (const rt of routines) {
      const nextRun = rt.next_run_at ? new Date(rt.next_run_at).toLocaleTimeString() : rt.schedule;

      const checkbox = el("input", {
        class: "routine-toggle",
        attrs: { type: "checkbox" },
      });
      checkbox.checked = Boolean(rt.enabled);
      checkbox.addEventListener("change", (e) => {
        rt.enabled = (e.target as HTMLInputElement).checked ? 1 : 0;
      });

      const toggleLabel = el(
        "label",
        { class: "toggle-switch" },
        checkbox,
        el("span", { class: "slider" })
      );

      const rtItem = el(
        "li",
        {
          class: "sidebar-item routine-item",
          onContextMenu: (e) => {
            e.preventDefault();
            openRoutineContextMenu(e.clientX, e.clientY, rt);
          },
        },
        el(
          "span",
          { class: "routine-info" },
          el("span", { class: "sidebar-item-label", text: rt.name }),
          el("span", { class: "routine-meta", text: nextRun })
        ),
        toggleLabel
      );

      routinesListEl.appendChild(rtItem);
    }

    if (routines.length === 0) {
      routinesListEl.appendChild(
        el("li", { class: "sidebar-item-empty", text: "No routines scheduled" })
      );
    }

    const routinesSection = el("div", { class: "sidebar-section" }, routinesHeader, routinesListEl);
    container.appendChild(routinesSection);
  }

  function openWorkspaceContextMenu(x: number, y: number, ws: Workspace) {
    const existing = document.getElementById("sidebar-context-menu");
    if (existing) existing.remove();

    const menu = el(
      "div",
      {
        id: "sidebar-context-menu",
        class: "context-menu",
        style: { left: `${x}px`, top: `${y}px` },
      },
      el("div", { class: "context-menu-item", dataset: { action: "rename" }, text: "Rename workspace..." }),
      el("div", { class: "context-menu-item", dataset: { action: "move" }, text: "Move to group..." }),
      el("div", { class: "context-menu-item danger", dataset: { action: "remove" }, text: "Remove workspace" })
    );

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

    const menu = el(
      "div",
      {
        id: "sidebar-context-menu",
        class: "context-menu",
        style: { left: `${x}px`, top: `${y}px` },
      },
      el("div", { class: "context-menu-item", dataset: { action: "merge" }, text: "Merge into develop" }),
      el("div", { class: "context-menu-item danger", dataset: { action: "remove" }, text: "Remove floor" })
    );

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

    const menu = el(
      "div",
      {
        id: "sidebar-context-menu",
        class: "context-menu",
        style: { left: `${x}px`, top: `${y}px` },
      },
      el("div", { class: "context-menu-item danger", dataset: { action: "delete" }, text: "Delete routine" })
    );

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
