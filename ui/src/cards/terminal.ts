import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { agentsSpawn, agentsStop, msgNotify, onAgentState, onPtyOutput, ptyResize, ptyWrite } from "../api";
import { icons } from "../icons";
import type { Agent, AgentState } from "../types";

export function stringToBase64(str: string): string {
  if (typeof TextEncoder !== "undefined") {
    const bytes = new TextEncoder().encode(str);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }
  return Buffer.from(str, "utf-8").toString("base64");
}

export function base64ToString(b64: string): string {
  if (typeof atob !== "undefined" && typeof TextDecoder !== "undefined") {
    const binary = atob(b64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return new TextDecoder().decode(bytes);
  }
  return Buffer.from(b64, "base64").toString("utf-8");
}

export interface TerminalCardOptions {
  cardId: string;
  agent: Agent;
  isMaximized?: boolean;
  onToggleMaximize?: () => void;
  onClose?: () => void;
}

export interface TerminalCardHandle {
  element: HTMLElement;
  fit: () => void;
  dispose: () => void;
  writeOutput: (text: string) => void;
  updateState: (state: AgentState, quotaResetAt?: number | null) => void;
}

export function createTerminalCard(options: TerminalCardOptions): TerminalCardHandle {
  const { cardId, agent, onToggleMaximize } = options;

  let currentAgent = { ...agent };
  let isMaximized = Boolean(options.isMaximized);

  const container = document.createElement("div");
  container.className = "card-inner terminal-card";
  container.dataset.cardId = cardId;
  container.dataset.agentId = currentAgent.id;

  const header = document.createElement("div");
  header.className = "card-header";

  const terminalBody = document.createElement("div");
  terminalBody.className = "terminal-body";

  container.appendChild(header);
  container.appendChild(terminalBody);

  // Initialize xterm
  const term = new Terminal({
    cursorBlink: true,
    fontFamily: '"Cascadia Mono", "JetBrains Mono", monospace',
    fontSize: 13,
    scrollback: 10000,
    theme: {
      background: "#0f1216",
      foreground: "#e6e9ee",
      cursor: "#2bb3a3",
      selectionBackground: "#2a313b",
    },
  });

  const fitAddon = new FitAddon();
  term.loadAddon(fitAddon);

  function renderHeader() {
    const quotaText =
      currentAgent.state === "waiting_quota" && currentAgent.quota_reset_at
        ? ` (${new Date(currentAgent.quota_reset_at).toLocaleTimeString()})`
        : "";

    header.innerHTML = `
      <div class="card-header-left">
        <span class="card-icon">${icons.terminal}</span>
        <span class="agent-name" title="${currentAgent.name}">${currentAgent.name}</span>
        <span class="badge preset-badge">${currentAgent.preset}</span>
        ${currentAgent.role ? `<span class="badge role-badge">${currentAgent.role}</span>` : ""}
        ${currentAgent.floor_id ? `<span class="badge floor-badge">${currentAgent.floor_id.replace(/^fl_/, "")}</span>` : ""}
        <span class="state-pill state-${currentAgent.state}">
          <span class="state-dot"></span>
          <span class="state-label">${currentAgent.state}${quotaText}</span>
        </span>
      </div>
      <div class="card-header-actions">
        <button class="card-action-btn send-msg-btn" title="Send message">${icons.messageSquare}</button>
        <button class="card-action-btn restart-btn" title="Restart">${icons.refresh}</button>
        <button class="card-action-btn stop-btn" title="Stop">${icons.stop}</button>
        <button class="card-action-btn maximize-btn" title="${isMaximized ? "Restore (Esc)" : "Maximize"}">
          ${isMaximized ? icons.minimize : icons.maximize}
        </button>
      </div>
    `;

    // Hook buttons
    header.querySelector(".restart-btn")?.addEventListener("click", async (e) => {
      e.stopPropagation();
      try {
        term.write("\r\n\x1b[33m[Restarting agent...]\x1b[0m\r\n");
        await agentsSpawn({
          workspace: currentAgent.workspace_id,
          name: currentAgent.name,
          preset: currentAgent.preset,
          role: currentAgent.role || undefined,
          floor: currentAgent.floor_id || undefined,
        });
      } catch (err) {
        term.write(`\r\n\x1b[31m[Restart failed: ${String(err)}]\x1b[0m\r\n`);
      }
    });

    header.querySelector(".stop-btn")?.addEventListener("click", async (e) => {
      e.stopPropagation();
      try {
        await agentsStop(currentAgent.name);
        term.write("\r\n\x1b[33m[Agent stopped]\x1b[0m\r\n");
      } catch (err) {
        term.write(`\r\n\x1b[31m[Stop failed: ${String(err)}]\x1b[0m\r\n`);
      }
    });

    header.querySelector(".send-msg-btn")?.addEventListener("click", (e) => {
      e.stopPropagation();
      openMessageComposer(currentAgent.name);
    });

    header.querySelector(".maximize-btn")?.addEventListener("click", (e) => {
      e.stopPropagation();
      isMaximized = !isMaximized;
      if (onToggleMaximize) {
        onToggleMaximize();
      }
      renderHeader();
      setTimeout(fit, 100);
    });
  }

  function openMessageComposer(targetAgent: string) {
    const existing = document.getElementById("terminal-composer-modal");
    if (existing) {
      existing.remove();
    }

    const modal = document.createElement("div");
    modal.id = "terminal-composer-modal";
    modal.className = "modal-overlay";
    modal.innerHTML = `
      <div class="modal-card">
        <div class="modal-header">
          <span>Send message to ${targetAgent}</span>
          <button class="modal-close-btn">&times;</button>
        </div>
        <div class="modal-body">
          <textarea class="modal-textarea" placeholder="Type prompt or instruction to agent..."></textarea>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary modal-cancel-btn">Cancel</button>
          <button class="btn btn-primary modal-send-btn">Send</button>
        </div>
      </div>
    `;

    document.body.appendChild(modal);

    const textarea = modal.querySelector(".modal-textarea") as HTMLTextAreaElement;
    textarea?.focus();

    const closeModal = () => modal.remove();
    modal.querySelector(".modal-close-btn")?.addEventListener("click", closeModal);
    modal.querySelector(".modal-cancel-btn")?.addEventListener("click", closeModal);

    modal.querySelector(".modal-send-btn")?.addEventListener("click", async () => {
      const body = textarea.value.trim();
      if (!body) return;
      try {
        await msgNotify(targetAgent, body);
        term.write(`\r\n\x1b[36m[Message sent to ${targetAgent}: "${body}"]\x1b[0m\r\n`);
        closeModal();
      } catch (err) {
        alert(`Failed to send message: ${String(err)}`);
      }
    });
  }

  renderHeader();

  // Mount terminal after container is connected to DOM
  setTimeout(() => {
    if (terminalBody && !terminalBody.hasChildNodes()) {
      term.open(terminalBody);
      fit();

      // Output initial replay message
      term.writeln(`\x1b[1;36mNexHub PTY connected for ${currentAgent.name} (${currentAgent.preset})\x1b[0m`);
      term.writeln(`\x1b[90mWorking directory: ${currentAgent.cwd}\x1b[0m`);
      if (currentAgent.last_prompt) {
        term.writeln(`\x1b[33mLast prompt: ${currentAgent.last_prompt}\x1b[0m`);
      }
      term.write("$ ");
    }
  }, 0);

  // Hook terminal input -> ptyWrite
  term.onData((data) => {
    const b64 = stringToBase64(data);
    ptyWrite(currentAgent.id, b64).catch((err) => {
      console.error("ptyWrite error", err);
    });
  });

  // Event unlisteners
  const unlistenPromises: Promise<() => void>[] = [];

  unlistenPromises.push(
    onPtyOutput((payload) => {
      if (payload.agent_id === currentAgent.id) {
        try {
          const str = base64ToString(payload.data_b64);
          term.write(str);
        } catch (err) {
          console.error("Failed to decode PTY output", err);
        }
      }
    })
  );

  unlistenPromises.push(
    onAgentState((payload) => {
      if (payload.agent_id === currentAgent.id) {
        currentAgent.state = payload.state;
        if (payload.quota_reset_at !== undefined) {
          currentAgent.quota_reset_at = payload.quota_reset_at;
        }
        renderHeader();
      }
    })
  );

  function fit() {
    try {
      fitAddon.fit();
      if (term.cols && term.rows) {
        ptyResize(currentAgent.id, term.cols, term.rows).catch(() => {});
      }
    } catch {
      // Container might not be visible yet
    }
  }

  return {
    element: container,
    fit,
    writeOutput: (text: string) => {
      term.write(text);
    },
    updateState: (newState: AgentState, quotaResetAt?: number | null) => {
      currentAgent.state = newState;
      if (quotaResetAt !== undefined) {
        currentAgent.quota_reset_at = quotaResetAt;
      }
      renderHeader();
    },
    dispose: () => {
      term.dispose();
      unlistenPromises.forEach((p) => p.then((unlisten) => unlisten()));
    },
  };
}
