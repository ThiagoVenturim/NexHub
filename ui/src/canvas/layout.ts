import type { CanvasCard, CanvasConnection, CanvasLayout } from "../types";

export function createDefaultLayout(): CanvasLayout {
  return {
    viewport: {
      panX: 40,
      panY: 40,
      zoom: 1.0,
    },
    cards: [],
    connections: [],
  };
}

export function serializeLayout(layout: CanvasLayout): string {
  return JSON.stringify({
    viewport: {
      panX: Math.round(layout.viewport.panX * 100) / 100,
      panY: Math.round(layout.viewport.panY * 100) / 100,
      zoom: Math.round(layout.viewport.zoom * 1000) / 1000,
    },
    cards: layout.cards.map((c) => ({
      id: c.id,
      type: c.type,
      x: Math.round(c.x),
      y: Math.round(c.y),
      width: Math.round(c.width),
      height: Math.round(c.height),
      title: c.title,
      agentId: c.agentId,
    })),
    connections: layout.connections.map((cn) => ({
      id: cn.id,
      from: { cardId: cn.from.cardId, side: cn.from.side },
      to: { cardId: cn.to.cardId, side: cn.to.side },
    })),
  });
}

export function deserializeLayout(json: string): CanvasLayout {
  if (!json || json.trim() === "" || json === "{}") {
    return createDefaultLayout();
  }

  try {
    const parsed = JSON.parse(json);
    const viewport = {
      panX: typeof parsed.viewport?.panX === "number" ? parsed.viewport.panX : 40,
      panY: typeof parsed.viewport?.panY === "number" ? parsed.viewport.panY : 40,
      zoom: typeof parsed.viewport?.zoom === "number" ? parsed.viewport.zoom : 1.0,
    };

    const cards: CanvasCard[] = Array.isArray(parsed.cards)
      ? parsed.cards.map((c: Partial<CanvasCard>) => ({
          id: String(c.id || `card_${Date.now()}`),
          type: (c.type === "note" ? "note" : "terminal") as "terminal" | "note",
          x: typeof c.x === "number" ? c.x : 100,
          y: typeof c.y === "number" ? c.y : 100,
          width: typeof c.width === "number" ? c.width : 400,
          height: typeof c.height === "number" ? c.height : 260,
          title: String(c.title || "Card"),
          agentId: c.agentId ? String(c.agentId) : undefined,
        }))
      : [];

    const connections: CanvasConnection[] = Array.isArray(parsed.connections)
      ? parsed.connections.map((cn: Partial<CanvasConnection>, i: number) => ({
          id: String(cn.id || `conn_${i}`),
          from: {
            cardId: String(cn.from?.cardId || ""),
            side: cn.from?.side || "right",
          },
          to: {
            cardId: String(cn.to?.cardId || ""),
            side: cn.to?.side || "left",
          },
        }))
      : [];

    return {
      viewport,
      cards,
      connections,
    };
  } catch {
    return createDefaultLayout();
  }
}

export interface DebouncedFunction<T extends (...args: any[]) => void> {
  (...args: Parameters<T>): void;
  cancel(): void;
  flush(): void;
}

export function debounce<T extends (...args: any[]) => void>(
  func: T,
  waitMs: number
): DebouncedFunction<T> {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  let lastArgs: Parameters<T> | null = null;
  let lastThis: any = null;

  const debounced = function (this: any, ...args: Parameters<T>) {
    lastArgs = args;
    lastThis = this;
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
    }
    timeoutId = setTimeout(() => {
      timeoutId = null;
      if (lastArgs) {
        func.apply(lastThis, lastArgs);
        lastArgs = null;
        lastThis = null;
      }
    }, waitMs);
  } as DebouncedFunction<T>;

  debounced.cancel = () => {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
      timeoutId = null;
    }
    lastArgs = null;
    lastThis = null;
  };

  debounced.flush = () => {
    if (timeoutId !== null && lastArgs) {
      clearTimeout(timeoutId);
      timeoutId = null;
      func.apply(lastThis, lastArgs);
      lastArgs = null;
      lastThis = null;
    }
  };

  return debounced;
}
