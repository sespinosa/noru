# noru

> Local-first desktop perception layer for AI agents — gives them ears and eyes via MCP.

**noru** (乗る — Japanese for *to ride*, *to board*) mounts onto your system's audio and screen, riding along with your workflow and feeding context to AI agents through the [Model Context Protocol](https://modelcontextprotocol.io/).

It's the sensory counterpart to [obsidian-surface](https://github.com/sespinosa/obsidian-surface): where obsidian-surface gives an agent a *display*, noru gives it *perception*.

## Status

🚧 Early development. Not yet usable. See [PLAN.md](./PLAN.md) for the architectural plan and roadmap.

## What it does

- **Detects meetings automatically.** Process and window heuristics + image classifier confirmation. Knows when you're in Zoom, Meet, Teams, etc.
- **Transcribes locally.** Uses Whisper (via [`whisper-rs`](https://github.com/tazz4843/whisper-rs)). System audio + microphone. Optional CUDA. **Nothing ever leaves your machine.**
- **Exposes everything via MCP.** Real-time transcript stream, meeting state, screenshots. Any MCP-compatible AI agent can consume the data.

## Principles

- **Local-first, private by default.** All processing happens on your machine. Zero data leaves the system.
- **Perception layer, not a product.** noru is not a closed meeting-notes app. The agent decides what to do with the context.
- **Lightweight when idle.** Detection runs continuously on CPU; transcription only when needed.
- **Open source, MIT.**

## Platforms

| Platform | Status |
|----------|--------|
| Windows 10/11 | 🚧 In progress (v1 target) |
| WSL2 (with Windows binary) | 🚧 In progress (v1 target) |
| macOS | Planned (v2) |
| Native Linux | Planned (v3) |

## Install (when ready)

```bash
npx -y noru
```

Then add to your MCP client:

```json
{
  "mcpServers": {
    "noru": {
      "command": "npx",
      "args": ["-y", "noru"]
    }
  }
}
```

Same snippet works in Claude Code, Claude Desktop, Cursor, Codex CLI, OpenCode, Continue, Cline, Zed, Windsurf, and any other MCP-compatible client.

## Development

See [PLAN.md](./PLAN.md) for the full architectural plan.

```bash
# Build (Linux/WSL — for Whisper development without audio capture)
LIBCLANG_PATH=/usr/lib/llvm-18/lib cargo build

# Transcribe a WAV file
cargo run -- --model base --file recording.wav
```

Windows builds happen via GitHub Actions — see `.github/workflows/build.yml`.

## License

MIT © Sebastián Espinosa
