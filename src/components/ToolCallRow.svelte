<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { ToolStatus } from "../lib/types";

  let { title, status }: { title: string; status: ToolStatus } = $props();

  const icon = $derived(status === "completed" ? "check" : status === "failed" ? "x" : "tool");
  const running = $derived(status === "pending" || status === "in_progress");
</script>

<div class="tool" class:failed={status === "failed"} class:running>
  <Icon name={icon} size={13} />
  <span class="title">{title}</span>
</div>

<style>
  .tool {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--fg-muted);
    font-size: 13px;
    padding: 2px 0;
  }

  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .running {
    color: var(--fg);
  }

  .running :global(.icon) {
    animation: pulse 1.2s ease-in-out infinite;
  }

  .failed {
    color: var(--error);
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .running :global(.icon) {
      animation: none;
    }
  }
</style>
