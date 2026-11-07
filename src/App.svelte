<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { LogicalSize } from "@tauri-apps/api/dpi";
  import Composer from "./components/Composer.svelte";
  import PermissionCard from "./components/PermissionCard.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import Transcript from "./components/Transcript.svelte";
  import * as ipc from "./lib/ipc";
  import { applyTheme } from "./lib/theme";
  import {
    applyUpdate,
    lastAgentText,
    lastUserText,
    noteItem,
    runningToolTitle,
    userItem,
    type Item,
  } from "./lib/transcript";
  import type { PermissionRequest, UpdateEnvelope } from "./lib/types";

  const WIDTH = 680;
  const MAX_HEIGHT = 640;

  let items = $state<Item[]>([]);
  let busy = $state(false);
  let sessionId = $state<string | null>(null);
  let permissions = $state<PermissionRequest[]>([]);
  let connecting = $state(false);
  let composer: Composer;
  let panel: HTMLDivElement;

  const status = $derived(statusText());

  function statusText(): string {
    if (connecting) return "Connecting to Kiro";
    if (permissions.length) return "Waiting for your approval";
    if (!busy) return "";
    const tool = runningToolTitle(items);
    return tool ? `Running ${tool}` : "Thinking";
  }

  async function ensureSession(): Promise<string> {
    if (sessionId) return sessionId;
    connecting = true;
    try {
      const info = await ipc.startSession();
      sessionId = info.sessionId;
      return info.sessionId;
    } finally {
      connecting = false;
    }
  }

  async function send(text: string) {
    items = [...items, userItem(text)];
    busy = true;
    try {
      await ensureSession();
      const stop = await ipc.sendPrompt(text);
      if (stop === "cancelled") items = [...items, noteItem("Stopped")];
      if (stop === "refusal") items = [...items, noteItem("Kiro declined to answer", "error")];
      if (stop === "max_tokens") items = [...items, noteItem("Answer cut off at the length limit")];
    } catch (error) {
      items = [...items, noteItem(String(error), "error")];
    } finally {
      busy = false;
    }
  }

  function reset() {
    items = [];
    permissions = [];
    sessionId = null;
    busy = false;
    composer?.clear();
  }

  async function startNewChat() {
    reset();
    await ipc.newChat();
    composer?.focus();
  }

  async function answer(optionId: string | null) {
    const [current, ...rest] = permissions;
    if (!current) return;
    permissions = rest;
    await ipc.answerPermission(current.requestId, optionId);
  }

  function onUpdate(envelope: UpdateEnvelope) {
    if (envelope.sessionId !== sessionId) return;
    items = applyUpdate(items, envelope.update);
  }

  function onPermission(request: PermissionRequest) {
    if (request.sessionId !== sessionId) return;
    permissions = [...permissions, request];
  }

  function onSessionEnded(ended: string) {
    if (ended === sessionId) reset();
  }

  function onAgentExited() {
    if (!sessionId) return;
    sessionId = null;
    permissions = [];
    busy = false;
    items = [...items, noteItem("Kiro stopped. Send a message to start a new chat.", "error")];
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      if (permissions.length) {
        void answer(null);
        return;
      }
      reset();
      void ipc.dismiss();
      return;
    }
    if (!event.metaKey) return;
    if (event.key === "k") {
      event.preventDefault();
      void startNewChat();
    } else if (event.key === ".") {
      event.preventDefault();
      void ipc.cancelTurn();
    } else if (event.key === "c" && event.shiftKey) {
      event.preventDefault();
      void navigator.clipboard.writeText(lastAgentText(items));
    }
  }

  function fitWindow() {
    const height = Math.min(MAX_HEIGHT, Math.ceil(panel.getBoundingClientRect().height));
    void getCurrentWindow().setSize(new LogicalSize(WIDTH, height));
  }

  onMount(() => {
    const observer = new ResizeObserver(() => requestAnimationFrame(fitWindow));
    observer.observe(panel);
    void ipc.getTheme().then(applyTheme);
    const unsubscribe = ipc.subscribe({
      update: onUpdate,
      permission: onPermission,
      sessionEnded: onSessionEnded,
      agentExited: onAgentExited,
      shown: () => composer?.focus(),
      theme: applyTheme,
    });
    composer?.focus();
    return () => {
      observer.disconnect();
      void unsubscribe.then((off) => off());
    };
  });
</script>

<svelte:window {onkeydown} />

<div class="panel" bind:this={panel}>
  {#if items.length}
    <Transcript {items} />
  {/if}
  {#if permissions.length}
    <PermissionCard request={permissions[0]} onanswer={answer} />
  {/if}
  <Composer
    bind:this={composer}
    {busy}
    recall={() => lastUserText(items)}
    onsend={send}
    onstop={() => void ipc.cancelTurn()}
  />
  {#if items.length || status}
    <StatusBar {status} />
  {/if}
</div>

<style>
  .panel {
    width: 100%;
    max-height: 640px;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    color: var(--fg);
    overflow: hidden;
    animation: appear 120ms ease-out;
  }

  @keyframes appear {
    from {
      opacity: 0;
      transform: scale(0.985);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .panel {
      animation: none;
    }
  }
</style>
