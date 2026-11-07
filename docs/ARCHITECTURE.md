# Kiro Spotlight: Architecture Spec

Version 0.2, 2026-11-07. Status: draft for review.

## 1. Goal

A macOS menu-bar app that opens a Spotlight-style floating panel on a global hotkey, lets the user chat with Kiro (same login, models, tools and MCP servers as `kiro-cli`), and leaves no trace of the conversation when the panel is dismissed.

Non-goals for v1: voice wake word, multi-window, chat history, file attachments beyond paste, Windows/Linux.

## 2. Constraints and decisions

| Topic | Decision | Why |
|---|---|---|
| Shell | Tauri 2 (Rust core, web UI) | Small binary, native WebKit, Rust owns the agent process |
| Agent backend | `kiro-cli acp` child process, JSON-RPC 2.0 over stdio (ACP) | Officially supported integration path; inherits `kiro-cli login`, agents, MCP servers |
| Engine | `--agent-engine v2` (default). Allow v3 with `--auth-method cli` as a setting | v2 is the default and needs no extra auth plumbing |
| Window | Real `NSPanel` via `tauri-nspanel` plugin, non-activating, floating level, joins all Spaces incl. fullscreen | Standard Spotlight behaviour; a plain NSWindow steals focus and does not overlay fullscreen apps |
| Hotkey | `tauri-plugin-global-shortcut`, default Option+Space, user-configurable | Matches ChatGPT desktop default |
| UI look | macOS structure (NSPanel, vibrancy, SF Pro, Spotlight geometry) with colours taken from the user's selected Kiro theme, live-reloaded (section 6.1) | Feels native on macOS and consistent with Kiro |
| Incognito | Ephemeral session: temp cwd, in-memory transcript, delete kiro-cli session records on close | kiro-cli persists sessions to disk; the app must undo that (section 7) |
| Tool safety | Dedicated agent profile `kiro-spotlight` with read-only tools trusted; everything else prompts | A hotkey panel should not silently write files or run shell |
| Front-end | TypeScript + Svelte (or React), Vite, no CSS framework; `marked` + `shiki` for Markdown/code | Keep the bundle small and styling hand-tuned to macOS |

## 3. System overview

```mermaid
flowchart LR
  subgraph Mac
    HK[Global hotkey<br/>Option+Space]
    MB[Menu-bar item<br/>NSStatusItem]
    subgraph App[Kiro Spotlight.app Tauri 2]
      P[NSPanel webview<br/>Svelte UI]
      C[Rust core<br/>panel ctl, settings,<br/>ACP client, cleanup]
    end
    K[kiro-cli acp<br/>child process]
    FS[~/.kiro/sessions<br/>~/.kiro/session-index<br/>App Support/kiro-cli/data.sqlite3]
  end
  Cloud[Kiro service<br/>models, auth]
  MCP[MCP servers from<br/>~/.kiro/settings/mcp.json]

  HK --> C
  MB --> C
  C <-->|Tauri IPC<br/>commands + events| P
  C <-->|JSON-RPC 2.0<br/>NDJSON over stdio| K
  K --> Cloud
  K --> MCP
  K -.writes.-> FS
  C -.deletes on close.-> FS
```

## 4. Components

### 4.1 Rust core (`src-tauri/`)

```
src-tauri/
  src/
    main.rs            app setup, plugins, tray, shortcut registration
    panel.rs           NSPanel creation, show/hide, positioning, focus-loss handling
    acp/
      mod.rs
      process.rs       spawn/kill kiro-cli acp, stdout reader task, stdin writer
      protocol.rs      serde types for ACP messages (subset we use)
      client.rs        request/response correlation, notification dispatch, permission routing
    session.rs         ephemeral session lifecycle (temp cwd, ids, cleanup)
    settings.rs        hotkey, agent, model, effort, engine, cleanup policy, theme source
    theme/
      mod.rs           resolve active theme, emit theme-changed events
      kiro_ide.rs      read Kiro IDE settings.json + theme extension JSON
      kiro_crew.rs     read Kiro Crew config.json theme_color/theme_mode
      palette.rs       map theme tokens to the app's CSS variable set
      watch.rs         file watcher (notify crate) + system appearance observer
    cleanup.rs         deletes kiro-cli session artefacts for a session id
  tauri.conf.json
  capabilities/default.json
  Entitlements.plist
```

Responsibilities:

