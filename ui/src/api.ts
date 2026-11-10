import type {
  Agent,
  AgentState,
  DoctorResult,
  Floor,
  Group,
  Message,
  Note,
  Routine,
  StatusResult,
  Workspace,
} from "./types";

export type InvokeHandler = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
export type EventListener = (event: string, handler: (event: { payload: unknown }) => void) => Promise<() => void>;

let customInvoker: InvokeHandler | null = null;
let customListener: EventListener | null = null;

export function setCustomInvoker(fn: InvokeHandler | null): void {
  customInvoker = fn;
}

export function setCustomListener(fn: EventListener | null): void {
  customListener = fn;
}

export function isMockMode(): boolean {
  if (customInvoker !== null) {
    return false;
  }
  const isViteMock = import.meta.env.VITE_NEXHUB_MOCK === "1";
  if (isViteMock) {
    return true;
  }
  if (typeof window === "undefined") {
    return true;
  }
  const win = window as unknown as { __TAURI_INTERNALS__?: unknown; __TAURI__?: unknown };
  return !win.__TAURI_INTERNALS__ && !win.__TAURI__;
}

// In-memory mock database state
const mockState = {
  groups: [
    { id: "grp_1", name: "OSS", position: 0 },
    { id: "grp_2", name: "Pessoais", position: 1 },
  ] as Group[],
  workspaces: [
    {
      id: "ws_default",
      name: "askdash",
      repo_path: "C:/Users/thiag/projetos-oss/askdash",
      group_id: "grp_1",
      layout_json: "{}",
      created_at: 1731000000000,
    },
    {
      id: "ws_support",
      name: "support",
      repo_path: "C:/Users/thiag/projetos-oss/Support_Intelligence_Engine",
      group_id: "grp_1",
      layout_json: "{}",
      created_at: 1731001000000,
    },
    {
      id: "ws_nextor",
      name: "nextor",
      repo_path: "C:/Users/thiag/projetos/nextor",
      group_id: "grp_2",
      layout_json: "{}",
      created_at: 1731002000000,
    },
  ] as Workspace[],
  floors: [
    {
      id: "fl_semantic",
      workspace_id: "ws_default",
      name: "semantic",
      branch: "layer/semantic",
      path: "C:/Users/thiag/projetos-oss/.nexhub-floors/askdash--semantic",
      base_ref: "origin/develop",
      status: "active" as const,
      created_at: 1731000000000,
    },
  ] as Floor[],
  agents: [
    {
      id: "agent_builder",
      workspace_id: "ws_default",
      floor_id: "fl_semantic",
      name: "builder",
      preset: "agy",
      role: "builder",
      cwd: "C:/Users/thiag/projetos-oss/.nexhub-floors/askdash--semantic",
      state: "busy" as AgentState,
      quota_reset_at: null,
      last_prompt: "Implement vector search index",
      created_at: 1731000000000,
    },
    {
      id: "agent_reviewer",
      workspace_id: "ws_default",
      floor_id: "fl_semantic",
      name: "reviewer",
      preset: "claude",
      role: "reviewer",
      cwd: "C:/Users/thiag/projetos-oss/.nexhub-floors/askdash--semantic",
      state: "running" as AgentState,
      quota_reset_at: null,
      last_prompt: "Review diff on layer/semantic",
      created_at: 1731000500000,
    },
    {
      id: "agent_conductor",
      workspace_id: "ws_default",
      floor_id: null,
      name: "conductor",
      preset: "claude",
      role: "conductor",
      cwd: "C:/Users/thiag/projetos-oss/askdash",
      state: "running" as AgentState,
      quota_reset_at: null,
      last_prompt: null,
      created_at: 1731000100000,
    },
  ] as Agent[],
  messages: [
    {
      id: "msg_1",
      workspace_id: "ws_default",
      from_agent: "reviewer",
      to_agent: "builder",
      kind: "ask" as const,
      reply_to: null,
      body: "PR ready?",
      state: "answered" as const,
      created_at: 1731000800000,
      delivered_at: 1731000801000,
      answered_at: 1731000830000,
    },
    {
      id: "msg_2",
      workspace_id: "ws_default",
      from_agent: "builder",
      to_agent: "reviewer",
      kind: "reply" as const,
      reply_to: "msg_1",
      body: "Running tests now; will signal once tests pass",
      state: "delivered" as const,
      created_at: 1731000830000,
      delivered_at: 1731000831000,
      answered_at: null,
    },
  ] as Message[],
  notes: [
    {
      id: "note_board",
      workspace_id: "ws_default",
      title: "board",
      body: "# Task Board\n\n- [x] Initial scaffold\n- [x] Canvas coordinate transforms\n- [ ] Terminal replay and pty bridge\n- [ ] Review PR before merge\n",
      updated_at: 1731000900000,
    },
  ] as Note[],
  routines: [
    {
      id: "rt_watchdog",
      workspace_id: "ws_default",
      name: "watchdog",
      schedule: "every 15m",
      target_agent: null,
      command: "git status",
      body: "",
      enabled: 1,
      next_run_at: 1731001800000,
    },
  ] as Routine[],
};

