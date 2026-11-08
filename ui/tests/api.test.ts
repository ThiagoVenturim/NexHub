import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  agentsList,
  agentsSpawn,
  agentsStop,
  doctorCheck,
  floorsCreate,
  floorsList,
  floorsMerge,
  floorsRemove,
  groupsList,
  layoutSave,
  msgAsk,
  msgBroadcast,
  msgInbox,
  msgNotify,
  msgReply,
  notesGet,
  notesList,
  notesSet,
  onAgentState,
  onFloorChanged,
  onMessage,
  onNoteChanged,
  onPtyOutput,
  onRoutineRun,
  ptyResize,
  ptyWrite,
  routinesAdd,
  routinesList,
  routinesRemove,
  routinesRun,
  setCustomInvoker,
  setCustomListener,
  status,
  workspaceAdd,
  workspacesList,
} from "../src/api";

describe("api.ts typed wrappers and command contract", () => {
  let calls: Array<{ cmd: string; args?: Record<string, unknown> }>;

  beforeEach(() => {
    calls = [];
    setCustomInvoker((cmd, args) => {
      calls.push({ cmd, args });
      return Promise.resolve({});
    });
  });

  afterEach(() => {
    setCustomInvoker(null);
    setCustomListener(null);
  });

  it("invokes agents commands with exact Section 11 / Section 7 names and args", async () => {
    await agentsList("ws_1");
    expect(calls[0]).toEqual({ cmd: "agents_list", args: { workspace: "ws_1" } });

    await agentsSpawn({
      workspace: "ws_1",
      name: "builder",
      preset: "agy",
      role: "builder",
      floor: "fl_semantic",
    });
    expect(calls[1]).toEqual({
      cmd: "agents_spawn",
      args: {
        workspace: "ws_1",
        name: "builder",
        preset: "agy",
        role: "builder",
        floor: "fl_semantic",
      },
    });

    await agentsStop("builder");
    expect(calls[2]).toEqual({ cmd: "agents_stop", args: { name: "builder" } });
  });

  it("invokes bus message commands with exact names and args", async () => {
    await msgAsk("reviewer", "Need review", 300);
    expect(calls[0]).toEqual({
      cmd: "msg_ask",
      args: { to: "reviewer", body: "Need review", timeout_s: 300 },
    });

    await msgNotify("builder", "Tests passed");
    expect(calls[1]).toEqual({
      cmd: "msg_notify",
      args: { to: "builder", body: "Tests passed" },
    });

    await msgBroadcast("All hands sync");
    expect(calls[2]).toEqual({
      cmd: "msg_broadcast",
      args: { body: "All hands sync" },
    });

    await msgReply("msg_123", "Acknowledged");
    expect(calls[3]).toEqual({
      cmd: "msg_reply",
      args: { message_id: "msg_123", body: "Acknowledged" },
    });

    await msgInbox();
    expect(calls[4]).toEqual({
      cmd: "msg_inbox",
      args: {},
    });
  });

  it("invokes notes commands with exact names and args", async () => {
    await notesGet("board");
    expect(calls[0]).toEqual({ cmd: "notes_get", args: { title: "board" } });

    await notesSet("board", "# Content");
    expect(calls[1]).toEqual({
      cmd: "notes_set",
      args: { title: "board", body: "# Content" },
    });

    await notesList();
    expect(calls[2]).toEqual({ cmd: "notes_list", args: {} });
  });

  it("invokes floors commands with exact names and args", async () => {
    await floorsCreate("ws_1", "ui", "origin/develop");
    expect(calls[0]).toEqual({
      cmd: "floors_create",
      args: { workspace: "ws_1", name: "ui", base_ref: "origin/develop" },
    });

    await floorsList("ws_1");
    expect(calls[1]).toEqual({ cmd: "floors_list", args: { workspace: "ws_1" } });

    await floorsRemove("ws_1", "ui", true);
    expect(calls[2]).toEqual({
      cmd: "floors_remove",
      args: { workspace: "ws_1", name: "ui", force: true },
    });

    await floorsMerge("ws_1", "ui", "develop");
    expect(calls[3]).toEqual({
      cmd: "floors_merge",
      args: { workspace: "ws_1", name: "ui", into: "develop" },
    });
  });

  it("invokes routines commands with exact names and args", async () => {
    await routinesList();
    expect(calls[0]).toEqual({ cmd: "routines_list", args: {} });

    await routinesAdd({
      name: "watchdog",
      schedule: "every 15m",
      command: "git status",
      body: "",
    });
    expect(calls[1]).toEqual({
      cmd: "routines_add",
      args: { name: "watchdog", schedule: "every 15m", command: "git status", body: "" },
    });

    await routinesRemove("watchdog");
    expect(calls[2]).toEqual({ cmd: "routines_remove", args: { name: "watchdog" } });

    await routinesRun("watchdog");
    expect(calls[3]).toEqual({ cmd: "routines_run", args: { name: "watchdog" } });
  });

  it("invokes pty, layout and diagnostic commands", async () => {
    await ptyWrite("agent_1", "b64data==");
    expect(calls[0]).toEqual({
      cmd: "pty_write",
      args: { agent_id: "agent_1", data_b64: "b64data==" },
    });

    await ptyResize("agent_1", 120, 30);
    expect(calls[1]).toEqual({
      cmd: "pty_resize",
      args: { agent_id: "agent_1", cols: 120, rows: 30 },
    });

    await layoutSave("ws_1", '{"cards":[]}');
    expect(calls[2]).toEqual({
      cmd: "layout_save",
      args: { workspace_id: "ws_1", layout_json: '{"cards":[]}' },
    });

    await status();
    expect(calls[3]).toEqual({ cmd: "status", args: {} });

    await doctorCheck();
    expect(calls[4]).toEqual({ cmd: "doctor_check", args: {} });

    await workspaceAdd({ name: "askdash", repo_path: "/repos/askdash", group_id: "grp_1" });
    expect(calls[5]).toEqual({
      cmd: "workspace_add",
      args: { name: "askdash", repo_path: "/repos/askdash", group_id: "grp_1" },
    });

    await workspacesList();
    expect(calls[6]).toEqual({ cmd: "workspaces_list", args: {} });

    await groupsList();
    expect(calls[7]).toEqual({ cmd: "groups_list", args: {} });
  });

  it("registers listeners for all Section 11 events", async () => {
    const listenedEvents: string[] = [];
    const handlers = new Map<string, (evt: { payload: unknown }) => void>();

    setCustomListener((event, handler) => {
      listenedEvents.push(event);
      handlers.set(event, handler);
      return Promise.resolve(() => {});
    });

    const ptyCb = vi.fn();
    const stateCb = vi.fn();
    const msgCb = vi.fn();
    const noteCb = vi.fn();
    const routineCb = vi.fn();
    const floorCb = vi.fn();

    await onPtyOutput(ptyCb);
    await onAgentState(stateCb);
    await onMessage(msgCb);
    await onNoteChanged(noteCb);
    await onRoutineRun(routineCb);
    await onFloorChanged(floorCb);

    expect(listenedEvents).toEqual([
      "pty-output",
      "agent-state",
      "message",
      "note-changed",
      "routine-run",
      "floor-changed",
    ]);

    // Dispatch mock payloads
    handlers.get("pty-output")!({ payload: { agent_id: "ag_1", data_b64: "aGVsbG8=" } });
    expect(ptyCb).toHaveBeenCalledWith({ agent_id: "ag_1", data_b64: "aGVsbG8=" });

    handlers.get("agent-state")!({ payload: { agent_id: "ag_1", state: "running" } });
    expect(stateCb).toHaveBeenCalledWith({ agent_id: "ag_1", state: "running" });

    handlers.get("message")!({
      payload: { id: "m1", workspace_id: "w1", from: "a", to: "b", kind: "ask", body: "x", state: "delivered" },
    });
    expect(msgCb).toHaveBeenCalled();

    handlers.get("note-changed")!({ payload: { workspace_id: "w1", title: "board" } });
    expect(noteCb).toHaveBeenCalledWith({ workspace_id: "w1", title: "board" });

    handlers.get("routine-run")!({ payload: { routine_id: "r1", run_id: "run1", status: "ok" } });
    expect(routineCb).toHaveBeenCalledWith({ routine_id: "r1", run_id: "run1", status: "ok" });

    handlers.get("floor-changed")!({ payload: { workspace_id: "w1", floor: { name: "ui" } } });
    expect(floorCb).toHaveBeenCalledWith({ workspace_id: "w1", floor: { name: "ui" } });
  });
});
