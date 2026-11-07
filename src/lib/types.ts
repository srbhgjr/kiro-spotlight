export type Kind = "light" | "dark";

export interface Palette {
  kind: Kind;
  bg: string;
  fg: string;
  fgMuted: string;
  accent: string;
  accentFg: string;
  inputBg: string;
  border: string;
  link: string;
  error: string;
  codeBg: string;
  selection: string;
}

export interface ThemePayload {
  source: "kiro-ide" | "kiro-crew" | "system";
  name: string;
  mode: Kind | "system";
  light: Palette | null;
  dark: Palette | null;
}

export interface SessionInfo {
  sessionId: string;
  agentName: string;
  agentVersion: string;
}

export type ToolStatus = "pending" | "in_progress" | "completed" | "failed";

export interface ContentBlock {
  type: string;
  text?: string;
}

export interface ToolCallFields {
  toolCallId: string;
  title?: string;
  kind?: string;
  status?: ToolStatus;
  rawInput?: Record<string, unknown>;
}

export interface PlanEntry {
  content: string;
  priority?: string;
  status: "pending" | "in_progress" | "completed";
}

export interface SessionUpdate extends Partial<ToolCallFields> {
  sessionUpdate: string;
  messageId?: string;
  content?: ContentBlock | unknown[];
  entries?: PlanEntry[];
}

export interface UpdateEnvelope {
  sessionId: string;
  update: SessionUpdate;
}

export interface PermissionOption {
  optionId: string;
  name: string;
  kind: "allow_once" | "allow_always" | "reject_once" | "reject_always";
}

export interface PermissionRequest {
  requestId: string;
  sessionId: string;
  toolCall: ToolCallFields;
  options: PermissionOption[];
}
