import { msgInbox, onMessage } from "../api";
import { clear, el, sanitizeMessageKind, sanitizeMessageState } from "../dom";
import { icons } from "../icons";
import type { Message } from "../types";

export interface MessageLogController {
  element: HTMLElement;
  refresh: () => Promise<void>;
  addMessage: (msg: Message) => void;
}

export function createMessageLog(container: HTMLElement): MessageLogController {
  clear(container);
  container.className = "message-log collapsed";
  container.id = "message-log";

  let messages: Message[] = [];
  let isCollapsed = true;

  function formatTime(ts: number): string {
    const d = new Date(ts);
    return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}:${d.getSeconds().toString().padStart(2, "0")}`;
  }

  function getLatestPreview(): string {
    if (messages.length === 0) {
      return "no messages";
    }
    const latest = messages[messages.length - 1];
    const from = latest.from_agent || "human";
    const to = latest.to_agent || "all";
    const snippet = latest.body.length > 50 ? `${latest.body.substring(0, 47)}...` : latest.body;
    return `${formatTime(latest.created_at)} ${from} -> ${to} [${latest.kind}]: "${snippet}"`;
  }

  function render() {
    clear(container);
    container.className = `message-log ${isCollapsed ? "collapsed" : "expanded"}`;

    const previewEl = el("span", {
      class: "message-log-preview",
      text: getLatestPreview(),
    });

    const summary = el(
      "div",
      { class: "message-log-summary" },
      el("span", { class: "message-log-icon", html: icons.messageSquare }),
      el("span", { class: "message-log-title", text: "Message log:" }),
      previewEl
    );

    const countBadge = el("span", {
      class: "message-count-badge",
      text: String(messages.length),
    });

    const toggleBtn = el("button", {
      class: "message-log-toggle-btn",
      html: isCollapsed ? icons.chevronDown : icons.chevronRight,
    });

    const controls = el("div", { class: "message-log-controls" }, countBadge, toggleBtn);

    const header = el(
      "div",
      {
        class: "message-log-header",
        id: "message-log-toggle",
        onClick: () => {
          isCollapsed = !isCollapsed;
          render();
        },
      },
      summary,
      controls
    );

    const tbody = el("tbody");

    if (messages.length === 0) {
      tbody.appendChild(
        el("tr", {}, el("td", { class: "message-empty", attrs: { colspan: "6" }, text: "No messages in bus yet" }))
      );
    } else {
      messages
        .slice()
        .reverse()
        .forEach((m) => {
          const safeKind = sanitizeMessageKind(m.kind);
          const safeState = sanitizeMessageState(m.state);

          const tr = el(
            "tr",
            {},
            el("td", { class: "col-time", text: formatTime(m.created_at) }),
            el("td", {}, el("span", { class: `kind-badge kind-${safeKind}`, text: m.kind })),
            el("td", { class: "col-from", text: m.from_agent || "human" }),
            el("td", { class: "col-to", text: m.to_agent || "broadcast" }),
            el("td", {}, el("span", { class: `state-badge state-${safeState}`, text: m.state })),
            el("td", { class: "col-body", text: m.body })
          );

          tbody.appendChild(tr);
        });
    }

    const thead = el(
      "thead",
      {},
      el(
        "tr",
        {},
        el("th", { text: "Time" }),
        el("th", { text: "Kind" }),
        el("th", { text: "From" }),
        el("th", { text: "To" }),
        el("th", { text: "State" }),
        el("th", { text: "Message" })
      )
    );

    const table = el("table", { class: "message-log-table" }, thead, tbody);
    const tableWrapper = el("div", { class: "message-log-table-wrapper" }, table);
    const content = el("div", { class: "message-log-content" }, tableWrapper);

    container.appendChild(header);
    container.appendChild(content);
  }

  async function refresh() {
    try {
      const res = await msgInbox();
      messages = res.messages;
      render();
    } catch {
      // Ignore
    }
  }

  function addMessage(msg: Message) {
    if (!messages.some((m) => m.id === msg.id)) {
      messages.push(msg);
      render();
    }
  }

  onMessage((msg) => {
    addMessage(msg);
  });

  refresh();

  return {
    element: container,
    refresh,
    addMessage,
  };
}