const mockEventSubscribers = new Map<string, Set<(payload: unknown) => void>>();

export function getMockState() {
  return mockState;
}

export function emitMockEvent(event: string, payload: unknown): void {
  const listeners = mockEventSubscribers.get(event);
  if (listeners) {
    listeners.forEach((fn) => {
      try {
        fn(payload);
      } catch (err) {
        console.error("Mock listener error", err);
      }
    });
  }
}

async function invokeTauri<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (customInvoker) {
    return (await customInvoker(cmd, args)) as T;
  }

  if (isMockMode()) {
    return handleMockCommand<T>(cmd, args);
  }

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

function handleMockCommand<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const safeArgs = args || {};

  switch (cmd) {
    case "agents_list": {
      const ws = safeArgs.workspace as string | undefined;
      const list = ws ? mockState.agents.filter((a) => a.workspace_id === ws) : mockState.agents;
      return Promise.resolve({ agents: list } as unknown as T);
    }
    case "agents_spawn": {
      const workspace = (safeArgs.workspace as string) || "ws_default";
      const name = (safeArgs.name as string) || `agent_${Date.now()}`;
      const preset = (safeArgs.preset as string) || "shell";
      const role = safeArgs.role as string | undefined;
      const floor = safeArgs.floor as string | undefined;

      const newAgent: Agent = {
        id: `agent_${Date.now()}_${Math.random().toString(36).substring(2, 6)}`,
        workspace_id: workspace,
        floor_id: floor || null,
        name,
        preset,
        role: role || null,
        cwd: "C:/Users/thiag/projetos-oss/askdash",
        state: "running",
        quota_reset_at: null,
        last_prompt: null,
        created_at: Date.now(),
      };
      mockState.agents.push(newAgent);
      emitMockEvent("agent-state", { agent_id: newAgent.id, state: newAgent.state });
      return Promise.resolve({ agent: newAgent } as unknown as T);
    }
    case "agents_stop": {
      const name = safeArgs.name as string;
      const ag = mockState.agents.find((a) => a.name === name);
      if (ag) {
        ag.state = "stopped";
        emitMockEvent("agent-state", { agent_id: ag.id, state: ag.state });
      }
      return Promise.resolve({} as unknown as T);
    }
    case "msg_ask": {
      const to = safeArgs.to as string;
      const body = safeArgs.body as string;
      const msg: Message = {
        id: `msg_${Date.now()}`,
        workspace_id: "ws_default",
        from_agent: null,
        to_agent: to,
        kind: "ask",
        reply_to: null,
        body,
        state: "answered",
        created_at: Date.now(),
        delivered_at: Date.now(),
        answered_at: Date.now(),
      };
      mockState.messages.push(msg);
      emitMockEvent("message", msg);
      return Promise.resolve({
        reply: {
          id: `reply_${Date.now()}`,
          from: to,
          body: `Mock reply from ${to} to: ${body}`,
        },
      } as unknown as T);
    }
    case "msg_notify": {
      const to = safeArgs.to as string;
      const body = safeArgs.body as string;
      const id = `msg_${Date.now()}`;
      const msg: Message = {
        id,
        workspace_id: "ws_default",
        from_agent: null,
        to_agent: to,
        kind: "notify",
        reply_to: null,
        body,
        state: "delivered",
        created_at: Date.now(),
        delivered_at: Date.now(),
        answered_at: null,
      };
      mockState.messages.push(msg);
      emitMockEvent("message", msg);
      return Promise.resolve({ message_id: id } as unknown as T);
    }
    case "msg_broadcast": {
      const body = safeArgs.body as string;
      const ids: string[] = [];
      mockState.agents.forEach((ag) => {
        const id = `msg_bc_${ag.id}_${Date.now()}`;
        ids.push(id);
        const msg: Message = {
          id,
          workspace_id: "ws_default",
          from_agent: null,
          to_agent: ag.name,
          kind: "broadcast",
          reply_to: null,
          body,
          state: "delivered",
          created_at: Date.now(),
          delivered_at: Date.now(),
          answered_at: null,
        };
        mockState.messages.push(msg);
        emitMockEvent("message", msg);
      });
      return Promise.resolve({ message_ids: ids } as unknown as T);
    }
    case "msg_reply": {
      const message_id = safeArgs.message_id as string;
      const body = safeArgs.body as string;
      const target = mockState.messages.find((m) => m.id === message_id);
      if (target) {
        target.state = "answered";
        target.answered_at = Date.now();
      }
      const replyMsg: Message = {
        id: `msg_reply_${Date.now()}`,
        workspace_id: "ws_default",
        from_agent: target?.to_agent || null,
        to_agent: target?.from_agent || null,
        kind: "reply",
        reply_to: message_id,
        body,
        state: "delivered",
        created_at: Date.now(),
        delivered_at: Date.now(),
        answered_at: null,
      };
      mockState.messages.push(replyMsg);
      emitMockEvent("message", replyMsg);
      return Promise.resolve({} as unknown as T);
    }
    case "msg_inbox": {
      return Promise.resolve({ messages: mockState.messages } as unknown as T);
    }
    case "notes_get": {
      const title = safeArgs.title as string;
      const note = mockState.notes.find((n) => n.title === title) || {
        id: `note_${Date.now()}`,
        workspace_id: "ws_default",
        title,
        body: "",
        updated_at: Date.now(),
      };
      return Promise.resolve(note as unknown as T);
    }
    case "notes_set": {
      const title = safeArgs.title as string;
      const body = (safeArgs.body as string) || "";
      const existing = mockState.notes.find((n) => n.title === title);
      if (existing) {
        existing.body = body;
        existing.updated_at = Date.now();
      } else {
        mockState.notes.push({
          id: `note_${Date.now()}`,
          workspace_id: "ws_default",
          title,
          body,
          updated_at: Date.now(),
        });
      }
      emitMockEvent("note-changed", { workspace_id: "ws_default", title });
      return Promise.resolve({} as unknown as T);
    }
    case "notes_list": {
      return Promise.resolve({ notes: mockState.notes } as unknown as T);
    }
    case "floors_create": {
      const workspace = (safeArgs.workspace as string) || "ws_default";
      const name = safeArgs.name as string;
      const base_ref = (safeArgs.base_ref as string) || "origin/develop";
      const floor: Floor = {
        id: `fl_${name}`,
        workspace_id: workspace,
        name,
        branch: `layer/${name}`,
        path: `C:/Users/thiag/projetos-oss/.nexhub-floors/askdash--${name}`,
        base_ref,
        status: "active",
        created_at: Date.now(),
      };
      mockState.floors.push(floor);
      emitMockEvent("floor-changed", { workspace_id: workspace, floor });
      return Promise.resolve({} as unknown as T);
    }
    case "floors_list": {
      const workspace = safeArgs.workspace as string;
      const list = workspace ? mockState.floors.filter((f) => f.workspace_id === workspace) : mockState.floors;
      return Promise.resolve({ floors: list } as unknown as T);
    }
    case "floors_remove": {
      const name = safeArgs.name as string;
      const f = mockState.floors.find((fl) => fl.name === name);
      if (f) {
        f.status = "removed";
        emitMockEvent("floor-changed", { workspace_id: f.workspace_id, floor: f });
      }
      return Promise.resolve({} as unknown as T);
    }
    case "floors_merge": {
      const name = safeArgs.name as string;
      const f = mockState.floors.find((fl) => fl.name === name);
      if (f) {
        f.status = "merged";
        emitMockEvent("floor-changed", { workspace_id: f.workspace_id, floor: f });
      }
      return Promise.resolve({} as unknown as T);
    }
    case "routines_list": {
      return Promise.resolve({ routines: mockState.routines } as unknown as T);
    }
    case "routines_add": {
      const name = safeArgs.name as string;
      const schedule = safeArgs.schedule as string;
      const body = (safeArgs.body as string) || "";
      const routine: Routine = {
        id: `rt_${Date.now()}`,
        workspace_id: "ws_default",
        name,
        schedule,
        target_agent: (safeArgs.target as string) || null,
        command: (safeArgs.command as string) || null,
        body,
        enabled: 1,
        next_run_at: Date.now() + 60000,
      };
      mockState.routines.push(routine);
      return Promise.resolve({} as unknown as T);
    }
    case "routines_remove": {
      const name = safeArgs.name as string;
      const idx = mockState.routines.findIndex((r) => r.name === name);
      if (idx >= 0) {
        mockState.routines.splice(idx, 1);
      }
      return Promise.resolve({} as unknown as T);
    }
    case "routines_run": {
      const name = safeArgs.name as string;
      const r = mockState.routines.find((rt) => rt.name === name);
      if (r) {
        emitMockEvent("routine-run", {
          routine_id: r.id,
          run_id: `run_${Date.now()}`,
          status: "success",
        });
      }
      return Promise.resolve({} as unknown as T);
    }
    case "status": {
      const runningCount = mockState.agents.filter((a) => a.state === "running" || a.state === "busy").length;
      const queuedCount = mockState.messages.filter((m) => m.state === "queued").length;
      return Promise.resolve({
        version: "0.1.0",
        workspaces: mockState.workspaces.length,
        agents_running: runningCount,
        queued_messages: queuedCount,
      } as unknown as T);
    }
    case "pty_write": {
      const agent_id = safeArgs.agent_id as string;
      const data_b64 = safeArgs.data_b64 as string;
      if (typeof window !== "undefined") {
        try {
          const text = atob(data_b64);
          if (text === "\r" || text === "\n") {
            const echoB64 = btoa("\r\n[mock: command acknowledged]\r\n$ ");
            emitMockEvent("pty-output", { agent_id, data_b64: echoB64 });
          } else {
            // Echo characters back
            emitMockEvent("pty-output", { agent_id, data_b64 });
          }
        } catch {
          // Ignore decode errors
        }
      }
      return Promise.resolve(undefined as unknown as T);
    }
    case "pty_resize": {
      return Promise.resolve(undefined as unknown as T);
    }
    case "layout_save": {
      const workspace_id = (safeArgs.workspace_id as string) || "ws_default";
      const layout_json = (safeArgs.layout_json as string) || "{}";
      const ws = mockState.workspaces.find((w) => w.id === workspace_id);
      if (ws) {
        ws.layout_json = layout_json;
      }
      return Promise.resolve(undefined as unknown as T);
    }
    case "doctor_check": {
      return Promise.resolve({
        git: true,
        agy: true,
        claude: true,
        gh: true,
        node: true,
        details: {
          git: "git version 2.44.0.windows.1",
          agy: "agy 2.0.0",
          claude: "claude-code 0.2.9",
          gh: "gh version 2.50.0",
          node: "v24.20.0",
        },
      } as unknown as T);
    }
    case "workspace_add": {
      const name = safeArgs.name as string;
      const repo_path = safeArgs.repo_path as string;
      const group_id = (safeArgs.group_id as string) || null;
      const ws: Workspace = {
        id: `ws_${Date.now()}`,
        name,
        repo_path,
        group_id,
        layout_json: "{}",
        created_at: Date.now(),
      };
      mockState.workspaces.push(ws);
      return Promise.resolve({ workspace: ws } as unknown as T);
    }
    case "workspaces_list": {
      return Promise.resolve({ workspaces: mockState.workspaces } as unknown as T);
    }
    case "groups_list": {
      return Promise.resolve({ groups: mockState.groups } as unknown as T);
    }
    default:
      return Promise.reject(new Error(`Unknown command: ${cmd}`));
  }
}

