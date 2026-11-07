<script lang="ts">
  const PATHS = {
    spark: "M12 3l1.9 6.1L20 11l-6.1 1.9L12 19l-1.9-6.1L4 11l6.1-1.9z",
    send: "M12 19V5M5 12l7-7 7 7",
    stop: "M7 7h10v10H7z",
    eyeOff:
      "M3 3l18 18M10.6 10.6a2 2 0 0 0 2.8 2.8M9.9 5.1A9.8 9.8 0 0 1 12 5c5 0 9 4.5 10 7a12.6 12.6 0 0 1-3.2 4.2M6.6 6.6C4.4 8 2.8 10.1 2 12c1 2.5 5 7 10 7 1.7 0 3.3-.5 4.6-1.3",
    check: "M5 12.5l4.5 4.5L19 7.5",
    x: "M6 6l12 12M18 6L6 18",
    tool: "M14.7 6.3a4 4 0 0 0-5.4 5.4L4 17l3 3 5.3-5.3a4 4 0 0 0 5.4-5.4l-2.5 2.5-2.4-.6-.6-2.4z",
    alert: "M12 8v5M12 16.5v.5M10.3 3.9L2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
    shield: "M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6z",
    circle: "M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16z",
  } as const;

  type Name = keyof typeof PATHS;

  let { name, size = 16, label = "" }: { name: Name; size?: number; label?: string } = $props();
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? "img" : undefined}
  aria-label={label || undefined}
  aria-hidden={label ? undefined : "true"}
>
  <path d={PATHS[name]} fill={name === "spark" || name === "stop" ? "currentColor" : "none"} />
</svg>

<style>
  .icon {
    flex: none;
    display: block;
  }
</style>
