import fs from "node:fs";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { setCustomInvoker } from "../src/api";
import { createNoteCardElement } from "../src/cards/note";
import { createTerminalCard } from "../src/cards/terminal";
import { openCommandPalette } from "../src/components/commandPalette";
import { createMessageLog } from "../src/components/messageLog";
import { createTopBar } from "../src/components/topBar";
import { createSidebar } from "../src/sidebar/sidebar";
import type { Agent, Floor, Group, Message, Routine, Workspace } from "../src/types";

describe("XSS defense and dynamic data rendering", () => {
  const XSS_PAYLOAD = '<img src=x onerror=window.__xss=1>';

  beforeEach(() => {
    delete (window as unknown as { __xss?: unknown }).__xss;
  });

  afterEach(() => {
    delete (window as unknown as { __xss?: unknown }).__xss;
    document.body.innerHTML = "";
  });

  it("fails if any innerHTML with ${ is found in ui/src", () => {
    const srcDir = path.resolve(__dirname, "../src");

    function scanDir(dir: string): string[] {
      const entries = fs.readdirSync(dir, { withFileTypes: true });
      let violations: string[] = [];
      for (const entry of entries) {
        const fullPath = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          violations = violations.concat(scanDir(fullPath));
        } else if (entry.isFile() && entry.name.endsWith(".ts")) {
          const content = fs.readFileSync(fullPath, "utf-8");
          // Check for any innerHTML assignment using template string with dynamic expression ${
          const innerHtmlMatches = content.match(/innerHTML\s*=\s*`[^`]*\${/g);
          if (innerHtmlMatches) {
            violations.push(
              `${entry.name}: found innerHTML with template interpolation: ${innerHtmlMatches.join(", ")}`
            );
          }
        }
      }
      return violations;
    }

    const violations = scanDir(srcDir);
    expect(violations).toEqual([]);
  });

  it("renders terminal header and actions safely with XSS payload", () => {
    const agent: Agent = {
      id: "ag_xss",
      workspace_id: "ws_1",
      name: XSS_PAYLOAD,
      preset: XSS_PAYLOAD,
      role: XSS_PAYLOAD,
      floor_id: "fl_xss",
      cwd: "/test",
      state: "running",
      created_at: Date.now(),
    };

    const handle = createTerminalCard({
      cardId: "card_1",
      agent,
    });

    document.body.appendChild(handle.element);

    expect((window as unknown as { __xss?: unknown }).__xss).toBeUndefined();
    expect(handle.element.textContent).toContain(XSS_PAYLOAD);
  });

  it("renders message composer modal safely with XSS payload in target name", () => {
    const agent: Agent = {
      id: "ag_xss_2",
      workspace_id: "ws_1",
      name: XSS_PAYLOAD,
      preset: "agy",
      cwd: "/test",
      state: "running",
      created_at: Date.now(),
    };

    const handle = createTerminalCard({
      cardId: "card_2",
      agent,
    });

    document.body.appendChild(handle.element);

    // Trigger message composer
    const sendBtn = handle.element.querySelector(".send-msg-btn") as HTMLButtonElement;
    sendBtn?.click();

    expect((window as unknown as { __xss?: unknown }).__xss).toBeUndefined();
    expect(document.body.textContent).toContain(`Send message to ${XSS_PAYLOAD}`);
  });

  it("renders top bar safely with XSS payload in workspace name", () => {
    const container = document.createElement("header");
    document.body.appendChild(container);

    const topBar = createTopBar(
      container,
      XSS_PAYLOAD,
      () => {},
      () => {}
    );

    expect((window as unknown as { __xss?: unknown }).__xss).toBeUndefined();
    expect(topBar.element.textContent).toContain(XSS_PAYLOAD);
  });

  it("renders sidebar safely with XSS payload in workspace, floor and routine names", async () => {
    const container = document.createElement("aside");
    document.body.appendChild(container);

    setCustomInvoker((cmd) => {
      if (cmd === "groups_list") {
        return Promise.resolve({
          groups: [{ id: "g1", name: XSS_PAYLOAD, position: 0 } as Group],
        });
      }
      if (cmd === "workspaces_list") {
        return Promise.resolve({
          workspaces: [
            {
              id: "w1",
              name: XSS_PAYLOAD,
              repo_path: "/test",
              group_id: "g1",
              layout_json: "{}",
              created_at: Date.now(),
            } as Workspace,
          ],
        });
      }
      if (cmd === "floors_list") {
        return Promise.resolve({
          floors: [
            {
              id: "fl_1",
              workspace_id: "w1",
              name: XSS_PAYLOAD,
              branch: "layer/xss",
              path: "/worktrees/xss",
              base_ref: "origin/develop",
              status: "active",
              created_at: Date.now(),
            } as Floor,
          ],
        });
      }
      if (cmd === "routines_list") {
        return Promise.resolve({
          routines: [
            {
              id: "r1",
              workspace_id: "w1",
              name: XSS_PAYLOAD,
              schedule: "every 5m",
              body: "",
              enabled: 1,
            } as Routine,
          ],
        });
      }
      return Promise.resolve({});
    });

    const sidebar = createSidebar(container, "w1", () => {});
    await sidebar.refresh();

    expect((window as unknown as { __xss?: unknown }).__xss).toBeUndefined();
    expect(sidebar.element.textContent).toContain(XSS_PAYLOAD);

    setCustomInvoker(null);
  });

  it("renders message log safely with XSS payload in message body and fields", () => {
    const container = document.createElement("footer");
    document.body.appendChild(container);

    const log = createMessageLog(container);

    const maliciousMsg: Message = {
      id: "msg_xss",
      workspace_id: "w1",
      from_agent: XSS_PAYLOAD,
      to_agent: XSS_PAYLOAD,
      kind: "ask",
      body: XSS_PAYLOAD,
      state: "delivered",
      created_at: Date.now(),
    };

    log.addMessage(maliciousMsg);

    expect((window as unknown as { __xss?: unknown }).__xss).toBeUndefined();
    expect(log.element.textContent).toContain(XSS_PAYLOAD);
  });

  it("renders command palette safely with XSS payload", () => {
    openCommandPalette([
      {
        id: "cmd_xss",
        title: XSS_PAYLOAD,
        description: XSS_PAYLOAD,
        shortcut: XSS_PAYLOAD,
        action: () => {},
      },
    ]);

    expect((window as unknown as { __xss?: unknown }).__xss).toBeUndefined();
    expect(document.body.textContent).toContain(XSS_PAYLOAD);
  });

  it("renders note card title safely with XSS payload", () => {
    const handle = createNoteCardElement("note_xss", XSS_PAYLOAD, "# Content");
    document.body.appendChild(handle.element);

    const input = handle.element.querySelector(".note-title-input") as HTMLInputElement;
    expect((window as unknown as { __xss?: unknown }).__xss).toBeUndefined();
    expect(input.value).toBe(XSS_PAYLOAD);
  });
});
