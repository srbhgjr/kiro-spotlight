<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { PermissionOption, PermissionRequest } from "../lib/types";

  let {
    request,
    onanswer,
  }: {
    request: PermissionRequest;
    onanswer: (optionId: string | null) => void;
  } = $props();

  const ORDER: PermissionOption["kind"][] = ["allow_once", "allow_always", "reject_once", "reject_always"];
  const options = $derived([...request.options].sort((a, b) => ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind)));
  const detail = $derived(describe(request.toolCall.rawInput));

  function describe(input: Record<string, unknown> | undefined): string {
    if (!input) return "";
    const value = input.command ?? input.path ?? input.url ?? input.query;
    return typeof value === "string" ? value : "";
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      const allow = options.find((o) => o.kind === "allow_once");
      if (allow) onanswer(allow.optionId);
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="card" role="alertdialog" aria-label="Permission request">
  <div class="head">
    <Icon name="shield" size={15} />
    <span>Kiro wants to run <strong>{request.toolCall.title ?? "a tool"}</strong></span>
  </div>
  {#if detail}
    <pre class="detail">{detail}</pre>
  {/if}
  <div class="buttons">
    {#each options as option (option.optionId)}
      <button
        class:primary={option.kind === "allow_once"}
        class:danger={option.kind.startsWith("reject")}
        onclick={() => onanswer(option.optionId)}
      >
        {option.name}
      </button>
    {/each}
  </div>
</div>

<style>
  .card {
    margin: 10px 14px 0;
    padding: 12px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--input-bg);
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 13px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--fg);
  }

  .head :global(.icon) {
    color: var(--accent);
  }

  .detail {
    margin: 0;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--code-bg);
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 12px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 96px;
    overflow-y: auto;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: flex-end;
  }

  button {
    font: inherit;
    font-size: 13px;
    padding: 4px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
  }

  .primary {
    background: var(--accent);
    color: var(--accent-fg);
    border-color: transparent;
  }

  .danger {
    color: var(--error);
  }
</style>
