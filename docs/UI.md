# NexHub UI

Own identity, not a copy of any other product. Calm, dense, made for long sessions. Built with vanilla TypeScript modules, xterm.js (`@xterm/xterm` + `@xterm/addon-fit`), Vite.

## Visual identity

- Name in the title bar: "NexHub". Same family as Nextor and NexDeck.
- Colors as CSS custom properties in `ui/src/styles/tokens.css`. Dark default, light theme via `[data-theme="light"]`.
  - background `#0f1216`, surface `#171b21`, surface-2 `#1f252d`, border `#2a313b`, text `#e6e9ee`, muted `#8b95a3`
  - accent (teal) `#2bb3a3`, warning (amber) `#e0a43a`, danger `#e5534b`, ok `#4caf7a`
  - light theme: background `#f6f7f9`, surface `#ffffff`, text `#1c2128`, same accent darkened to `#1f8f83`
- Type: system UI font for chrome, `"Cascadia Mono", "JetBrains Mono", monospace` for terminals, 13px.
- Canvas background: subtle dot grid (1px dots every 24px at 100% zoom), scales with zoom.
- No emoji anywhere. Icons are inline SVG (a small set in `ui/src/icons.ts`), 16px, stroke 1.5.

## Layout

```
+----------------------------------------------------------------------------+
| NexHub   [workspace name]                 [search]   [status: 3 running]   |
+-----------+----------------------------------------------------------------+
| Groups    |                                                                |
|  OSS      |     +-- builder (agy) ------ busy --+    +-- note: board --+    |
|   askdash |     | xterm                         |--->| markdown        |    |
|   support |     |                               |    +-----------------+    |
|  Pessoais |     +-------------------------------+                           |
|   nextor  |                 +-- reviewer (claude) -- idle --+               |
| Floors    |                 | xterm                         |               |
|  semantic |                 +-------------------------------+               |
| Routines  |                                                                |
|  watchdog |                                                    [minimap]   |
+-----------+----------------------------------------------------------------+
| Message log (collapsible): 10:42 reviewer -> builder ask "PR ready?"        |
+----------------------------------------------------------------------------+
```

## Canvas behavior

- Pan: drag on empty space, or space + drag, or two-finger scroll. Zoom: ctrl + wheel, pinch; range 25% to 200%; zoom around the cursor.
- Cards: drag by the header, resize from the bottom-right corner (min 360x220 for terminals). Positions are in canvas coordinates and saved to `workspaces.layout_json` (debounced 500 ms) via `layout_save`.
- Connections: drag from the small port on a card edge to another card. A connection between two agents allows messages between them and shows a short pulse when a message passes. Delete with select + Delete key.
- Double-click empty space opens the "new card" menu: Agent (choose preset, role, floor), Note, Shell.
- Minimap bottom-right; click to jump.

## Terminal card

- Header: agent name, preset badge, role title, floor badge, state pill (stopped grey, starting muted, running teal outline, busy teal filled with a slow pulse, waiting_quota amber with the reset time, exited grey, error red).
- Header actions: restart, stop, send message (opens a small composer that calls `msg_notify`), maximize (fills the canvas viewport, Esc returns).
- Body: xterm.js with fit addon; resize triggers `pty_resize`. Scrollback 10 000 lines. Replay ring buffer on mount.

## Note card

Markdown text area with preview toggle; saves via `notes.set` after 800 ms idle. Title editable.

## Sidebar

- Groups (collapsible) with workspaces; right-click: rename, move to group, remove (does not delete files).
- Floors of the current workspace with status; actions: new, merge into develop, remove.
- Routines with next run time and a toggle.

## First-run assistant

Shown when the database has no workspace. Steps: 1) detect tools (`nexhub doctor` logic: git, agy, claude, gh, node) and show found/missing with install hints; 2) pick a repository folder; 3) pick a starting team (Solo: one agy agent; Pair: builder + reviewer; Floor team: conductor + 2 builders); 4) open the canvas with those cards placed.

## Keyboard

`Ctrl+K` command palette (spawn agent, new note, new floor, jump to card, send message), `Ctrl+1..9` focus card by order, `Ctrl+0` fit all, `Esc` leave maximized card.

## Accessibility

Focus rings visible, all actions reachable by keyboard, state pills have text not just color, contrast at least 4.5:1 for text.
