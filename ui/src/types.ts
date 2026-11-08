export type AgentState =
  | "stopped"
  | "starting"
  | "running"
  | "busy"
  | "waiting_quota"
  | "exited"
  | "error";

export type MessageKind = "ask" | "notify" | "broadcast" | "reply";
export type MessageState = "queued" | "delivered" | "answered" | "expired" | "failed";
export type FloorStatus = "active" | "merged" | "removed";

export interface Group {
  id: string;
  name: string;
  position: number;
}

export interface Workspace {
  id: string;
  name: string;
  repo_path: string;
  group_id: string | null;
  layout_json: string;
  created_at: number;
}

export interface Floor {
  id: string;
  workspace_id: string;
  name: string;
  branch: string;
  path: string;
  base_ref: string;
  status: FloorStatus;
  created_at: number;
}

export interface Agent {
  id: string;
  workspace_id: string;
  floor_id?: string | null;
  name: string;
  preset: string;
  role?: string | null;
  cwd: string;
  state: AgentState;
  quota_reset_at?: number | null;
  last_prompt?: string | null;
  created_at: number;
}

export interface Message {
  id: string;
  workspace_id: string;
  from_agent: string | null;
  to_agent: string | null;
  kind: MessageKind;
  reply_to?: string | null;
  body: string;
  state: MessageState;
  created_at: number;
  delivered_at?: number | null;
  answered_at?: number | null;
}

export interface Note {
  id: string;
  workspace_id: string;
  title: string;
  body: string;
  updated_at: number;
}

export interface Connection {
  workspace_id: string;
  from_id: string;
  to_id: string;
}

export interface Routine {
  id: string;
  workspace_id: string;
  name: string;
  schedule: string;
  target_agent?: string | null;
  command?: string | null;
  body: string;
  enabled: number;
  next_run_at?: number | null;
}

export interface Run {
  id: string;
  routine_id: string;
  started_at: number;
  finished_at?: number | null;
  exit_code?: number | null;
  output_tail?: string | null;
}

export interface DoctorResult {
  git: boolean;
  agy: boolean;
  claude: boolean;
  gh: boolean;
  node: boolean;
  details: Record<string, string>;
}

export interface StatusResult {
  version: string;
  workspaces: number;
  agents_running: number;
  queued_messages: number;
}

export type PortSide = "left" | "right" | "top" | "bottom";

export interface PortRef {
  cardId: string;
  side: PortSide;
}

export interface CanvasCard {
  id: string;
  type: "terminal" | "note";
  x: number;
  y: number;
  width: number;
  height: number;
  title: string;
  agentId?: string;
  maximized?: boolean;
}

export interface CanvasConnection {
  id: string;
  from: PortRef;
  to: PortRef;
  pulseUntil?: number;
}

export interface CanvasLayout {
  viewport: {
    panX: number;
    panY: number;
    zoom: number;
  };
  cards: CanvasCard[];
  connections: CanvasConnection[];
}
