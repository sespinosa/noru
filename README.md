# noru

> **A meeting recorder that just works. Local-first transcription. Optional ChatGPT-powered summaries.**

Open the .exe. Tray icon appears. Start a Zoom / Meet / Teams call. noru detects it, records it, transcribes it locally with [Whisper](https://github.com/openai/whisper), and stores it on your machine. **Your audio never leaves your computer.**

If you want noru to help you do something with the transcripts (summarize, extract action items, pull out key decisions), there's an opt-in **AI Features (experimental)** option in Settings. Sign in with your ChatGPT account once and the AI features become available — no API keys, nothing to configure, uses your existing ChatGPT Plus / Pro subscription.

The name **noru** (乗る — Japanese for *to ride*, *to board*) describes what it does: it mounts onto your audio and screen, riding along with your workflow.

---

## Status

🚧 **Early development.** Not yet shippable. The Whisper transcription core works (verified on Linux/WSL); the Tauri tray app, meeting detection, transcript browser, and ChatGPT OAuth flow are next.

See [PLAN.md](./PLAN.md) for the v1 plan and [ROADMAP.md](./ROADMAP.md) for the phased vision (v1 → v4).

---

## What v1 does

- 🎙️ **Records meetings automatically.** Detects Zoom, Meet, Teams, etc. via process and window heuristics. Captures system audio + microphone with WASAPI on Windows. Clear visual indicator in the tray when recording.
- 📝 **Transcribes locally** with [`whisper-rs`](https://github.com/tazz4843/whisper-rs). Models auto-download on first use. CPU-only in v1; CUDA later.
- 📂 **Stores transcripts** in a local sqlite database. Browse, search, and read them in the built-in viewer.
- 🤖 **Three AI features** *(opt-in, experimental)*: summarize, extract action items, extract key decisions. One click each. Powered by your ChatGPT subscription via an experimental sign-in flow — no API keys, no setup.
- ⚙️ **Just works on first launch.** No setup wizard, no questions, no prompts. Open the .exe and you're ready to record.
- 🪟 **Single Windows .exe.** Download from GitHub Releases, double-click to run. Optional auto-start with Windows.

## What's coming after v1

- **v1.5:** MCP server + mailbox + scheduler. Any MCP-compatible AI agent (Claude Code, Cursor, Codex CLI, Claude Desktop, OpenCode, …) can read your transcripts, schedule reminders, and react to noru's events.
- **v2:** Voice activation with a custom wake word, attention state machine, and R2-D2-style audio feedback (no expensive TTS — beeps with personality).
- **v3:** Voice → agent dispatch. Say *"summarize what we just talked about and put it in my notes"* and noru spawns the right agent with the relevant transcript already in its context.
- **v4:** Full computer control via [trycua/cua](https://github.com/trycua/cua) and Anthropic Computer Use.

The killer combination — local-first + open-source + multi-modal capture + voice → multi-agent dispatch — doesn't exist as a public product as of April 2026. v1 is the foundation.

Read [ROADMAP.md](./ROADMAP.md) for the full phased vision.

---

## Principles

- **Local-first, private by default.** Audio capture and Whisper transcription happen entirely on your machine. The only network calls noru ever makes are: (a) Whisper model download on first use, (b) ChatGPT calls *only if you've explicitly enabled AI features in Settings*.
- **Just works on first launch.** No setup wizard. No prompts. No questions.
- **AI is opt-in and experimental.** Lives in Settings → "AI Features (experimental)". Easy to find for those who want it, invisible to those who don't.
- **One LLM path.** ChatGPT OAuth only — no decision fatigue, no API key juggling.
- **Open source, MIT.**

---

## Platforms

| Platform | Status |
|----------|--------|
| Windows 10/11 | 🚧 In progress (v1 target) |
| WSL2 (running the Windows .exe) | 🚧 In progress (v1 target) |
| macOS | Planned (v2) |
| Native Linux | Planned (v3) |

Why Windows-first: WSL2 can't directly access audio or screen, so the binary has to be a Windows process. From WSL2 you can launch it via `cmd.exe /c noru.exe`, but it runs as a full Windows process with WASAPI / Win32 access.

---

## Install (when ready)

Download the latest `noru.exe` from [GitHub Releases](https://github.com/sespinosa/noru/releases) and double-click. That's it.

To enable AI features (optional): open Settings → AI Features (experimental) → Sign in with ChatGPT. You'll need a ChatGPT Plus / Pro subscription. The flow is unofficial and may break — the rest of the app keeps working regardless.

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
