import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PermissionRequest, SessionInfo, ThemePayload, UpdateEnvelope } from "./types";

export const startSession = () => invoke<SessionInfo>("start_session");
export const sendPrompt = (text: string) => invoke<string>("send_prompt", { text });
export const cancelTurn = () => invoke<void>("cancel_turn");
export const answerPermission = (requestId: string, optionId: string | null) =>
  invoke<void>("answer_permission", { requestId, optionId });
export const newChat = () => invoke<void>("new_chat");
export const dismiss = () => invoke<void>("dismiss");
export const hidePanel = () => invoke<void>("hide");
export const getTheme = () => invoke<ThemePayload>("get_theme");
export const hotkeyLabel = () => invoke<string>("hotkey_label");

export interface Handlers {
  update: (envelope: UpdateEnvelope) => void;
  permission: (request: PermissionRequest) => void;
  sessionEnded: (sessionId: string) => void;
  agentExited: () => void;
  shown: () => void;
  theme: (payload: ThemePayload) => void;
}

export async function subscribe(h: Handlers): Promise<UnlistenFn> {
  const offs = await Promise.all([
    listen<UpdateEnvelope>("acp://update", (e) => h.update(e.payload)),
    listen<PermissionRequest>("acp://permission", (e) => h.permission(e.payload)),
    listen<string>("session://ended", (e) => h.sessionEnded(e.payload)),
    listen("acp://exited", () => h.agentExited()),
    listen("panel://shown", () => h.shown()),
    listen<ThemePayload>("theme://changed", (e) => h.theme(e.payload)),
  ]);
  return () => offs.forEach((off) => off());
}