async function listenTauri<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  if (customListener) {
    return customListener(event, (evt) => handler(evt.payload as T));
  }

  if (isMockMode()) {
    if (!mockEventSubscribers.has(event)) {
      mockEventSubscribers.set(event, new Set());
    }
    const set = mockEventSubscribers.get(event)!;
    const wrappedHandler = (payload: unknown) => handler(payload as T);
    set.add(wrappedHandler);
    return () => {
      set.delete(wrappedHandler);
    };
  }

  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<T>(event, (evt) => {
    handler(evt.payload);
  });
  return unlisten;
}

// Typed API Functions

export async function agentsList(workspace?: string): Promise<{ agents: Agent[] }> {
  return invokeTauri<{ agents: Agent[] }>("agents_list", { workspace });
}

export async function agentsSpawn(params: {
  workspace: string;
  name: string;
  preset: string;
  role?: string;
  floor?: string;
}): Promise<{ agent: Agent }> {
  return invokeTauri<{ agent: Agent }>("agents_spawn", params);
}

export async function agentsStop(name: string): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("agents_stop", { name });
}

export async function msgAsk(
  to: string,
  body: string,
  timeout_s?: number
): Promise<{ reply: { id: string; from: string; body: string } }> {
  return invokeTauri<{ reply: { id: string; from: string; body: string } }>("msg_ask", {
    to,
    body,
    timeout_s,
  });
}

