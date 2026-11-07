<script lang="ts">
  import Icon from "./Icon.svelte";

  let { status }: { status: string } = $props();
</script>

<footer class="status">
  <span class="incognito" title="Nothing from this chat is kept after it ends">
    <Icon name="eyeOff" size={12} />
    Incognito
  </span>
  <span class="state">{status}</span>
  <span class="keys">
    <kbd>esc</kbd> close
    <kbd>&#8984;K</kbd> new chat
  </span>
</footer>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 14px 8px;
    font-size: 11px;
    color: var(--fg-muted);
    border-top: 1px solid var(--border);
  }

  .incognito {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .state {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .keys {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  kbd {
    font-family: inherit;
    font-size: 10px;
    padding: 0 4px;
    border-radius: 4px;
    border: 1px solid var(--border);
    margin-left: 6px;
  }
</style>
