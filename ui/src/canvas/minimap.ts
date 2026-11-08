import type { CanvasCard } from "../types";
import type { Viewport } from "./transform";

export interface MinimapOptions {
  container: HTMLElement;
  cards: CanvasCard[];
  viewport: Viewport;
  viewportWidth: number;
  viewportHeight: number;
  onJumpTo: (canvasX: number, canvasY: number) => void;
}

export function renderMinimap(options: MinimapOptions): void {
  const { container, cards, viewport, viewportWidth, viewportHeight, onJumpTo } = options;
  container.innerHTML = "";

  const mapWidth = 140;
  const mapHeight = 90;
  container.style.width = `${mapWidth}px`;
  container.style.height = `${mapHeight}px`;

  // Calculate bounding box containing both cards and current visible viewport
  const visibleCanvasLeft = -viewport.panX / viewport.zoom;
  const visibleCanvasTop = -viewport.panY / viewport.zoom;
  const visibleCanvasRight = (viewportWidth - viewport.panX) / viewport.zoom;
  const visibleCanvasBottom = (viewportHeight - viewport.panY) / viewport.zoom;

  let minX = Math.min(-200, visibleCanvasLeft);
  let minY = Math.min(-200, visibleCanvasTop);
  let maxX = Math.max(1200, visibleCanvasRight);
  let maxY = Math.max(800, visibleCanvasBottom);

  for (const c of cards) {
    minX = Math.min(minX, c.x);
    minY = Math.min(minY, c.y);
    maxX = Math.max(maxX, c.x + c.width);
    maxY = Math.max(maxY, c.y + c.height);
  }

  const worldWidth = Math.max(1, maxX - minX);
  const worldHeight = Math.max(1, maxY - minY);
  const scale = Math.min(mapWidth / worldWidth, mapHeight / worldHeight);

  const canvasXToMini = (cx: number) => (cx - minX) * scale;
  const canvasYToMini = (cy: number) => (cy - minY) * scale;

  // Mini canvas or SVG
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", String(mapWidth));
  svg.setAttribute("height", String(mapHeight));
  svg.setAttribute("class", "minimap-svg");

  // Render cards
  for (const card of cards) {
    const rx = canvasXToMini(card.x);
    const ry = canvasYToMini(card.y);
    const rw = Math.max(2, card.width * scale);
    const rh = Math.max(2, card.height * scale);

    const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
    rect.setAttribute("x", String(rx));
    rect.setAttribute("y", String(ry));
    rect.setAttribute("width", String(rw));
    rect.setAttribute("height", String(rh));
    rect.setAttribute("rx", "2");
    rect.setAttribute("fill", card.type === "terminal" ? "#2bb3a3" : "#e0a43a");
    rect.setAttribute("opacity", "0.6");
    svg.appendChild(rect);
  }

  // Render current visible viewport rect
  const vx = canvasXToMini(visibleCanvasLeft);
  const vy = canvasYToMini(visibleCanvasTop);
  const vw = Math.max(4, (visibleCanvasRight - visibleCanvasLeft) * scale);
  const vh = Math.max(4, (visibleCanvasBottom - visibleCanvasTop) * scale);

  const viewRect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  viewRect.setAttribute("x", String(vx));
  viewRect.setAttribute("y", String(vy));
  viewRect.setAttribute("width", String(vw));
  viewRect.setAttribute("height", String(vh));
  viewRect.setAttribute("fill", "none");
  viewRect.setAttribute("stroke", "#ffffff");
  viewRect.setAttribute("stroke-width", "1.5");
  viewRect.setAttribute("opacity", "0.85");
  svg.appendChild(viewRect);

  container.appendChild(svg);

  // Click handler to jump viewport
  container.onclick = (e) => {
    const rect = container.getBoundingClientRect();
    const clickMiniX = e.clientX - rect.left;
    const clickMiniY = e.clientY - rect.top;

    const targetCanvasX = minX + clickMiniX / scale;
    const targetCanvasY = minY + clickMiniY / scale;

    onJumpTo(targetCanvasX, targetCanvasY);
  };
}
