import { describe, expect, it } from "vitest";
import {
  canvasToScreen,
  clampZoom,
  fitAll,
  MAX_ZOOM,
  MIN_ZOOM,
  screenToCanvas,
  zoomAtScreenPoint,
  type Viewport,
} from "../src/canvas/transform";

describe("canvas mathematics", () => {
  it("converts screen to canvas and back losslessly", () => {
    const viewport: Viewport = { panX: 120, panY: -80, zoom: 1.5 };
    const originalScreen = { x: 350, y: 420 };

    const canvasPoint = screenToCanvas(originalScreen.x, originalScreen.y, viewport);
    const convertedScreen = canvasToScreen(canvasPoint.x, canvasPoint.y, viewport);

    expect(convertedScreen.x).toBeCloseTo(originalScreen.x, 5);
    expect(convertedScreen.y).toBeCloseTo(originalScreen.y, 5);

    const originalCanvas = { x: -45.5, y: 310.2 };
    const screenPt = canvasToScreen(originalCanvas.x, originalCanvas.y, viewport);
    const convertedCanvas = screenToCanvas(screenPt.x, screenPt.y, viewport);

    expect(convertedCanvas.x).toBeCloseTo(originalCanvas.x, 5);
    expect(convertedCanvas.y).toBeCloseTo(originalCanvas.y, 5);
  });

  it("zooming at cursor preserves the point under cursor", () => {
    const initialViewport: Viewport = { panX: 50, panY: 100, zoom: 1.0 };
    const cursor = { x: 400, y: 300 };

    // Point in canvas before zoom
    const canvasPointBefore = screenToCanvas(cursor.x, cursor.y, initialViewport);

    // Zoom in by factor 1.25
    const newViewport = zoomAtScreenPoint(cursor.x, cursor.y, 1.25, initialViewport);

    // Screen point of the same canvas point after zoom
    const screenPointAfter = canvasToScreen(canvasPointBefore.x, canvasPointBefore.y, newViewport);

    expect(screenPointAfter.x).toBeCloseTo(cursor.x, 5);
    expect(screenPointAfter.y).toBeCloseTo(cursor.y, 5);
    expect(newViewport.zoom).toBeCloseTo(1.25, 5);
  });

  it("enforces zoom limits between 25% and 200%", () => {
    expect(clampZoom(0.1)).toBe(MIN_ZOOM);
    expect(clampZoom(0.24)).toBe(MIN_ZOOM);
    expect(clampZoom(0.25)).toBe(0.25);
    expect(clampZoom(1.0)).toBe(1.0);
    expect(clampZoom(2.0)).toBe(2.0);
    expect(clampZoom(2.1)).toBe(MAX_ZOOM);
    expect(clampZoom(10.0)).toBe(MAX_ZOOM);

    const viewport: Viewport = { panX: 0, panY: 0, zoom: 1.8 };
    const zoomedPastMax = zoomAtScreenPoint(200, 200, 2.0, viewport);
    expect(zoomedPastMax.zoom).toBe(MAX_ZOOM);

    const zoomedPastMin = zoomAtScreenPoint(200, 200, 0.05, viewport);
    expect(zoomedPastMin.zoom).toBe(MIN_ZOOM);
  });

  it("fitAll calculates bounding box and fits items inside viewport", () => {
    const cards = [
      { x: 100, y: 100, width: 400, height: 300 },
      { x: 600, y: 200, width: 400, height: 300 },
    ];
    // Overall bounds: [100, 100] to [1000, 500], width 900, height 400
    const vp = fitAll(cards, 1200, 800, 40);

    expect(vp.zoom).toBeGreaterThanOrEqual(MIN_ZOOM);
    expect(vp.zoom).toBeLessThanOrEqual(MAX_ZOOM);

    // Center of bounds (550, 300) should be near screen center (600, 400)
    const centerScreen = canvasToScreen(550, 300, vp);
    expect(centerScreen.x).toBeCloseTo(600, 1);
    expect(centerScreen.y).toBeCloseTo(400, 1);
  });

  it("fitAll handles empty card list safely", () => {
    const vp = fitAll([], 800, 600);
    expect(vp).toEqual({ panX: 0, panY: 0, zoom: 1 });
  });
});
