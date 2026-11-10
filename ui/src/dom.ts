export interface ElOptions {
  id?: string;
  class?: string;
  className?: string;
  text?: string;
  title?: string;
  attrs?: Record<string, string>;
  dataset?: Record<string, string>;
  style?: Record<string, string | number>;
  onClick?: (e: MouseEvent) => void;
  onContextMenu?: (e: MouseEvent) => void;
  html?: string; // Allowed only for static SVG icons, NEVER dynamic data
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  options?: ElOptions,
  ...children: (Node | string | null | undefined)[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);

  if (options) {
    if (options.id) node.id = options.id;
    if (options.class) node.className = options.class;
    if (options.className) node.className = options.className;
    if (options.text !== undefined) node.textContent = options.text;
    if (options.title !== undefined) node.title = options.title;

    if (options.attrs) {
      for (const [key, val] of Object.entries(options.attrs)) {
        node.setAttribute(key, val);
      }
    }

    if (options.dataset) {
      for (const [key, val] of Object.entries(options.dataset)) {
        node.dataset[key] = val;
      }
    }

    if (options.style) {
      for (const [key, val] of Object.entries(options.style)) {
        if (val !== undefined && val !== null) {
          node.style.setProperty(key, String(val));
        }
      }
    }

    if (options.html !== undefined) {
      node.innerHTML = options.html;
    }

    if (options.onClick) {
      node.addEventListener("click", options.onClick as EventListener);
    }
    if (options.onContextMenu) {
      node.addEventListener("contextmenu", options.onContextMenu as EventListener);
    }
  }

  for (const child of children) {
    if (child === null || child === undefined) continue;
    if (typeof child === "string") {
      node.appendChild(document.createTextNode(child));
    } else {
      node.appendChild(child);
    }
  }

  return node;
}

export function clear(node: Node): void {
  while (node.firstChild) {
    node.removeChild(node.firstChild);
  }
}

const VALID_AGENT_STATES = new Set([
  "stopped",
  "starting",
  "running",
  "busy",
  "waiting_quota",
  "exited",
  "error",
]);

export function sanitizeAgentState(state: string): string {
  return VALID_AGENT_STATES.has(state) ? state : "unknown";
}

const VALID_FLOOR_STATUSES = new Set(["active", "merged", "removed"]);

export function sanitizeFloorStatus(status: string): string {
  return VALID_FLOOR_STATUSES.has(status) ? status : "unknown";
}

const VALID_MESSAGE_KINDS = new Set(["ask", "notify", "broadcast", "reply"]);

export function sanitizeMessageKind(kind: string): string {
  return VALID_MESSAGE_KINDS.has(kind) ? kind : "unknown";
}

const VALID_MESSAGE_STATES = new Set([
  "queued",
  "delivered",
  "answered",
  "expired",
  "failed",
]);

export function sanitizeMessageState(state: string): string {
  return VALID_MESSAGE_STATES.has(state) ? state : "unknown";
}
