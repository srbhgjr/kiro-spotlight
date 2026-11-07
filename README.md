# Kiro Spotlight

A Spotlight-style quick chat panel for macOS that talks to Kiro through `kiro-cli acp`.
Toggle it with a global hotkey (default Option+Space), ask, dismiss. Nothing is kept.

## Status

M0 to M2 implemented on `feat/m0-spike`: panel, hotkey, menu bar, ACP client, streaming chat,
permission prompts, incognito cleanup and Kiro theme following. Not yet signed or packaged.
See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Requirements

- macOS 13 or later
- [Kiro CLI](https://kiro.dev) installed and signed in (`kiro-cli login`)
- For development: Rust stable, Node 22+, pnpm, Xcode Command Line Tools

## Develop

```sh
pnpm install
pnpm tauri dev
```

The app lives in the menu bar (no Dock icon). Press Option+Space to open the panel.

| Key | Action |
|---|---|
| Enter / Shift+Enter | Send / new line |
| Up (empty input) | Recall last prompt |
| Esc | Close and end the chat (rejects a pending permission first) |
| Cmd+K | New chat |
| Cmd+. | Stop the current answer |
| Cmd+Shift+C | Copy the last answer |

## Test

```sh
pnpm check && pnpm test                        # front-end types and reducer tests
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
cargo test live -- --ignored --nocapture       # against the real signed-in kiro-cli (uses credits)
```

## Settings

Optional JSON at `~/Library/Application Support/dev.srbhgjr.kiro-spotlight/settings.json`:

```json
{ "hotkey": "Alt+Space", "model": null, "themeSource": "auto", "idleMinutes": 10, "cliPath": null }
```

`themeSource` is `auto` (Kiro IDE, then Kiro Crew, then macOS), `kiro-ide`, `kiro-crew` or `system`.

## Layout

```
src/          web UI (Svelte 5 + TypeScript)
src-tauri/    Rust core: panel, hotkey, ACP client, session cleanup, theme
docs/         architecture spec
```

## Principles

- Uses the user's existing `kiro-cli` login and MCP servers. The app holds no credentials.
- Incognito by construction: temp working directory per chat, in-memory transcript,
  `session/delete` plus a sweep of kiro-cli's per-cwd storage when the chat ends.
- Commands and file writes always ask first (`autopilot: off`).
- Looks like a macOS panel and uses the colours of your Kiro theme.

## License

MIT
