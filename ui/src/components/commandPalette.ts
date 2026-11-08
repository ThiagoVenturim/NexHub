import { clear, el } from "../dom";
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

  const input = el("input", {
    class: "palette-input",
    attrs: { type: "text", placeholder: "Type a command or search cards..." },
  });

  const inputWrapper = el(
    "div",
    { class: "palette-input-wrapper" },
    el("span", { class: "palette-search-icon", html: icons.search }),
    input,
    el("kbd", { class: "palette-esc-kbd", text: "ESC" })
  );

  const listEl = el("div", { class: "palette-list", id: "palette-list" });

  const card = el("div", { class: "palette-card" }, inputWrapper, listEl);

  const overlay = el(
    "div",
    {
      id: "command-palette-modal",
      class: "palette-overlay",
      onClick: (e) => {
        if (e.target === overlay) {
          close();
        }
      },
    },
    card
  );

  document.body.appendChild(overlay);

  let selectedIndex = 0;
  let filteredCommands = [...commands];

  function renderList() {
    clear(listEl);
    if (filteredCommands.length === 0) {
      listEl.appendChild(el("div", { class: "palette-empty", text: "No matching commands" }));
      return;
    }

    filteredCommands.forEach((cmd, i) => {
      const iconSpan = el("span", {
        class: "palette-item-icon",
        html: cmd.icon || icons.terminal,
      });

      const textContainer = el(
        "div",
        { class: "palette-item-text" },
        el("span", { class: "palette-item-title", text: cmd.title }),
        el("span", { class: "palette-item-desc", text: cmd.description })
      );

      const itemMain = el("div", { class: "palette-item-main" }, iconSpan, textContainer);

      const item = el(
        "div",
        {
          class: `palette-item ${i === selectedIndex ? "selected" : ""}`,
          onClick: () => {
            close();
            cmd.action();
          },
        },
        itemMain
      );

      if (cmd.shortcut) {
        item.appendChild(el("kbd", { class: "palette-shortcut", text: cmd.shortcut }));
      }

      listEl.appendChild(item);
    });

    const activeItem = listEl.querySelector(".palette-item.selected");
    if (activeItem && typeof activeItem.scrollIntoView === "function") {
      activeItem.scrollIntoView({ block: "nearest" });
    }
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

  input.focus();
  renderList();
}
