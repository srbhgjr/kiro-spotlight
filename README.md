# Kiro Spotlight

A Spotlight-style quick chat panel for macOS that talks to Kiro through `kiro-cli acp`.
Toggle it with a global hotkey (default Option+Space), ask, dismiss. Nothing is kept.

## Status

Design phase. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Requirements

- macOS 13 or later
- [Kiro CLI](https://kiro.dev) installed and signed in (`kiro-cli login`)
- For development: Rust stable, Node 22, pnpm, Tauri CLI 2, Xcode Command Line Tools

## Layout

```
src/          web UI (Svelte + TypeScript)
src-tauri/    Rust core: panel, hotkey, ACP client, cleanup
docs/         architecture and design notes
```

## Principles

- Uses the user's existing `kiro-cli` login, agents and MCP servers. The app holds no credentials.
- Incognito by construction: temp working directory, in-memory transcript, kiro-cli session records deleted on dismiss.
- Read-only tools by default; anything that writes or runs prompts for permission.
- Looks like a macOS panel, not a web page.

## License

MIT
