import { icons } from "../icons";

export interface CommandItem {
  id: string;
  title: string;
  description: string;
  shortcut?: string;
  icon?: string;
  action: () => void;
}

export function openCommandPalette(commands: CommandItem[]): void {
  const existing = document.getElementById("command-palette-modal");
  if (existing) {
    existing.remove();
  }

  const overlay = document.createElement("div");
  overlay.id = "command-palette-modal";
  overlay.className = "palette-overlay";

  overlay.innerHTML = `
    <div class="palette-card">
      <div class="palette-input-wrapper">
        <span class="palette-search-icon">${icons.search}</span>
        <input class="palette-input" type="text" placeholder="Type a command or search cards..." autofocus />
        <kbd class="palette-esc-kbd">ESC</kbd>
      </div>
      <div class="palette-list" id="palette-list"></div>
    </div>
  `;

  document.body.appendChild(overlay);

  const input = overlay.querySelector(".palette-input") as HTMLInputElement;
  const listEl = overlay.querySelector("#palette-list") as HTMLElement;

  let selectedIndex = 0;
  let filteredCommands = [...commands];

  function renderList() {
    listEl.innerHTML = "";
    if (filteredCommands.length === 0) {
      listEl.innerHTML = `<div class="palette-empty">No matching commands</div>`;
      return;
    }

    filteredCommands.forEach((cmd, i) => {
      const item = document.createElement("div");
      item.className = `palette-item ${i === selectedIndex ? "selected" : ""}`;
      item.innerHTML = `
        <div class="palette-item-main">
          <span class="palette-item-icon">${cmd.icon || icons.terminal}</span>
          <div class="palette-item-text">
            <span class="palette-item-title">${cmd.title}</span>
            <span class="palette-item-desc">${cmd.description}</span>
          </div>
        </div>
        ${cmd.shortcut ? `<kbd class="palette-shortcut">${cmd.shortcut}</kbd>` : ""}
      `;

      item.addEventListener("click", () => {
        close();
        cmd.action();
      });

      listEl.appendChild(item);
    });

    const activeItem = listEl.querySelector(".palette-item.selected");
    activeItem?.scrollIntoView({ block: "nearest" });
  }

  function close() {
    overlay.remove();
  }

  input.addEventListener("input", () => {
    const query = input.value.toLowerCase().trim();
    if (!query) {
      filteredCommands = [...commands];
    } else {
      filteredCommands = commands.filter(
        (c) =>
          c.title.toLowerCase().includes(query) ||
          c.description.toLowerCase().includes(query)
      );
    }
    selectedIndex = 0;
    renderList();
  });

  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % Math.max(1, filteredCommands.length);
      renderList();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex =
        (selectedIndex - 1 + filteredCommands.length) % Math.max(1, filteredCommands.length);
      renderList();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filteredCommands[selectedIndex]) {
        close();
        filteredCommands[selectedIndex].action();
      }
    } else if (e.key === "Escape") {
      close();
    }
  });

  overlay.addEventListener("mousedown", (e) => {
    if (e.target === overlay) {
      close();
    }
  });

  input.focus();
  renderList();
}