- **panel.rs**: converts the main `WebviewWindow` to an `NSPanel` with `nonactivatingPanel` style, level `NSFloatingWindowLevel`, collection behaviour `canJoinAllSpaces | fullScreenAuxiliary | moveToActiveSpace`. Centres horizontally at ~22% from the top of the screen with the mouse cursor. Hides on Esc, on hotkey toggle, and on `resignKey` (click outside) unless a permission prompt is pending.
- **acp/process.rs**: one child `kiro-cli acp --agent kiro-spotlight [--model M] [--effort E] [--trust-tools ...]` per app run (not per panel open) to keep latency low. Restarts it on crash with exponential backoff. stdout is read line by line; stderr goes to the app log.
- **acp/client.rs**: `initialize` on start; `session/new` per panel open; `session/prompt` per message; `session/cancel` on stop. Dispatches `session/update` notifications to the UI as Tauri events. Answers agent-to-client requests: `session/request_permission` (routed to UI), `fs/read_text_file` and `fs/write_text_file` (allowed only under the temp cwd), `terminal/*` (declined in v1; capability not advertised).
- **session.rs**: see section 7.
- **settings.rs**: JSON at `~/Library/Application Support/kiro-spotlight/settings.json`. Nothing conversational is ever stored here.

### 4.2 Web UI (`src/`)

```
src/
  App.svelte            panel root, keyboard handling, state machine
  lib/
    acp.ts              typed wrappers over Tauri commands and events
    markdown.ts         marked + shiki, sanitised with DOMPurify
    theme.css           macOS tokens: vibrancy, -apple-system, accent colour, dark mode
  components/
    Composer.svelte     single-line input that grows to 6 lines, Enter sends, Shift+Enter newline
    Transcript.svelte   streamed assistant text, tool-call rows, plan rows
    ToolCallRow.svelte  collapsed by default, status icon, expand for args/output
    PermissionCard.svelte  Allow once / Allow always (this session) / Deny
    StatusBar.svelte    model, effort, token spinner, "Incognito" badge, Stop button
    Settings.svelte     hotkey recorder, agent/model/effort pickers, engine toggle
```

Panel states: `idle` (composer only, 560x56 px), `streaming` (transcript grows to max 560x640 px, then scrolls), `permission` (card pinned above composer), `error`.

### 4.3 Agent profile (`~/.kiro/agents/kiro-spotlight.json`)

Installed by the app on first run if missing (never overwritten if present):

```json
{
  "name": "kiro-spotlight",
  "description": "Hotkey quick chat. Read-only by default.",
  "model": "auto",
  "tools": ["fs_read", "grep", "glob", "web_fetch", "web_search", "execute_bash", "fs_write", "@*"],
  "allowedTools": ["fs_read", "grep", "glob", "web_fetch", "web_search"],
  "includeMcpJson": true,
  "resources": [],
  "prompt": "You are Kiro in a quick-chat panel. Answers are short. This conversation is not saved."
}
```

`includeMcpJson: true` gives the panel the user's MCP servers (Gmail, Drive, etc.) so it has the same reach as the CLI. `execute_bash` and `fs_write` are available but always go through the permission card.

## 5. ACP message flow

```mermaid
sequenceDiagram
  participant U as User
  participant UI as Svelte UI
  participant R as Rust core
  participant K as kiro-cli acp

  Note over R,K: app launch
  R->>K: spawn kiro-cli acp --agent kiro-spotlight
  R->>K: initialize {protocolVersion, clientCapabilities: fs.read, fs.write}
  K-->>R: {agentCapabilities, authMethods}

  Note over U,K: hotkey pressed
  U->>R: Option+Space
  R->>R: mkdtemp cwd, show panel
  R->>K: session/new {cwd, mcpServers: []}
  K-->>R: {sessionId}
  R->>UI: event session-ready

  U->>UI: types prompt, Enter
  UI->>R: cmd send_prompt(text)
  R->>K: session/prompt {sessionId, prompt:[{type:text}]}
  loop streaming
    K-->>R: session/update agent_message_chunk, tool_call, tool_call_update, plan
    R->>UI: event acp-update (same payload)
  end
  K->>R: session/request_permission {toolCall, options}
  R->>UI: event permission-request
  U->>UI: Allow once
  UI->>R: cmd answer_permission(requestId, optionId)
  R-->>K: response {outcome: selected, optionId}
  K-->>R: session/prompt result {stopReason: end_turn}
  R->>UI: event turn-done

  Note over U,K: Esc or click outside
  U->>R: dismiss
  R->>K: session/cancel (if streaming)
  R->>R: hide panel, run cleanup for sessionId
```