export async function msgNotify(to: string, body: string): Promise<{ message_id: string }> {
  return invokeTauri<{ message_id: string }>("msg_notify", { to, body });
}

export async function msgBroadcast(body: string): Promise<{ message_ids: string[] }> {
  return invokeTauri<{ message_ids: string[] }>("msg_broadcast", { body });
}

export async function msgReply(message_id: string, body: string): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("msg_reply", { message_id, body });
}

export async function msgInbox(): Promise<{ messages: Message[] }> {
  return invokeTauri<{ messages: Message[] }>("msg_inbox", {});
}

export async function notesGet(title: string): Promise<Note> {
  return invokeTauri<Note>("notes_get", { title });
}

export async function notesSet(title: string, body: string): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("notes_set", { title, body });
}

export async function notesList(): Promise<{ notes: Note[] }> {
  return invokeTauri<{ notes: Note[] }>("notes_list", {});
}

export async function floorsCreate(
  workspace: string,
  name: string,
  base_ref?: string
): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("floors_create", {
    workspace,
    name,
    base_ref,
  });
}

export async function floorsList(workspace: string): Promise<{ floors: Floor[] }> {
  return invokeTauri<{ floors: Floor[] }>("floors_list", { workspace });
}

export async function floorsRemove(
  workspace: string,
  name: string,
  force?: boolean
): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("floors_remove", {
    workspace,
    name,
    force,
  });
}

