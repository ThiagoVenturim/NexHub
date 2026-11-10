export interface Viewport {
  panX: number;
  panY: number;
  zoom: number;
}

export interface Point {
  x: number;
  y: number;
}

export interface Bounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 2.0;

export function clampZoom(zoom: number): number {
  return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, zoom));
}

export function screenToCanvas(screenX: number, screenY: number, viewport: Viewport): Point {
  return {
    x: (screenX - viewport.panX) / viewport.zoom,
    y: (screenY - viewport.panY) / viewport.zoom,
  };
}

export function canvasToScreen(canvasX: number, canvasY: number, viewport: Viewport): Point {
  return {
    x: canvasX * viewport.zoom + viewport.panX,
    y: canvasY * viewport.zoom + viewport.panY,
  };
}

export function zoomAtScreenPoint(
  screenX: number,
  screenY: number,
  factor: number,
  viewport: Viewport
): Viewport {
  const canvasPoint = screenToCanvas(screenX, screenY, viewport);
  const newZoom = clampZoom(viewport.zoom * factor);

  return {
    zoom: newZoom,
    panX: screenX - canvasPoint.x * newZoom,
    panY: screenY - canvasPoint.y * newZoom,
  };
}

export function fitAll(
  items: Bounds[],
  viewportWidth: number,
  viewportHeight: number,
  padding = 40
): Viewport {
  if (items.length === 0) {
    return { panX: 0, panY: 0, zoom: 1 };
  }

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const item of items) {
    minX = Math.min(minX, item.x);
    minY = Math.min(minY, item.y);
    maxX = Math.max(maxX, item.x + item.width);
    maxY = Math.max(maxY, item.y + item.height);
  }

  const contentWidth = maxX - minX;
  const contentHeight = maxY - minY;

  const availableWidth = Math.max(10, viewportWidth - padding * 2);
  const availableHeight = Math.max(10, viewportHeight - padding * 2);

  const scaleX = availableWidth / (contentWidth || 1);
  const scaleY = availableHeight / (contentHeight || 1);
  const newZoom = clampZoom(Math.min(scaleX, scaleY, 1.0));

  const centerX = (minX + maxX) / 2;
  const centerY = (minY + maxY) / 2;

  const panX = viewportWidth / 2 - centerX * newZoom;
  const panY = viewportHeight / 2 - centerY * newZoom;

  return {
    zoom: newZoom,
    panX,
    panY,
  };
}
