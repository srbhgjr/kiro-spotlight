import type { Palette, ThemePayload } from "./types";

const VARS: Record<keyof Omit<Palette, "kind">, string> = {
  bg: "--bg",
  fg: "--fg",
  fgMuted: "--fg-muted",
  accent: "--accent",
  accentFg: "--accent-fg",
  inputBg: "--input-bg",
  border: "--border",
  link: "--link",
  error: "--error",
  codeBg: "--code-bg",
  selection: "--selection",
};

const media = window.matchMedia("(prefers-color-scheme: dark)");
let current: ThemePayload | null = null;

function pick(theme: ThemePayload, systemDark: boolean): Palette | null {
  const preferDark = theme.mode === "dark" || (theme.mode === "system" && systemDark);
  return preferDark ? (theme.dark ?? theme.light) : (theme.light ?? theme.dark);
}

function render() {
  const root = document.documentElement;
  const palette = current ? pick(current, media.matches) : null;
  for (const name of Object.values(VARS)) root.style.removeProperty(name);
  const kind = palette?.kind ?? (media.matches ? "dark" : "light");
  root.dataset.kind = kind;
  root.style.colorScheme = kind;
  if (!palette) return;
  for (const [key, name] of Object.entries(VARS)) {
    root.style.setProperty(name, palette[key as keyof typeof VARS]);
  }
}

/** Apply a resolved Kiro theme to the document as CSS variables. */
export function applyTheme(theme: ThemePayload) {
  current = theme;
  render();
}

media.addEventListener("change", render);
