# noru

Local-first meeting capture and transcription for Windows.

---

noru runs in your system tray and automatically detects meetings on Zoom, Teams, Google Meet, Slack, Discord, and Webex. When a meeting starts, it records system audio and your microphone, transcribes everything locally using [Whisper](https://github.com/openai/whisper), and stores the transcript on your machine. Your audio never leaves your computer. Optionally, sign in with your ChatGPT account to unlock AI-powered summaries, action items, and key decisions — no API keys, no configuration.

The name **noru** (乗る — Japanese for *to ride*, *to board*) describes what it does: it mounts onto your audio, riding along with your workflow.

## Features

- **Automatic meeting detection** — recognizes Zoom, Teams, Meet, Slack, Discord, and Webex via process and window heuristics
- **Local Whisper transcription** — CPU-based, models auto-download on first use, multiple model sizes available
- **Transcript browser** — searchable list of all recordings with timestamps, duration, and detected platform
- **AI features (experimental)** — one-click summarize, action items, and key decisions powered by your ChatGPT subscription
- **System tray** — always-on with clear state indicators (idle / recording / transcribing), manual record option
- **Settings** — four sections: General, Recording, Whisper, AI Features
- **Zero setup** — no installer, no wizard, no prompts. Open the .exe and it works

## Download

Get the latest release from [GitHub Releases](https://github.com/sespinosa/noru/releases/latest).

Single `.exe` file. No installer needed. Download, double-click, done.

## System requirements

- Windows 10 or 11 (x86_64)
- WebView2 runtime (ships with Windows 10 1803+ and all Windows 11)
- No other dependencies

## Quick start

1. Download `noru.exe` from [Releases](https://github.com/sespinosa/noru/releases/latest)
2. Run it — a tray icon appears
3. Start a meeting on any supported platform
4. noru detects it and begins recording automatically
5. When the meeting ends, the transcript appears in the built-in browser

For manual recordings outside of detected meetings, right-click the tray icon and select **Start Recording**.

## Settings

| Section | What it controls |
|---------|-----------------|
| **General** | Auto-start with Windows, transcript storage location, theme (light/dark/system) |
| **Recording** | Which platforms to auto-detect, audio device selection (input + system audio) |
| **Whisper** | Model size (tiny / base / small / medium / large-v3), language, download management |
| **AI Features (experimental)** | ChatGPT sign-in, account info, sign-out |

## AI Features (experimental)

To enable AI features:

1. Open Settings → AI Features (experimental)
2. Click **Sign in with ChatGPT**
3. Authorize in your browser (requires a ChatGPT Plus or Pro subscription)

Once connected, three buttons appear in the transcript viewer:

- **Summarize** — concise paragraph summary of the meeting
- **Action items** — bulleted list of actionable items mentioned
- **Key decisions** — bulleted list of decisions made during the meeting

Results are saved with the transcript so they don't need to be regenerated.

**Note:** The ChatGPT sign-in uses an unofficial OAuth flow (the same one used by Cline, Roo Code, and other tools). It works today but OpenAI could change it at any time. If the flow breaks, AI features stop working — recording and transcription are unaffected.

## Screenshots

Screenshots coming soon.

## Known limitations

- **Windows only.** macOS support is planned for v2, native Linux for v3.
- **ChatGPT OAuth only.** No BYO API key, no local LLM support. If the OAuth flow breaks, AI features are unavailable until a fix ships.
- **No auto-updater.** Check GitHub Releases for new versions manually.
- **Unsigned binary.** Windows SmartScreen may show a warning on first run. Click "More info" → "Run anyway".
- **CPU-only transcription.** CUDA/GPU acceleration is planned for a future release.

## Building from source

Requires:
- Rust toolchain (stable)
- Node.js 20+
- LLVM (for `whisper-rs-sys` / libclang)

```bash
cd ui && npm install && npm run build
cd ../src-tauri && cargo build --release
```

The release binary will be at `src-tauri/target/release/noru.exe`.

Windows builds are also available via GitHub Actions — see `.github/workflows/build.yml`.

## License

MIT