Details:

- Transport is newline-delimited JSON-RPC 2.0; one message per line, no embedded newlines ([ACP transport](https://agentclientprotocol.com/protocol/v1/session-setup)).
- `mcpServers: []` in `session/new` because MCP comes from the agent profile (`includeMcpJson`), not from the client.
- A prompt sent while a turn is streaming is queued in the UI and sent after `turn-done`; the Stop button sends `session/cancel`.
- Pasted images are sent as `{type: "image", data, mimeType}` content blocks if `agentCapabilities.promptCapabilities.image` is true; otherwise the paste is rejected with a toast.

## 6. Native look and feel

- **Material**: panel background is transparent; Rust attaches an `NSVisualEffectView` (material `.hudWindow` in dark, `.popover` in light, blending `behindWindow`) so the panel blurs whatever is behind it like Spotlight.
- **Shape**: 12 px corner radius on the panel; no title bar, no traffic lights, no resize handles (`decorations: false`, `resizable: false`, `transparent: true`).
- **Typography**: `font-family: -apple-system, BlinkMacSystemFont`, 15 px body, 13 px secondary, SF Mono for code via `ui-monospace`.
- **Colour**: all colours come from the active Kiro theme (section 6.1) as CSS variables. The vibrancy material follows the theme's light/dark kind, and the theme background is applied at ~85% opacity over it so the blur still shows through. macOS system colours are used only when no Kiro theme can be resolved.
- **Motion**: panel fades/scales in over 120 ms with `ease-out`, matching Spotlight; respects `prefers-reduced-motion`.
- **Keyboard**: Esc hides; Cmd+, opens settings; Cmd+K clears the conversation (new session); Cmd+Shift+C copies the last answer; Up arrow recalls the last prompt within the current session only.
- **Menu bar**: `NSStatusItem` with template icon, menu: Open (hotkey shown), New conversation, Settings, Quit. The app has no Dock icon (`LSUIElement = true`).
- **Icons**: SF Symbols rendered through Rust (`NSImage(systemSymbolName:)` to PNG at launch) or a bundled subset of equivalent SVGs. No emoji anywhere in the UI.

### 6.1 Following the Kiro theme

The panel uses whatever theme the user picked in Kiro and switches live when they change it.

**Sources, in resolution order** (configurable in settings as `theme_source`: `auto` | `kiro-ide` | `kiro-crew` | `system`; `auto` walks this list):

| # | Source | Where | What is read |
|---|---|---|---|
| 1 | Kiro IDE | `~/Library/Application Support/Kiro/User/settings.json` (JSONC) | `workbench.colorTheme`; if `window.autoDetectColorScheme` is true, `workbench.preferredLightColorTheme` / `workbench.preferredDarkColorTheme` picked by current macOS appearance |
| 2 | Kiro Crew | `~/.kiro/crew/config.json` | `theme_color` (e.g. `rosepine`) and `theme_mode` (`light` / `dark` / `system`) |
| 3 | macOS | `NSApp.effectiveAppearance`, `NSColor.controlAccentColor` | system label, separator and accent colours |

**Kiro IDE theme lookup** (`kiro_ide.rs`):

1. Take the theme label, e.g. `Rosé Pine Dawn`.
2. Scan `package.json` of each extension in `~/.kiro/extensions/*/` and of the built-in themes in `/Applications/Kiro.app/Contents/Resources/app/extensions/theme-*/` for a `contributes.themes[]` entry whose `label` (or `id`) matches.
3. Load the referenced theme JSON (JSONC, may use `include` to chain a parent theme; follow it). `uiTheme` gives the kind: `vs` = light, `vs-dark` = dark, `hc-*` = high contrast.
4. Read the `colors` map. Missing tokens fall back to the VS Code defaults for that kind.

**Kiro Crew lookup** (`kiro_crew.rs`): a built-in table maps each Kiro Crew `theme_color` id to a light and a dark palette (Rosé Pine to Dawn/Main, and so on). `theme_mode: system` follows macOS appearance. Unknown ids fall through to macOS.

**Token mapping** (`palette.rs`), VS Code token to CSS variable:

| CSS variable | Primary token | Fallbacks |
|---|---|---|
| `--bg` | `editor.background` | `sideBar.background` |
| `--fg` | `editor.foreground` | `foreground` |
| `--fg-muted` | `descriptionForeground` | `--fg` at 65% |
| `--accent` | `button.background` | `focusBorder` (opaque), `textLink.foreground` |
| `--accent-fg` | `button.foreground` | `--bg` |
| `--input-bg` | `input.background` | `--bg` |
| `--border` | `widget.border` | `panel.border`, `--fg` at 12% |
| `--link` | `textLink.foreground` | `--accent` |
| `--error` | `errorForeground` | `editorError.foreground` |
| `--code-bg` | `textCodeBlock.background` | `--input-bg` |
| `--selection` | `editor.selectionBackground` | `--accent` at 25% |

Colours with alpha (e.g. `#6e6a8614`, `#0000`) are composited over `--bg` before use so no token renders invisible. Contrast between `--fg` and `--bg` is checked (WCAG AA 4.5:1); on failure `--fg` falls back to the theme kind's default.

**Syntax highlighting**: the same theme JSON's `tokenColors` is passed to `shiki` as a custom theme, so code blocks match the editor.

**Live updates** (`watch.rs`): `notify` watches the two settings files and the resolved theme file; macOS appearance changes come from observing `AppleInterfaceThemeChangedNotification`. On change, Rust re-resolves (debounced 200 ms) and emits a `theme-changed` event with the full palette; the UI swaps CSS variables on `:root` with a 150 ms transition. The theme is also resolved once at launch and sent before the panel's first paint to avoid a flash.

**Observed on this machine (2026-11-07)**: Kiro IDE `workbench.colorTheme` = `Rosé Pine Dawn` from extension `mvllow.rose-pine-2.15.2-universal` (`uiTheme: vs`, `editor.background #faf4ed`, `editor.foreground #575279`, `button.background #d7827e`); Kiro Crew `theme_color` = `rosepine`, `theme_mode` = `system`. Both sources agree on Rosé Pine; `auto` would pick the IDE's Dawn variant.

## 7. Incognito guarantees

What kiro-cli writes on disk per session (observed on kiro-cli 3.1.0):

- `~/.kiro/sessions/<sessionId>/` (transcript and state)
- `~/.kiro/session-index/<sessionId>.jsonl`
- `~/Library/Application Support/kiro-cli/data.sqlite3` (conversation and history tables; exact schema to be confirmed in M1)
- `~/Library/Application Support/kiro-cli/history` (shell history for the TUI; not touched by ACP, verify)

Policy:

1. **Isolated cwd**: each panel session runs in `mkdtemp()` under `$TMPDIR/kiro-spotlight/`. The agent's `--resume` lookups key on cwd, so nothing from the user's repos is resumed, and nothing the agent writes by default lands in a real project.
2. **In-memory transcript only**: the UI keeps the transcript in component state; no localStorage, no IndexedDB, no Tauri store. The webview is created with `incognito: true` so WebKit uses a non-persistent data store.
3. **Cleanup on dismiss** (`cleanup.rs`), run after `session/cancel` completes or the turn ends:
   - delete `~/.kiro/sessions/<sessionId>/` and `~/.kiro/session-index/<sessionId>.jsonl`;
   - delete rows for `<sessionId>` in `data.sqlite3` (tables identified in M1; use `rusqlite` with `busy_timeout`, since kiro-cli may hold the DB);
   - remove the temp cwd;
   - zero the in-memory transcript.
   If `kiro-cli` exposes `session/delete` (ACP v2 draft), call it first and fall back to file deletion.
4. **Cleanup on crash**: at launch, delete any leftover `$TMPDIR/kiro-spotlight/*` and any session ids recorded in `~/Library/Application Support/kiro-spotlight/pending-cleanup.json` (written before `session/new`, cleared after cleanup).
5. **Clipboard**: copying an answer is a user action and is allowed; the app never writes the clipboard on its own.
6. **Logs**: app log at `~/Library/Logs/kiro-spotlight/` records protocol method names and timings only, never message bodies. Log level `debug` (bodies included) is opt-in and shows a persistent banner in the panel.
7. **What is not covered**: kiro-cli telemetry and the Kiro service side. Incognito is local; it does not change what the service retains under the account's data policy.

## 8. Permissions and security

- Agent runs with `allowedTools` limited to read-only tools; `execute_bash`, `fs_write` and MCP tools prompt via `session/request_permission` every time unless the user picks "Allow for this session" (held in memory, cleared at cleanup).
- Client-side `fs/read_text_file` and `fs/write_text_file` requests are honoured only for paths under the session temp cwd; anything else returns a JSON-RPC error so the agent falls back to its own tools (which prompt).
- `terminal/*` client capability is not advertised in v1.
- The agent's streamed text is Markdown rendered through DOMPurify; links open in the default browser via `tauri-plugin-opener`, never inside the webview.
- Tauri capability file grants only: `core:window`, `global-shortcut`, `opener`, `clipboard-manager:write`, and the app's own commands. No `shell` plugin in the frontend; only Rust spawns `kiro-cli`.
- Hardened runtime, app sandbox off (the app must read `~/.kiro` and spawn a user binary), notarised for distribution.
- `kiro-cli` is located via `settings.cli_path`, then `which kiro-cli`, then `/usr/local/bin` and `~/.local/bin`. Not bundled; the user's installed CLI and login are used.

## 9. Error handling

| Condition | Behaviour |
|---|---|
| `kiro-cli` not found | Panel shows install hint with a link to the Kiro CLI docs |
| Not signed in (`initialize` returns `authMethods` and `session/new` fails with auth error) | Panel shows "Sign in" button that opens Terminal with `kiro-cli login`; the app re-runs `initialize` when the panel is next opened |
| Child process exits | Toast, auto-restart up to 3 times in 30 s, then "Restart Kiro" button |
| `session/prompt` error | Inline error row, composer re-enabled, session kept |
| Cleanup failure (DB locked) | Retry 5 times over 10 s, then record id in `pending-cleanup.json` and retry at next launch |

## 10. Settings (all non-conversational)

hotkey, agent name (default `kiro-spotlight`), model, effort, engine (v2/v3), theme source (`auto` / `kiro-ide` / `kiro-crew` / `system`), "hide on focus loss", "launch at login" (`tauri-plugin-autostart`), log level, `cli_path`.

## 11. Build, test, release

- Toolchain: Rust stable, Node 22, `pnpm`, Tauri CLI 2. Xcode CLT for codesign.
- Unit tests: `protocol.rs` serde round-trips against recorded ACP fixtures; `cleanup.rs` against a fake `~/.kiro` tree; `client.rs` request correlation with a fake agent (a small Rust binary that replays NDJSON).
- Integration test: spawn real `kiro-cli acp`, run `initialize` + `session/new` + `session/cancel`, assert the session directory is gone after cleanup. Needs a signed-in machine; marked `#[ignore]` in CI.
- UI: Playwright against the Vite dev server with a mocked Tauri IPC layer.
- Release: `tauri build` universal binary, codesign with Developer ID, notarise, DMG. Auto-update via `tauri-plugin-updater` is v2.

## 12. Milestones

| M | Scope | Exit criteria |
|---|---|---|
| M0 | Spike | Hotkey toggles an NSPanel with vibrancy over a fullscreen app; `kiro-cli acp` answers one `session/prompt` from Rust and streams to the webview |
| M1 | Incognito | Confirm which files and `data.sqlite3` tables kiro-cli writes under ACP; cleanup deletes them; test proves nothing remains |
| M2 | Chat UX | Markdown streaming, tool-call rows, permission card, Stop, Cmd+K, error states |
| M3 | Native polish + Kiro theme | Kiro theme resolution and live reload, shiki theme from `tokenColors`, SF Symbols, animations, menu bar, settings, hotkey recorder |
| M4 | Ship | Sign, notarise, DMG, launch-at-login, crash cleanup |

## 13. Open questions

1. Does `kiro-cli acp` honour a per-session `--agent` other than at process start? If not, changing the agent in settings restarts the child (acceptable).
2. Does kiro-cli 3.1.0 implement ACP `session/delete`? If yes, M1 cleanup becomes a protocol call plus a verification sweep.
3. Should "Allow for this session" also cover MCP tools from `mcp.json`, or only built-ins? Default proposal: built-ins only.
4. Whether to support v3 engine `--auth-method client` in v1 (the app would have to answer access-token requests). Proposal: no.

## References

- Agent Client Protocol spec: [agentclientprotocol.com](https://agentclientprotocol.com/protocol/v1/session-setup)
- tauri-nspanel plugin: [github.com/ahkohd/tauri-nspanel](https://github.com/ahkohd/tauri-nspanel)
- Spotlight example for Tauri: [github.com/ahkohd/tauri-macos-spotlight-example](https://github.com/ahkohd/tauri-macos-spotlight-example)
- Tauri global shortcut plugin: [tauri.app/plugin/global-shortcut](https://tauri.app/plugin/global-shortcut/)
