# noru

> **Open-source, local-first meeting transcription that AI agents can actually use.**

Records your meetings, transcribes them locally with [Whisper](https://github.com/openai/whisper), and exposes the transcripts (plus meeting state, screenshots, and events) to any [MCP](https://modelcontextprotocol.io/)-compatible AI agent. **Your audio never leaves your machine.**

Think of it as the *perception layer* for AI agents — meeting transcription is the first concrete use case, but the same primitives enable broader desktop awareness over time. It's the sensory counterpart to [obsidian-surface](https://github.com/sespinosa/obsidian-surface): obsidian-surface gives an agent a *display*, noru gives it *senses*.

The name **noru** (乗る — Japanese for *to ride*, *to board*) describes what it does: it mounts onto your audio and screen, riding along with your workflow.

---

## Status

🚧 **Early development.** Not yet usable end-to-end. Whisper transcription core works (verified on Linux/WSL with WAV input). Windows binary, MCP server, tray app, and detection module are next.

See [PLAN.md](./PLAN.md) for the v1 architecture and [ROADMAP.md](./ROADMAP.md) for the phased vision (v1 → v3).

---

## What it does (v1)

- 🎙️ **Captures system audio + microphone** with WASAPI on Windows. Never sends a byte off your machine.
- 📝 **Transcribes locally** with [`whisper-rs`](https://github.com/tazz4843/whisper-rs). Models auto-download on first use. Optional CUDA.
- 👀 **Detects meetings automatically** via process and window heuristics, with an image classifier for confirmation.
- 📬 **Mailbox + MCP** — exposes transcripts, meeting state, and an inbox/outbox for agent-to-agent messaging through standard MCP tools and resources. Any MCP-compatible client (Claude Code, Cursor, Codex CLI, Claude Desktop, OpenCode, …) can read from and write to it.
- ⏰ **Scheduler / time dimension** — the same mailbox supports scheduled and recurring delivery. Agents can say "deliver this message in 10 minutes" or "every weekday at 5pm" and noru handles it. Turns noru from a passive sensor into a proactive scheduler.
- 🪟 **System tray app** for config, transcript history, and visible status — built with [Tauri v2](https://v2.tauri.app/).

## What it could become

The phases beyond v1 add: voice activation with a custom wake word, an attention state machine, R2-D2-style audio feedback, voice → coding agent dispatch with meeting context auto-attached, and eventually full computer control via [trycua/cua](https://github.com/trycua/cua).

The novel insight is the **voice → meeting-aware agent dispatch**: say "summarize what we just talked about and put it in my notes," and noru spawns the right agent with the relevant transcript already in its context. Nobody is shipping this publicly as of April 2026.

Read [ROADMAP.md](./ROADMAP.md) for the full phased vision.

---

## Principles

- **Local-first, private by default.** All processing happens on your machine. Zero data leaves the system.
- **Perception layer, not a closed product.** noru is not another Otter/Granola. It's an open toolkit. The agent decides what to do with the data.
- **Lightweight when idle.** Detection runs cheap and continuously; transcription only when it should.
- **Open source, MIT.**

---

## Platforms

| Platform | Status |
|----------|--------|
| Windows 10/11 | 🚧 In progress (v1 target) |
| WSL2 (with Windows binary) | 🚧 In progress (v1 target) |
| macOS | Planned (v2) |
| Native Linux | Planned (v3) |

Why Windows-first: WSL2 cannot directly access audio or screen, so the binary must be a Windows process. From WSL2 you launch it via `cmd.exe /c noru.exe`, but it runs as a full Windows process with WASAPI / Win32 access.

---

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

The same 4-line snippet works in Claude Code, Claude Desktop, Cursor, Codex CLI, OpenCode, Continue, Cline, Zed, Windsurf, and any other MCP-compatible client.

To promote to a permanent install (system tray app, autostart, no Node.js needed at runtime), ask any agent that has the noru MCP installed:

> "use the noru_setup tool to install permanently with autostart"

The setup tool runs inside `noru.exe` itself with full Windows access, copies the binary to `%LOCALAPPDATA%\Programs\noru\`, adds it to PATH, optionally registers Windows startup, and you're done.

---

## Development

See [PLAN.md](./PLAN.md) for the architectural plan and current status.

```bash
# Build (Linux/WSL — for Whisper development without audio capture)
LIBCLANG_PATH=/usr/lib/llvm-18/lib cargo build

# Transcribe a WAV file
cargo run -- --model base --file recording.wav

# Live capture (Windows only — needs WASAPI)
cargo run -- --model base
```

Windows builds happen via GitHub Actions (`.github/workflows/build.yml`). Local Windows builds are explicitly avoided to keep the dev environment clean — see PLAN.md §7.

---

## License

MIT © Sebastián Espinosa
