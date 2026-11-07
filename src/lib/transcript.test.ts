import { describe, expect, it } from "vitest";
import { applyUpdate, lastAgentText, runningToolTitle, userItem, type Item } from "./transcript";

const chunk = (text: string, messageId?: string) => ({
  sessionUpdate: "agent_message_chunk",
  messageId,
  content: { type: "text", text },
});

describe("applyUpdate", () => {
  it("merges consecutive agent chunks into one message", () => {
    let items: Item[] = [userItem("hi")];
    items = applyUpdate(items, chunk("Hel"));
    items = applyUpdate(items, chunk("lo"));
    expect(items).toHaveLength(2);
    expect(lastAgentText(items)).toBe("Hello");
  });

  it("starts a new message when messageId changes", () => {
    let items: Item[] = [];
    items = applyUpdate(items, chunk("a", "m1"));
    items = applyUpdate(items, chunk("b", "m2"));
    expect(items.map((i) => i.kind)).toEqual(["agent", "agent"]);
  });

  it("tracks tool calls through updates", () => {
    let items: Item[] = [];
    items = applyUpdate(items, { sessionUpdate: "tool_call", toolCallId: "t1", title: "Read file", status: "pending" });
    expect(runningToolTitle(items)).toBe("Read file");
    items = applyUpdate(items, { sessionUpdate: "tool_call_update", toolCallId: "t1", status: "completed" });
    expect(runningToolTitle(items)).toBe("");
    expect(items[0]).toMatchObject({ kind: "tool", status: "completed", title: "Read file" });
  });

  it("replaces the plan in place", () => {
    let items: Item[] = [];
    items = applyUpdate(items, { sessionUpdate: "plan", entries: [{ content: "a", status: "pending" }] });
    items = applyUpdate(items, { sessionUpdate: "plan", entries: [{ content: "a", status: "completed" }] });
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({ kind: "plan", entries: [{ status: "completed" }] });
  });

  it("ignores unknown and Kiro-specific updates", () => {
    const items: Item[] = [userItem("x")];
    expect(applyUpdate(items, { sessionUpdate: "session_info_update" })).toBe(items);
    expect(applyUpdate(items, { sessionUpdate: "available_commands_update" })).toBe(items);
  });
});
