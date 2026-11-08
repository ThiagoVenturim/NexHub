import { agentsList, agentsSpawn, layoutSave, notesList, onMessage } from "../api";
import { createNoteCardElement } from "../cards/note";
import { createTerminalCard, type TerminalCardHandle } from "../cards/terminal";
import { clear, el } from "../dom";
import type { Agent, CanvasCard, CanvasConnection, CanvasLayout, PortSide } from "../types";
import { getPortPosition, hitTestPort } from "./hitTest";
import { debounce, deserializeLayout, serializeLayout } from "./layout";
import { renderMinimap } from "./minimap";
import { clampZoom, fitAll, screenToCanvas, zoomAtScreenPoint, type Viewport } from "./transform";

export interface CanvasController {
  element: HTMLElement;
  loadLayout: (layoutJson: string) => Promise<void>;
  addAgentCard: (agent: Agent, x?: number, y?: number) => void;
  addNoteCard: (title: string, body?: string, x?: number, y?: number) => void;
  fitAllCards: () => void;
  focusCardByIndex: (index: number) => void;
  leaveMaximized: () => void;
  hasMaximizedCard: () => boolean;
  saveNow: () => void;
  dispose: () => void;
}

export function createCanvas(container: HTMLElement, workspaceId: string): CanvasController {
  clear(container);
  container.classList.add("nexhub-canvas-root");

  // Create layer hierarchy
  const gridBackground = document.createElement("div");
  gridBackground.className = "canvas-grid-bg";

  const svgConnections = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svgConnections.setAttribute("class", "canvas-connections-svg");

  const cardsContainer = document.createElement("div");
  cardsContainer.className = "canvas-cards-container";

  const minimapElement = document.createElement("div");
  minimapElement.className = "canvas-minimap";

  container.appendChild(gridBackground);
  container.appendChild(svgConnections);
  container.appendChild(cardsContainer);
  container.appendChild(minimapElement);

  let viewport: Viewport = { panX: 60, panY: 60, zoom: 1.0 };
  let cards: CanvasCard[] = [];
  let connections: CanvasConnection[] = [];
  let selectedCardId: string | null = null;
  let selectedConnectionId: string | null = null;
  let maximizedCardId: string | null = null;

  const cardHandles = new Map<string, TerminalCardHandle | { element: HTMLElement; saveImmediately: () => void }>();

  // Layout save debounced to 500 ms
  const debouncedSave = debounce(() => {
    const layout: CanvasLayout = {
      viewport,
      cards,
      connections,
    };
    const json = serializeLayout(layout);
    layoutSave(workspaceId, json).catch((err) => {
      console.error("Failed to save layout:", err);
    });
  }, 500);

  function applyTransform() {
    // Update grid dot position & size
    gridBackground.style.backgroundPosition = `${viewport.panX}px ${viewport.panY}px`;
    gridBackground.style.backgroundSize = `${24 * viewport.zoom}px ${24 * viewport.zoom}px`;

    // Transform cards container
    cardsContainer.style.transform = `translate(${viewport.panX}px, ${viewport.panY}px) scale(${viewport.zoom})`;
    cardsContainer.style.transformOrigin = "0 0";

    renderConnections();
    updateMinimap();
  }

  function updateMinimap() {
    renderMinimap({
      container: minimapElement,
      cards,
      viewport,
      viewportWidth: container.clientWidth || 800,
      viewportHeight: container.clientHeight || 600,
      onJumpTo: (cx, cy) => {
        viewport.panX = (container.clientWidth || 800) / 2 - cx * viewport.zoom;
        viewport.panY = (container.clientHeight || 600) / 2 - cy * viewport.zoom;
        applyTransform();
        debouncedSave();
      },
    });
  }

  function getBezierPath(p1: { x: number; y: number }, s1: PortSide, p2: { x: number; y: number }, s2: PortSide): string {
    const dist = Math.hypot(p2.x - p1.x, p2.y - p1.y);
    const offset = Math.max(30, dist * 0.4);

    let cp1x = p1.x;
    let cp1y = p1.y;
    if (s1 === "left") cp1x -= offset;
    else if (s1 === "right") cp1x += offset;
    else if (s1 === "top") cp1y -= offset;
    else if (s1 === "bottom") cp1y += offset;

    let cp2x = p2.x;
    let cp2y = p2.y;
    if (s2 === "left") cp2x -= offset;
    else if (s2 === "right") cp2x += offset;
    else if (s2 === "top") cp2y -= offset;
    else if (s2 === "bottom") cp2y += offset;

    return `M ${p1.x} ${p1.y} C ${cp1x} ${cp1y}, ${cp2x} ${cp2y}, ${p2.x} ${p2.y}`;
  }

  function renderConnections() {
    clear(svgConnections);
    svgConnections.style.width = "100%";
    svgConnections.style.height = "100%";

    const now = Date.now();

    for (const conn of connections) {
      const fromCard = cards.find((c) => c.id === conn.from.cardId);
      const toCard = cards.find((c) => c.id === conn.to.cardId);
      if (!fromCard || !toCard) continue;

      const p1Canvas = getPortPosition(fromCard, conn.from.side);
      const p2Canvas = getPortPosition(toCard, conn.to.side);

      // Convert to screen coordinates for SVG overlay
      const p1Screen = {
        x: p1Canvas.x * viewport.zoom + viewport.panX,
        y: p1Canvas.y * viewport.zoom + viewport.panY,
      };
      const p2Screen = {
        x: p2Canvas.x * viewport.zoom + viewport.panX,
        y: p2Canvas.y * viewport.zoom + viewport.panY,
      };

      const pathData = getBezierPath(p1Screen, conn.from.side, p2Screen, conn.to.side);

      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", pathData);
      path.setAttribute("fill", "none");

      const isSelected = selectedConnectionId === conn.id;
      const isPulsing = Boolean(conn.pulseUntil && conn.pulseUntil > now);

      if (isPulsing) {
        path.setAttribute("stroke", "#4caf7a");
        path.setAttribute("stroke-width", "3.5");
        path.setAttribute("class", "connection-pulsing");
      } else if (isSelected) {
        path.setAttribute("stroke", "#2bb3a3");
        path.setAttribute("stroke-width", "2.5");
        path.setAttribute("stroke-dasharray", "4 2");
      } else {
        path.setAttribute("stroke", "#3f4957");
        path.setAttribute("stroke-width", "2");
      }

      path.style.cursor = "pointer";
      path.onclick = (e) => {
        e.stopPropagation();
        selectedConnectionId = conn.id;
        selectedCardId = null;
        renderConnections();
        updateSelectionUi();
      };

      svgConnections.appendChild(path);
    }
  }

  function updateSelectionUi() {
    const cardEls = cardsContainer.querySelectorAll(".canvas-card-wrapper");
    cardEls.forEach((el) => {
      const id = (el as HTMLElement).dataset.cardId;
      if (id === selectedCardId) {
        el.classList.add("selected");
      } else {
        el.classList.remove("selected");
      }
    });
  }

  function renderCardElement(card: CanvasCard, contentElement: HTMLElement) {
    const wrapper = document.createElement("div");
    wrapper.className = "canvas-card-wrapper";
    wrapper.dataset.cardId = card.id;

    if (maximizedCardId === card.id) {
      wrapper.classList.add("maximized");
      wrapper.style.left = "0px";
      wrapper.style.top = "0px";
      wrapper.style.width = "100%";
      wrapper.style.height = "100%";
      wrapper.style.transform = "none";
    } else {
      wrapper.style.left = `${card.x}px`;
      wrapper.style.top = `${card.y}px`;
      wrapper.style.width = `${card.width}px`;
      wrapper.style.height = `${card.height}px`;
    }

    // Four ports: left, right, top, bottom
    const portSides: PortSide[] = ["left", "right", "top", "bottom"];
    portSides.forEach((side) => {
      const port = document.createElement("div");
      port.className = `canvas-port port-${side}`;
      port.dataset.side = side;
      port.dataset.cardId = card.id;

      port.addEventListener("mousedown", (e) => {
        e.stopPropagation();
        startConnecting(card.id, side, e);
      });

      wrapper.appendChild(port);
    });

    // Resize handle
    const resizeHandle = document.createElement("div");
    resizeHandle.className = "canvas-resize-handle";
    resizeHandle.addEventListener("mousedown", (e) => {
      e.stopPropagation();
      startResizing(card, e);
    });
    wrapper.appendChild(resizeHandle);

    // Header drag handler
    wrapper.addEventListener("mousedown", (e) => {
      const target = e.target as HTMLElement;
      if (target.closest(".canvas-port") || target.closest(".canvas-resize-handle")) {
        return;
      }
      if (target.closest("button") || target.closest("input") || target.closest("textarea")) {
        return;
      }

      selectedCardId = card.id;
      selectedConnectionId = null;
      updateSelectionUi();
      renderConnections();

      // Drag card
      if (maximizedCardId !== card.id) {
        startDraggingCard(card, e);
      }
    });

    wrapper.appendChild(contentElement);
    cardsContainer.appendChild(wrapper);
  }

  function startDraggingCard(card: CanvasCard, startEvent: MouseEvent) {
    const startX = startEvent.clientX;
    const startY = startEvent.clientY;
    const initialCardX = card.x;
    const initialCardY = card.y;

    const onMouseMove = (e: MouseEvent) => {
      const dx = (e.clientX - startX) / viewport.zoom;
      const dy = (e.clientY - startY) / viewport.zoom;
      card.x = Math.round(initialCardX + dx);
      card.y = Math.round(initialCardY + dy);

      const wrapper = cardsContainer.querySelector(`[data-card-id="${card.id}"]`) as HTMLElement;
      if (wrapper) {
        wrapper.style.left = `${card.x}px`;
        wrapper.style.top = `${card.y}px`;
      }
      renderConnections();
      updateMinimap();
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      debouncedSave();
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  }

  function startResizing(card: CanvasCard, startEvent: MouseEvent) {
    const startX = startEvent.clientX;
    const startY = startEvent.clientY;
    const initialW = card.width;
    const initialH = card.height;
    const minW = card.type === "terminal" ? 360 : 240;
    const minH = card.type === "terminal" ? 220 : 160;

    const onMouseMove = (e: MouseEvent) => {
      const dw = (e.clientX - startX) / viewport.zoom;
      const dh = (e.clientY - startY) / viewport.zoom;
      card.width = Math.max(minW, Math.round(initialW + dw));
      card.height = Math.max(minH, Math.round(initialH + dh));

      const wrapper = cardsContainer.querySelector(`[data-card-id="${card.id}"]`) as HTMLElement;
      if (wrapper) {
        wrapper.style.width = `${card.width}px`;
        wrapper.style.height = `${card.height}px`;
      }

      const handle = cardHandles.get(card.id);
      if (handle && "fit" in handle) {
        handle.fit();
      }

      renderConnections();
      updateMinimap();
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      debouncedSave();
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  }

  function startConnecting(fromCardId: string, fromSide: PortSide, _startEvent: MouseEvent) {
    const fromCard = cards.find((c) => c.id === fromCardId);
    if (!fromCard) return;

    const tempPath = document.createElementNS("http://www.w3.org/2000/svg", "path");
    tempPath.setAttribute("fill", "none");
    tempPath.setAttribute("stroke", "#2bb3a3");
    tempPath.setAttribute("stroke-width", "2");
    tempPath.setAttribute("stroke-dasharray", "4 4");
    svgConnections.appendChild(tempPath);

    const p1Canvas = getPortPosition(fromCard, fromSide);
    const p1Screen = {
      x: p1Canvas.x * viewport.zoom + viewport.panX,
      y: p1Canvas.y * viewport.zoom + viewport.panY,
    };

    const onMouseMove = (e: MouseEvent) => {
      const p2Screen = { x: e.clientX, y: e.clientY };
      const d = getBezierPath(p1Screen, fromSide, p2Screen, "left");
      tempPath.setAttribute("d", d);
    };

    const onMouseUp = (e: MouseEvent) => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      tempPath.remove();

      // Check if dropped on a port
      const targetCanvas = screenToCanvas(e.clientX, e.clientY, viewport);
      for (const card of cards) {
        if (card.id === fromCardId) continue;
        const hitSide = hitTestPort(targetCanvas.x, targetCanvas.y, card, 20);
        if (hitSide) {
          // Create connection
          const newConn: CanvasConnection = {
            id: `conn_${Date.now()}`,
            from: { cardId: fromCardId, side: fromSide },
            to: { cardId: card.id, side: hitSide },
          };
          connections.push(newConn);
          renderConnections();
          debouncedSave();
          break;
        }
      }
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  }

  // Pan interaction
  let isPanning = false;
  let isSpacePressed = false;
  let panStartX = 0;
  let panStartY = 0;
  let startViewportPanX = 0;
  let startViewportPanY = 0;

  window.addEventListener("keydown", (e) => {
    if (e.code === "Space" && (document.activeElement?.tagName !== "INPUT" && document.activeElement?.tagName !== "TEXTAREA")) {
      isSpacePressed = true;
    }
  });

  window.addEventListener("keyup", (e) => {
    if (e.code === "Space") {
      isSpacePressed = false;
    }
  });

  container.addEventListener("mousedown", (e) => {
    // Left click on empty space or middle click or Space + click
    if (e.target === container || e.target === gridBackground || (e.target as HTMLElement).tagName === "svg" || e.button === 1 || isSpacePressed) {
      isPanning = true;
      panStartX = e.clientX;
      panStartY = e.clientY;
      startViewportPanX = viewport.panX;
      startViewportPanY = viewport.panY;
      selectedCardId = null;
      selectedConnectionId = null;
      updateSelectionUi();
      renderConnections();
    }
  });

  window.addEventListener("mousemove", (e) => {
    if (!isPanning) return;
    const dx = e.clientX - panStartX;
    const dy = e.clientY - panStartY;
    viewport.panX = startViewportPanX + dx;
    viewport.panY = startViewportPanY + dy;
    applyTransform();
  });

  window.addEventListener("mouseup", () => {
    if (isPanning) {
      isPanning = false;
      debouncedSave();
    }
  });

  // Zoom interaction
  container.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const rect = container.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;

      if (e.ctrlKey || e.metaKey) {
        // Zoom around cursor
        const zoomFactor = e.deltaY < 0 ? 1.1 : 0.9;
        viewport = zoomAtScreenPoint(screenX, screenY, zoomFactor, viewport);
      } else {
        // Two-finger scroll / trackpad pan
        viewport.panX -= e.deltaX;
        viewport.panY -= e.deltaY;
      }

      applyTransform();
      debouncedSave();
    },
    { passive: false }
  );

  // Double click empty space: new card menu
  container.addEventListener("dblclick", (e) => {
    if (e.target !== container && e.target !== gridBackground && (e.target as HTMLElement).tagName !== "svg") {
      return;
    }
    const canvasPoint = screenToCanvas(e.clientX, e.clientY, viewport);
    openNewCardMenu(e.clientX, e.clientY, canvasPoint.x, canvasPoint.y);
  });

  function openNewCardMenu(screenX: number, screenY: number, canvasX: number, canvasY: number) {
    const existing = document.getElementById("canvas-context-menu");
    if (existing) existing.remove();

    const menu = el(
      "div",
      {
        id: "canvas-context-menu",
        class: "context-menu",
        style: { left: `${screenX}px`, top: `${screenY}px` },
      },
      el("div", { class: "context-menu-item", dataset: { action: "agent" }, text: "Add Agent Card..." }),
      el("div", { class: "context-menu-item", dataset: { action: "note" }, text: "Add Note Card" }),
      el("div", { class: "context-menu-item", dataset: { action: "shell" }, text: "Add Shell Card" })
    );

    document.body.appendChild(menu);

    const closeMenu = () => menu.remove();
    setTimeout(() => {
      window.addEventListener("click", closeMenu, { once: true });
    }, 10);

    menu.addEventListener("click", async (e) => {
      const action = (e.target as HTMLElement).dataset.action;
      if (!action) return;
      closeMenu();

      if (action === "note") {
        addNoteCard("New note", "# New Note\n\n- Write here...", canvasX, canvasY);
      } else if (action === "shell") {
        const ag = await agentsSpawn({
          workspace: workspaceId,
          name: `shell_${Date.now().toString(36).substring(4)}`,
          preset: "shell",
        });
        addAgentCard(ag.agent, canvasX, canvasY);
      } else if (action === "agent") {
        promptAddAgent(canvasX, canvasY);
      }
    });
  }

  function promptAddAgent(canvasX: number, canvasY: number) {
    const name = window.prompt("Agent name:", `agent_${Date.now().toString(36).substring(4)}`);
    if (!name) return;
    const preset = window.prompt("Preset (agy, claude, shell):", "agy") || "agy";
    const role = window.prompt("Role title (e.g. builder, reviewer):", "builder") || "builder";

    agentsSpawn({
      workspace: workspaceId,
      name,
      preset,
      role,
    }).then((res) => {
      addAgentCard(res.agent, canvasX, canvasY);
    }).catch((err) => {
      alert(`Failed to spawn agent: ${String(err)}`);
    });
  }

  // Delete key deletes selected card or connection
  window.addEventListener("keydown", (e) => {
    if (e.key === "Delete" || e.key === "Backspace") {
      const activeEl = document.activeElement;
      if (activeEl && (activeEl.tagName === "INPUT" || activeEl.tagName === "TEXTAREA")) {
        return;
      }

      if (selectedConnectionId) {
        connections = connections.filter((c) => c.id !== selectedConnectionId);
        selectedConnectionId = null;
        renderConnections();
        debouncedSave();
      } else if (selectedCardId) {
        removeCard(selectedCardId);
      }
    } else if (e.key === "Escape") {
      leaveMaximized();
    } else if (e.ctrlKey && e.key === "0") {
      e.preventDefault();
      fitAllCards();
    }
  });

  function removeCard(cardId: string) {
    cards = cards.filter((c) => c.id !== cardId);
    connections = connections.filter((cn) => cn.from.cardId !== cardId && cn.to.cardId !== cardId);
    const wrapper = cardsContainer.querySelector(`[data-card-id="${cardId}"]`);
    if (wrapper) wrapper.remove();
    const handle = cardHandles.get(cardId);
    if (handle && "dispose" in handle) {
      handle.dispose();
    }
    cardHandles.delete(cardId);
    if (selectedCardId === cardId) selectedCardId = null;
    if (maximizedCardId === cardId) maximizedCardId = null;
    renderConnections();
    updateMinimap();
    debouncedSave();
  }

  function addAgentCard(agent: Agent, x?: number, y?: number) {
    const cardId = `card_agent_${agent.id}`;
    if (cards.some((c) => c.id === cardId)) return;

    const posX = x !== undefined ? x : 100 + (cards.length % 5) * 440;
    const posY = y !== undefined ? y : 80 + Math.floor(cards.length / 5) * 320;

    const card: CanvasCard = {
      id: cardId,
      type: "terminal",
      x: posX,
      y: posY,
      width: 460,
      height: 300,
      title: agent.name,
      agentId: agent.id,
    };

    cards.push(card);

    const handle = createTerminalCard({
      cardId,
      agent,
      isMaximized: maximizedCardId === cardId,
      onToggleMaximize: () => {
        if (maximizedCardId === cardId) {
          maximizedCardId = null;
        } else {
          maximizedCardId = cardId;
        }
        reRenderAllCards();
      },
    });

    cardHandles.set(cardId, handle);
    renderCardElement(card, handle.element);
    applyTransform();
    debouncedSave();
  }

  function addNoteCard(title: string, body = "", x?: number, y?: number) {
    const cardId = `card_note_${Date.now()}_${Math.random().toString(36).substring(2, 6)}`;
    const posX = x !== undefined ? x : 560;
    const posY = y !== undefined ? y : 80;

    const card: CanvasCard = {
      id: cardId,
      type: "note",
      x: posX,
      y: posY,
      width: 380,
      height: 280,
      title,
    };

    cards.push(card);

    const handle = createNoteCardElement(cardId, title, body, (newTitle) => {
      card.title = newTitle;
      debouncedSave();
    });

    cardHandles.set(cardId, handle);
    renderCardElement(card, handle.element);
    applyTransform();
    debouncedSave();
  }

  function reRenderAllCards() {
    clear(cardsContainer);
    for (const card of cards) {
      const handle = cardHandles.get(card.id);
      if (handle) {
        renderCardElement(card, handle.element);
      }
    }
    applyTransform();
  }

  function fitAllCards() {
    const bounds = cards.map((c) => ({
      x: c.x,
      y: c.y,
      width: c.width,
      height: c.height,
    }));
    viewport = fitAll(bounds, container.clientWidth || 800, container.clientHeight || 600, 48);
    applyTransform();
    debouncedSave();
  }

  function focusCardByIndex(index: number) {
    if (index >= 0 && index < cards.length) {
      const card = cards[index];
      selectedCardId = card.id;
      viewport.panX = (container.clientWidth || 800) / 2 - (card.x + card.width / 2) * viewport.zoom;
      viewport.panY = (container.clientHeight || 600) / 2 - (card.y + card.height / 2) * viewport.zoom;
      applyTransform();
      updateSelectionUi();
    }
  }

  function leaveMaximized() {
    if (maximizedCardId) {
      maximizedCardId = null;
      reRenderAllCards();
      debouncedSave();
    }
  }

  // Subscribe to bus messages for pulse effect on connections
  onMessage((msg) => {
    if (!msg.from_agent || !msg.to_agent) return;
    const fromAgentCard = cards.find((c) => c.title === msg.from_agent);
    const toAgentCard = cards.find((c) => c.title === msg.to_agent);
    if (!fromAgentCard || !toAgentCard) return;

    const conn = connections.find(
      (cn) =>
        (cn.from.cardId === fromAgentCard.id && cn.to.cardId === toAgentCard.id) ||
        (cn.from.cardId === toAgentCard.id && cn.to.cardId === fromAgentCard.id)
    );
    if (conn) {
      conn.pulseUntil = Date.now() + 1500;
      renderConnections();
      setTimeout(renderConnections, 1600);
    }
  });

  async function loadLayout(layoutJson: string): Promise<void> {
    const parsedLayout = deserializeLayout(layoutJson);
    viewport = parsedLayout.viewport;
    viewport.zoom = clampZoom(viewport.zoom);
    connections = parsedLayout.connections;

    // Fetch agents and notes from backend / mock
    const [agentsRes, notesRes] = await Promise.all([
      agentsList(workspaceId),
      notesList(),
    ]);

    clear(cardsContainer);
    cardHandles.forEach((h) => {
      if ("dispose" in h) h.dispose();
    });
    cardHandles.clear();

    const matchedCardIds = new Set<string>();

    for (const cardData of parsedLayout.cards) {
      if (cardData.type === "terminal") {
        const agent = agentsRes.agents.find((a) => a.id === cardData.agentId || a.name === cardData.title);
        if (agent) {
          matchedCardIds.add(agent.id);
          const handle = createTerminalCard({
            cardId: cardData.id,
            agent,
            isMaximized: maximizedCardId === cardData.id,
            onToggleMaximize: () => {
              maximizedCardId = maximizedCardId === cardData.id ? null : cardData.id;
              reRenderAllCards();
            },
          });
          cardHandles.set(cardData.id, handle);
          renderCardElement(cardData, handle.element);
        }
      } else if (cardData.type === "note") {
        const note = notesRes.notes.find((n) => n.title === cardData.title);
        const handle = createNoteCardElement(
          cardData.id,
          cardData.title,
          note?.body || "",
          (newTitle) => {
            cardData.title = newTitle;
            debouncedSave();
          }
        );
        cardHandles.set(cardData.id, handle);
        renderCardElement(cardData, handle.element);
      }
    }

    // Add any remaining agents not already placed
    let autoX = 100;
    let autoY = 80;
    for (const agent of agentsRes.agents) {
      if (!matchedCardIds.has(agent.id)) {
        const cardId = `card_agent_${agent.id}`;
        const newCard: CanvasCard = {
          id: cardId,
          type: "terminal",
          x: autoX,
          y: autoY,
          width: 460,
          height: 300,
          title: agent.name,
          agentId: agent.id,
        };
        cards.push(newCard);
        const handle = createTerminalCard({
          cardId,
          agent,
          onToggleMaximize: () => {
            maximizedCardId = maximizedCardId === cardId ? null : cardId;
            reRenderAllCards();
          },
        });
        cardHandles.set(cardId, handle);
        renderCardElement(newCard, handle.element);
        autoX += 480;
      }
    }

    // Add default note if none exist
    if (cards.length > 0 && !cards.some((c) => c.type === "note") && notesRes.notes.length > 0) {
      const note = notesRes.notes[0];
      const cardId = `card_note_${note.id}`;
      const newCard: CanvasCard = {
        id: cardId,
        type: "note",
        x: autoX,
        y: autoY,
        width: 380,
        height: 280,
        title: note.title,
      };
      cards.push(newCard);
      const handle = createNoteCardElement(cardId, note.title, note.body, (newTitle) => {
        newCard.title = newTitle;
        debouncedSave();
      });
      cardHandles.set(cardId, handle);
      renderCardElement(newCard, handle.element);

      // Create a default connection if there are agents
      if (cards.length >= 2) {
        connections.push({
          id: "conn_init_1",
          from: { cardId: cards[0].id, side: "right" },
          to: { cardId: newCard.id, side: "left" },
        });
      }
    }

    // Set cards list
    cards = parsedLayout.cards.length > 0 ? parsedLayout.cards : cards;
    applyTransform();
  }

  // Handle window resize
  window.addEventListener("resize", () => {
    applyTransform();
    cardHandles.forEach((h) => {
      if ("fit" in h) h.fit();
    });
  });

  return {
    element: container,
    loadLayout,
    addAgentCard,
    addNoteCard,
    fitAllCards,
    focusCardByIndex,
    leaveMaximized,
    hasMaximizedCard: () => Boolean(maximizedCardId),
    saveNow: () => debouncedSave.flush(),
    dispose: () => {
      cardHandles.forEach((h) => {
        if ("dispose" in h) h.dispose();
      });
      cardHandles.clear();
    },
  };
}
