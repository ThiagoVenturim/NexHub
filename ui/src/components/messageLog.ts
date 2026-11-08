import { msgInbox, onMessage } from "../api";
import { icons } from "../icons";
import type { Message } from "../types";

export interface MessageLogController {
  element: HTMLElement;
  refresh: () => Promise<void>;
  addMessage: (msg: Message) => void;
}

export function createMessageLog(container: HTMLElement): MessageLogController {
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
    container.innerHTML = `
      <div class="message-log-header" id="message-log-toggle">
        <div class="message-log-summary">
          <span class="message-log-icon">${icons.messageSquare}</span>
          <span class="message-log-title">Message log:</span>
          <span class="message-log-preview">${getLatestPreview()}</span>
        </div>
        <div class="message-log-controls">
          <span class="message-count-badge">${messages.length}</span>
          <button class="message-log-toggle-btn">${isCollapsed ? icons.chevronDown : icons.chevronRight}</button>
        </div>
      </div>
      <div class="message-log-content">
        <div class="message-log-table-wrapper">
          <table class="message-log-table">
            <thead>
              <tr>
                <th>Time</th>
                <th>Kind</th>
                <th>From</th>
                <th>To</th>
                <th>State</th>
                <th>Message</th>
              </tr>
            </thead>
            <tbody>
              ${
                messages.length === 0
                  ? `<tr><td colspan="6" class="message-empty">No messages in bus yet</td></tr>`
                  : messages
                      .slice()
                      .reverse()
                      .map(
                        (m) => `
                    <tr>
                      <td class="col-time">${formatTime(m.created_at)}</td>
                      <td><span class="kind-badge kind-${m.kind}">${m.kind}</span></td>
                      <td class="col-from">${m.from_agent || "human"}</td>
                      <td class="col-to">${m.to_agent || "broadcast"}</td>
                      <td><span class="state-badge state-${m.state}">${m.state}</span></td>
                      <td class="col-body">${m.body.replace(/</g, "&lt;").replace(/>/g, "&gt;")}</td>
                    </tr>
                  `
                      )
                      .join("")
              }
            </tbody>
          </table>
        </div>
      </div>
    `;

    container.className = `message-log ${isCollapsed ? "collapsed" : "expanded"}`;

    const toggle = container.querySelector("#message-log-toggle");
    toggle?.addEventListener("click", () => {
      isCollapsed = !isCollapsed;
      render();
    });
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
    // Avoid duplicates
    if (!messages.some((m) => m.id === msg.id)) {
      messages.push(msg);
      render();
    }
  }

  // Hook bus events
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
