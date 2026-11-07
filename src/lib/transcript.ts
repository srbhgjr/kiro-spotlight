import type { ContentBlock, PlanEntry, SessionUpdate, ToolStatus } from "./types";

export type Item =
  | { kind: "user"; id: number; text: string }
  | { kind: "agent"; id: number; messageId?: string; text: string }
  | { kind: "thought"; id: number; messageId?: string; text: string }
  | { kind: "tool"; id: number; toolCallId: string; title: string; status: ToolStatus; toolKind: string }
  | { kind: "plan"; id: number; entries: PlanEntry[] }
  | { kind: "note"; id: number; text: string; tone: "muted" | "error" };

let nextId = 1;
const id = () => nextId++;

export const userItem = (text: string): Item => ({ kind: "user", id: id(), text });
export const noteItem = (text: string, tone: "muted" | "error" = "muted"): Item => ({
  kind: "note",
  id: id(),
  text,
  tone,
});

type Handler = (items: Item[], update: SessionUpdate) => Item[];

function textOf(content: SessionUpdate["content"]): string {
  if (!content || Array.isArray(content)) return "";
  const block = content as ContentBlock;
  return block.type === "text" ? (block.text ?? "") : "";
}

function appendChunk(kind: "agent" | "thought"): Handler {
  return (items, update) => {
    const text = textOf(update.content);
    if (!text) return items;
    const last = items[items.length - 1];
    const sameMessage = !update.messageId || !last || !("messageId" in last) || last.messageId === update.messageId;
    if (last && last.kind === kind && sameMessage) {
      return [...items.slice(0, -1), { ...last, text: last.text + text }];
    }
    return [...items, { kind, id: id(), messageId: update.messageId, text }];
  };
}

const addTool: Handler = (items, update) => {
  if (!update.toolCallId) return items;
  return [
    ...items,
    {
      kind: "tool",
      id: id(),
      toolCallId: update.toolCallId,
      title: update.title ?? "Tool call",
      status: update.status ?? "pending",
      toolKind: update.kind ?? "other",
    },
  ];
};

const updateTool: Handler = (items, update) =>
  items.map((item) =>
    item.kind === "tool" && item.toolCallId === update.toolCallId
      ? {
          ...item,
          title: update.title ?? item.title,
          status: update.status ?? item.status,
          toolKind: update.kind ?? item.toolKind,
        }
      : item,
  );

const setPlan: Handler = (items, update) => {
  const entries = update.entries ?? [];
  const index = items.findIndex((item) => item.kind === "plan");
  if (index === -1) return [...items, { kind: "plan", id: id(), entries }];
  return items.map((item, i) => (i === index ? { ...item, entries } : item)) as Item[];
};

const handlers: Record<string, Handler> = {
  agent_message_chunk: appendChunk("agent"),
  agent_thought_chunk: appendChunk("thought"),
  tool_call: addTool,
  tool_call_update: updateTool,
  plan: setPlan,
};

/** Fold one ACP `session/update` into the transcript. Unknown update kinds are ignored. */
export function applyUpdate(items: Item[], update: SessionUpdate): Item[] {
  const handler = handlers[update.sessionUpdate];
  return handler ? handler(items, update) : items;
}

export function lastAgentText(items: Item[]): string {
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i];
    if (item.kind === "agent") return item.text;
  }
  return "";
}

export function lastUserText(items: Item[]): string {
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i];
    if (item.kind === "user") return item.text;
  }
  return "";
}

export function runningToolTitle(items: Item[]): string {
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i];
    if (item.kind === "tool" && (item.status === "pending" || item.status === "in_progress")) return item.title;
  }
  return "";
}
