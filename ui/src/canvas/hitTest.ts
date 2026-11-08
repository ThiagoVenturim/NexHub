import type { Bounds, Point } from "./transform";
import type { PortSide } from "../types";

export interface CardHitResult {
  hit: boolean;
  area?: "header" | "body" | "resizeHandle";
}

export function hitTestCard(
  canvasX: number,
  canvasY: number,
  card: Bounds,
  headerHeight = 36,
  resizeHandleSize = 16
): CardHitResult {
  if (
    canvasX < card.x ||
    canvasX > card.x + card.width ||
    canvasY < card.y ||
    canvasY > card.y + card.height
  ) {
    return { hit: false };
  }

  // Check resize handle in bottom-right corner
  if (
    canvasX >= card.x + card.width - resizeHandleSize &&
    canvasY >= card.y + card.height - resizeHandleSize
  ) {
    return { hit: true, area: "resizeHandle" };
  }

  // Check header area
  if (canvasY <= card.y + headerHeight) {
    return { hit: true, area: "header" };
  }

  return { hit: true, area: "body" };
}

export function getPortPosition(card: Bounds, side: PortSide): Point {
  switch (side) {
    case "left":
      return { x: card.x, y: card.y + card.height / 2 };
    case "right":
      return { x: card.x + card.width, y: card.y + card.height / 2 };
    case "top":
      return { x: card.x + card.width / 2, y: card.y };
    case "bottom":
      return { x: card.x + card.width / 2, y: card.y + card.height };
  }
}

export function hitTestPort(
  canvasX: number,
  canvasY: number,
  card: Bounds,
  radius = 12
): PortSide | null {
  const sides: PortSide[] = ["left", "right", "top", "bottom"];
  const radiusSq = radius * radius;

  for (const side of sides) {
    const pos = getPortPosition(card, side);
    const dx = canvasX - pos.x;
    const dy = canvasY - pos.y;
    if (dx * dx + dy * dy <= radiusSq) {
      return side;
    }
  }

  return null;
}
