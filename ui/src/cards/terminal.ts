import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { agentsSpawn, agentsStop, msgNotify, onAgentState, onPtyOutput, ptyResize, ptyWrite } from "../api";
import { clear, el, sanitizeAgentState } from "../dom";
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

  const container = el("div", {
    class: "card-inner terminal-card",
    dataset: { cardId, agentId: currentAgent.id },
  });

  const header = el("div", { class: "card-header" });
  const terminalBody = el("div", { class: "terminal-body" });

  container.appendChild(header);
  container.appendChild(terminalBody);

  if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
    window.matchMedia = (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    });
  }

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
    clear(header);

    const safeState = sanitizeAgentState(currentAgent.state);
    const quotaText =
      currentAgent.state === "waiting_quota" && currentAgent.quota_reset_at
        ? ` (${new Date(currentAgent.quota_reset_at).toLocaleTimeString()})`
        : "";

    const headerLeft = el("div", { class: "card-header-left" });
    headerLeft.appendChild(el("span", { class: "card-icon", html: icons.terminal }));

    const nameEl = el("span", {
      class: "agent-name",
      text: currentAgent.name,
      title: currentAgent.name,
    });
    headerLeft.appendChild(nameEl);

    const presetEl = el("span", {
      class: "badge preset-badge",
      text: currentAgent.preset,
    });
    headerLeft.appendChild(presetEl);

    if (currentAgent.role) {
      const roleEl = el("span", {
        class: "badge role-badge",
        text: currentAgent.role,
      });
      headerLeft.appendChild(roleEl);
    }

    if (currentAgent.floor_id) {
      const floorEl = el("span", {
        class: "badge floor-badge",
        text: currentAgent.floor_id.replace(/^fl_/, ""),
      });
      headerLeft.appendChild(floorEl);
    }

    const statePill = el(
      "span",
      { class: `state-pill state-${safeState}` },
      el("span", { class: "state-dot" }),
      el("span", { class: "state-label", text: `${currentAgent.state}${quotaText}` })
    );
    headerLeft.appendChild(statePill);

    const headerActions = el("div", { class: "card-header-actions" });

    const sendBtn = el("button", {
      class: "card-action-btn send-msg-btn",
      title: "Send message",
      html: icons.messageSquare,
      onClick: (e) => {
        e.stopPropagation();
        openMessageComposer(currentAgent.name);
      },
    });

    const restartBtn = el("button", {
      class: "card-action-btn restart-btn",
      title: "Restart",
      html: icons.refresh,
      onClick: async (e) => {
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
      },
    });

    const stopBtn = el("button", {
      class: "card-action-btn stop-btn",
      title: "Stop",
      html: icons.stop,
      onClick: async (e) => {
        e.stopPropagation();
        try {
          await agentsStop(currentAgent.name);
          term.write("\r\n\x1b[33m[Agent stopped]\x1b[0m\r\n");
        } catch (err) {
          term.write(`\r\n\x1b[31m[Stop failed: ${String(err)}]\x1b[0m\r\n`);
        }
      },
    });

    const maxBtn = el("button", {
      class: "card-action-btn maximize-btn",
      title: isMaximized ? "Restore (Esc)" : "Maximize",
      html: isMaximized ? icons.minimize : icons.maximize,
      onClick: (e) => {
        e.stopPropagation();
        isMaximized = !isMaximized;
        if (onToggleMaximize) {
          onToggleMaximize();
        }
        renderHeader();
        setTimeout(fit, 100);
      },
    });

    headerActions.appendChild(sendBtn);
    headerActions.appendChild(restartBtn);
    headerActions.appendChild(stopBtn);
    headerActions.appendChild(maxBtn);

    header.appendChild(headerLeft);
    header.appendChild(headerActions);
  }

  function openMessageComposer(targetAgent: string) {
    const existing = document.getElementById("terminal-composer-modal");
    if (existing) {
      existing.remove();
    }

    const titleEl = el("span", { text: `Send message to ${targetAgent}` });
    const closeBtn = el("button", { class: "modal-close-btn", text: "\u00D7" });

    const modalHeader = el("div", { class: "modal-header" }, titleEl, closeBtn);

    const textarea = el("textarea", {
      class: "modal-textarea",
      attrs: { placeholder: "Type prompt or instruction to agent..." },
    });

    const modalBody = el("div", { class: "modal-body" }, textarea);

    const cancelBtn = el("button", { class: "btn btn-secondary modal-cancel-btn", text: "Cancel" });
    const sendBtn = el("button", { class: "btn btn-primary modal-send-btn", text: "Send" });

    const modalFooter = el("div", { class: "modal-footer" }, cancelBtn, sendBtn);

    const modalCard = el("div", { class: "modal-card" }, modalHeader, modalBody, modalFooter);

    const modal = el(
      "div",
      { id: "terminal-composer-modal", class: "modal-overlay" },
      modalCard
    );

    document.body.appendChild(modal);
    textarea.focus();

    const closeModal = () => modal.remove();
    closeBtn.addEventListener("click", closeModal);
    cancelBtn.addEventListener("click", closeModal);

    sendBtn.addEventListener("click", async () => {
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
