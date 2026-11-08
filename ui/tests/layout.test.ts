import { describe, expect, it } from "vitest";
import { createDefaultLayout, deserializeLayout, serializeLayout } from "../src/canvas/layout";
import type { CanvasLayout } from "../src/types";

describe("layout serialization", () => {
  it("serializes and deserializes layout losslessly", () => {
    const layout: CanvasLayout = {
      viewport: {
        panX: 120.5,
        panY: -45.2,
        zoom: 1.25,
      },
      cards: [
        {
          id: "card_1",
          type: "terminal",
          x: 100,
          y: 80,
          width: 460,
          height: 300,
          title: "builder",
          agentId: "ag_1",
        },
        {
          id: "card_2",
          type: "note",
          x: 600,
          y: 120,
          width: 380,
          height: 280,
          title: "board",
        },
      ],
      connections: [
        {
          id: "conn_1",
          from: { cardId: "card_1", side: "right" },
          to: { cardId: "card_2", side: "left" },
        },
      ],
    };

    const json = serializeLayout(layout);
    expect(typeof json).toBe("string");

    const restored = deserializeLayout(json);

    expect(restored.viewport.panX).toBe(120.5);
    expect(restored.viewport.panY).toBe(-45.2);
    expect(restored.viewport.zoom).toBe(1.25);

    expect(restored.cards).toHaveLength(2);
    expect(restored.cards[0]).toEqual({
      id: "card_1",
      type: "terminal",
      x: 100,
      y: 80,
      width: 460,
      height: 300,
      title: "builder",
      agentId: "ag_1",
    });
    expect(restored.cards[1]).toEqual({
      id: "card_2",
      type: "note",
      x: 600,
      y: 120,
      width: 380,
      height: 280,
      title: "board",
      agentId: undefined,
    });

    expect(restored.connections).toHaveLength(1);
    expect(restored.connections[0]).toEqual({
      id: "conn_1",
      from: { cardId: "card_1", side: "right" },
      to: { cardId: "card_2", side: "left" },
    });

    // Re-serializing gives the exact same JSON
    expect(serializeLayout(restored)).toBe(json);
  });

  it("handles empty or malformed layout JSON gracefully", () => {
    const emptyLayout = deserializeLayout("");
    expect(emptyLayout).toEqual(createDefaultLayout());

    const jsonEmptyObj = deserializeLayout("{}");
    expect(jsonEmptyObj).toEqual(createDefaultLayout());

    const malformed = deserializeLayout("not a json string");
    expect(malformed).toEqual(createDefaultLayout());
  });
});
