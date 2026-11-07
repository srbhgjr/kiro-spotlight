<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    busy,
    recall,
    onsend,
    onstop,
  }: {
    busy: boolean;
    recall: () => string;
    onsend: (text: string) => void;
    onstop: () => void;
  } = $props();

  let text = $state("");
  let input: HTMLTextAreaElement;

  export function focus() {
    input?.focus();
  }

  export function clear() {
    text = "";
    queueMicrotask(grow);
  }

  function grow() {
    if (!input) return;
    input.style.height = "auto";
    input.style.height = `${Math.min(input.scrollHeight, 160)}px`;
  }

  function submit() {
    const value = text.trim();
    if (!value || busy) return;
    onsend(value);
    clear();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      submit();
      return;
    }
    if (event.key === "ArrowUp" && text === "") {
      const previous = recall();
      if (previous) {
        event.preventDefault();
        text = previous;
        queueMicrotask(grow);
      }
    }
  }
</script>

<div class="composer">
  <span class="glyph"><Icon name="spark" size={20} /></span>
  <textarea
    bind:this={input}
    bind:value={text}
    oninput={grow}
    {onkeydown}
    rows="1"
    placeholder="Ask Kiro"
    spellcheck="false"
    autocomplete="off"
  ></textarea>
  {#if busy}
    <button class="action stop" onclick={onstop} title="Stop (Cmd+.)" aria-label="Stop">
      <Icon name="stop" size={14} />
    </button>
  {:else}
    <button class="action" onclick={submit} disabled={!text.trim()} title="Send (Enter)" aria-label="Send">
      <Icon name="send" size={16} />
    </button>
  {/if}
</div>

<style>
  .composer {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 64px;
    padding: 0 14px 0 18px;
    box-sizing: border-box;
  }

  .glyph {
    color: var(--accent);
    display: flex;
  }

  textarea {
    flex: 1;
    resize: none;
    border: none;
    outline: none;
    background: transparent;
    color: var(--fg);
    font: inherit;
    font-size: 19px;
    line-height: 26px;
    padding: 19px 0;
    max-height: 160px;
    user-select: text;
    -webkit-user-select: text;
  }

  textarea::placeholder {
    color: var(--fg-muted);
  }

  .action {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    border: none;
    background: var(--accent);
    color: var(--accent-fg);
    cursor: default;
    transition: opacity 120ms ease-out;
  }

  .action:disabled {
    opacity: 0.35;
  }

  .stop {
    background: var(--input-bg);
    color: var(--fg);
    border: 1px solid var(--border);
  }
</style>
