import { describe, expect, it } from "vitest";
import { getPortPosition, hitTestCard, hitTestPort } from "../src/canvas/hitTest";

describe("hit testing", () => {
  const card = { x: 100, y: 150, width: 400, height: 260 };

  describe("card hit testing", () => {
    it("detects points outside card bounds", () => {
      expect(hitTestCard(50, 150, card).hit).toBe(false);
      expect(hitTestCard(100, 100, card).hit).toBe(false);
      expect(hitTestCard(550, 200, card).hit).toBe(false);
      expect(hitTestCard(200, 450, card).hit).toBe(false);
    });

    it("identifies header clicks", () => {
      const result = hitTestCard(150, 160, card, 36);
      expect(result.hit).toBe(true);
      expect(result.area).toBe("header");
    });

    it("identifies body clicks", () => {
      const result = hitTestCard(200, 250, card, 36);
      expect(result.hit).toBe(true);
      expect(result.area).toBe("body");
    });

    it("identifies resize handle clicks in bottom-right corner", () => {
      const result = hitTestCard(495, 405, card, 36, 16);
      expect(result.hit).toBe(true);
      expect(result.area).toBe("resizeHandle");
    });
  });

  describe("port hit testing", () => {
    it("returns correct coordinates for all 4 ports", () => {
      expect(getPortPosition(card, "left")).toEqual({ x: 100, y: 280 });
      expect(getPortPosition(card, "right")).toEqual({ x: 500, y: 280 });
      expect(getPortPosition(card, "top")).toEqual({ x: 300, y: 150 });
      expect(getPortPosition(card, "bottom")).toEqual({ x: 300, y: 410 });
    });

    it("hits port within radius", () => {
      // Near left port (100, 280)
      expect(hitTestPort(104, 282, card, 12)).toBe("left");
      // Near right port (500, 280)
      expect(hitTestPort(497, 279, card, 12)).toBe("right");
      // Near top port (300, 150)
      expect(hitTestPort(302, 153, card, 12)).toBe("top");
      // Near bottom port (300, 410)
      expect(hitTestPort(299, 408, card, 12)).toBe("bottom");
    });

    it("returns null when point is far from all ports", () => {
      expect(hitTestPort(200, 200, card, 12)).toBeNull();
      expect(hitTestPort(100, 200, card, 12)).toBeNull();
      expect(hitTestPort(400, 410, card, 12)).toBeNull();
    });
  });
});
