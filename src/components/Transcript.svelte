<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ToolCallRow from "./ToolCallRow.svelte";
  import { linkTarget, renderMarkdown } from "../lib/markdown";
  import type { Item } from "../lib/transcript";

  let { items }: { items: Item[] } = $props();

  let scroller: HTMLDivElement;

  $effect(() => {
    void items.length;
    const last = items[items.length - 1];
    void (last && "text" in last ? last.text.length : 0);
    queueMicrotask(() => scroller?.scrollTo({ top: scroller.scrollHeight }));
  });

  function onclick(event: MouseEvent) {
    const url = linkTarget(event);
    if (!url) return;
    event.preventDefault();
    void openUrl(url);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="transcript" bind:this={scroller} {onclick}>
  {#each items as item (item.id)}
    {#if item.kind === "user"}
      <div class="user">{item.text}</div>
    {:else if item.kind === "agent"}
      <div class="agent markdown">{@html renderMarkdown(item.text)}</div>
    {:else if item.kind === "thought"}
      <details class="thought">
        <summary>Thinking</summary>
        <div class="markdown">{@html renderMarkdown(item.text)}</div>
      </details>
    {:else if item.kind === "tool"}
      <ToolCallRow title={item.title} status={item.status} />
    {:else if item.kind === "plan"}
      <ol class="plan">
        {#each item.entries as entry, i (i)}
          <li class:done={entry.status === "completed"}>{entry.content}</li>
        {/each}
      </ol>
    {:else}
      <div class="note" class:error={item.tone === "error"}>{item.text}</div>
    {/if}
  {/each}
</div>

<style>
  .transcript {
    max-height: 480px;
    overflow-y: auto;
    padding: 16px 18px 8px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    user-select: text;
    -webkit-user-select: text;
    border-bottom: 1px solid var(--border);
  }

  .user {
    align-self: flex-end;
    max-width: 85%;
    background: var(--input-bg);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 7px 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .agent {
    line-height: 1.5;
  }

  .thought {
    color: var(--fg-muted);
    font-size: 13px;
  }

  .thought summary {
    cursor: default;
    list-style: none;
  }

  .plan {
    margin: 0;
    padding-left: 20px;
    color: var(--fg-muted);
    font-size: 13px;
  }

  .plan .done {
    text-decoration: line-through;
  }

  .note {
    color: var(--fg-muted);
    font-size: 13px;
  }

  .note.error {
    color: var(--error);
  }

  .markdown :global(p) {
    margin: 0 0 8px;
  }

  .markdown :global(p:last-child) {
    margin-bottom: 0;
  }

  .markdown :global(a) {
    color: var(--link);
    text-decoration: none;
  }

  .markdown :global(a:hover) {
    text-decoration: underline;
  }

  .markdown :global(code) {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 13px;
    background: var(--code-bg);
    border-radius: 5px;
    padding: 1px 5px;
  }

  .markdown :global(pre) {
    background: var(--code-bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
    overflow-x: auto;
    margin: 6px 0;
  }

  .markdown :global(pre code) {
    background: none;
    padding: 0;
  }

  .markdown :global(ul),
  .markdown :global(ol) {
    margin: 4px 0 8px;
    padding-left: 22px;
  }

  .markdown :global(table) {
    border-collapse: collapse;
    font-size: 13px;
  }

  .markdown :global(th),
  .markdown :global(td) {
    border: 1px solid var(--border);
    padding: 4px 8px;
  }

  .markdown :global(blockquote) {
    margin: 6px 0;
    padding-left: 10px;
    border-left: 3px solid var(--border);
    color: var(--fg-muted);
  }
</style>