export async function floorsMerge(
  workspace: string,
  name: string,
  into: string
): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("floors_merge", {
    workspace,
    name,
    into,
  });
}

export async function routinesList(): Promise<{ routines: Routine[] }> {
  return invokeTauri<{ routines: Routine[] }>("routines_list", {});
}

export async function routinesAdd(params: {
  name: string;
  schedule: string;
  target?: string;
  command?: string;
  body: string;
}): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("routines_add", params);
}

export async function routinesRemove(name: string): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("routines_remove", { name });
}

export async function routinesRun(name: string): Promise<Record<string, never>> {
  return invokeTauri<Record<string, never>>("routines_run", { name });
}

export async function status(): Promise<StatusResult> {
  return invokeTauri<StatusResult>("status", {});
}

export async function ptyWrite(agent_id: string, data_b64: string): Promise<void> {
  return invokeTauri<void>("pty_write", { agent_id, data_b64 });
}

export async function ptyResize(agent_id: string, cols: number, rows: number): Promise<void> {
  return invokeTauri<void>("pty_resize", { agent_id, cols, rows });
}

export async function layoutSave(workspace_id: string, layout_json: string): Promise<void> {
  return invokeTauri<void>("layout_save", { workspace_id, layout_json });
}

export async function doctorCheck(): Promise<DoctorResult> {
  return invokeTauri<DoctorResult>("doctor_check", {});
}

export async function workspaceAdd(params: {
  name: string;
  repo_path: string;
  group_id?: string;
}): Promise<{ workspace: Workspace }> {
  return invokeTauri<{ workspace: Workspace }>("workspace_add", params);
}

export async function workspacesList(): Promise<{ workspaces: Workspace[] }> {
  return invokeTauri<{ workspaces: Workspace[] }>("workspaces_list", {});
}

export async function groupsList(): Promise<{ groups: Group[] }> {
  return invokeTauri<{ groups: Group[] }>("groups_list", {});
}

// Event Listeners

export function onPtyOutput(
  cb: (payload: { agent_id: string; data_b64: string }) => void
): Promise<() => void> {
  return listenTauri<{ agent_id: string; data_b64: string }>("pty-output", cb);
}

export function onAgentState(
  cb: (payload: { agent_id: string; state: AgentState; quota_reset_at?: number }) => void
): Promise<() => void> {
  return listenTauri<{ agent_id: string; state: AgentState; quota_reset_at?: number }>(
    "agent-state",
    cb
  );
}

export function onMessage(
  cb: (payload: Message) => void
): Promise<() => void> {
  return listenTauri<Message>("message", cb);
}

export function onNoteChanged(
  cb: (payload: { workspace_id: string; title: string }) => void
): Promise<() => void> {
  return listenTauri<{ workspace_id: string; title: string }>("note-changed", cb);
}

export function onRoutineRun(
  cb: (payload: { routine_id: string; run_id: string; status: string }) => void
): Promise<() => void> {
  return listenTauri<{ routine_id: string; run_id: string; status: string }>(
    "routine-run",
    cb
  );
}

export function onFloorChanged(
  cb: (payload: { workspace_id: string; floor: Floor }) => void
): Promise<() => void> {
  return listenTauri<{ workspace_id: string; floor: Floor }>("floor-changed", cb);
}
