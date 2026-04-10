# noru — v1 Plan

This document defines **v1**: the standalone Windows app. Future phases (MCP integration, voice activation, agent dispatch, full Jarvis vision) live in [ROADMAP.md](./ROADMAP.md). v1 has to ship before any of that becomes relevant.

---

## 1. The premise

> *We can look at what you're doing and help you proactively when needed.*

The first proof of that premise is **auto-recording meetings, transcribing them locally, and helping you do something useful with the transcript via AI.**

That's the entire v1 product.

## 2. Audience

Two clearly distinct user types, no in-between:

- **Normies** — know ChatGPT.com, have a Plus subscription, double-click `.exe` to install. Want a tray app that *just works*. No API keys, no terminal, no MCP, no setup wizards, no questions.
- **Technical users** — same recording/transcription core. They can get extra value out of v1.5+ when MCP and other integrations land. For v1, they're a normie too — install the .exe, use the app.

**v1 must serve the normie path natively.** If a normie can't open the .exe and immediately start getting value, v1 has failed.

## 3. Core principles

- **Local-first, private by default.** Audio capture and Whisper transcription happen entirely on the user's machine. The only network calls noru ever makes are: (a) Whisper model download on first use, (b) ChatGPT API calls *only when the user has explicitly opted into the experimental AI features in Settings*.
- **Just works on first launch.** No setup wizard. No prompts. No questions. Open the .exe, the tray icon appears, you start a meeting, noru records and transcribes. Done.
- **AI features are opt-in and experimental.** Lives in Settings → "AI Features (experimental)". Easy to find for those who want it, invisible to those who don't.
- **One LLM path.** ChatGPT OAuth only. No BYO API keys, no Ollama, no provider chooser. One path, one experience, no decision fatigue.
- **Open source, MIT.**

## 4. What v1 does

### 4.1 Always-on meeting capture

A Tauri tray app that runs in the background. The tray icon shows current state at a glance:

| State | Icon |
|-------|------|
| Idle | gray microphone |
| Recording | red dot (clearly visible) |
| Transcribing | spinner |

Clicking the tray icon opens the main window (transcript browser).

### 4.2 Automatic meeting detection

Process and window heuristics, polled every ~5 seconds:

- Known meeting processes: `Zoom.exe`, `Teams.exe`, `slack.exe`, `discord.exe`, etc.
- Window title patterns: `"Zoom Meeting"`, `"Meet - "`, `"Microsoft Teams meeting"`, etc.
- Both signals required to trigger auto-recording (process running + meeting-shaped window) — reduces false positives from launchers being open.

When detected, noru starts recording automatically. The tray icon turns red. The user can stop or pause from the tray menu at any time.

**Manual recording** is also supported via the tray menu — for impromptu recordings outside of detected meetings.

### 4.3 Local audio capture and transcription

- **Capture:** WASAPI loopback (system audio) + microphone, mixed and saved as WAV
- **Transcription:** [`whisper-rs`](https://github.com/tazz4843/whisper-rs) wrapping `whisper.cpp`, CPU-only in v1
- **Models:** auto-downloaded to `~/.noru/models/` on first use. Default model: `base` (~140MB, decent quality, fast on CPU). User can change in Settings.
- **Output:** transcript stored as structured segments (text, start_ms, end_ms) in sqlite

### 4.4 Transcript browser

The main window. Three things:

1. **Sidebar:** chronological list of recorded meetings, with date, duration, detected platform, and transcript word count
2. **Viewer:** the transcript itself with timestamps, scrollable, searchable, copyable
3. **AI panel** (only visible when ChatGPT is connected): three buttons — *Summarize*, *Action items*, *Key decisions* — each runs a single LLM call against the transcript and shows the result in the panel

If the user has not connected ChatGPT, the AI panel shows a small unobtrusive line: *"AI features available — see Settings"*. No nagging, no popups.

### 4.5 Settings (4 sections, simple)

| Section | Contents |
|---------|----------|
| **General** | Auto-start with Windows. Where to save transcripts (default: `~/.noru/transcripts/`). Theme (light/dark/system). |
| **Recording** | Which meeting platforms to auto-detect (checkboxes). Audio devices (input + system audio source). Manual recording controls. |
| **Whisper** | Model selection (tiny / base / small / medium / large-v3). Download progress when changing. Language (auto / en / es / …). |
| **AI Features (experimental)** | "Sign in with ChatGPT" button. When signed in: account info, "Sign out", and a brief description of what the AI features do. Clear "experimental" label and a one-line warning that it relies on an unofficial OpenAI flow that may break. |

That's all of Settings. Four sections. No more.

### 4.6 ChatGPT OAuth (the only AI path)

When the user clicks "Sign in with ChatGPT" in Settings, noru opens the system browser to OpenAI's OAuth page using the public Codex CLI client ID (`app_EMoamEEZ73f0CkXaXp7hrann`) with PKCE. The same flow that [Cline](https://cline.bot/blog/introducing-openai-codex-oauth) and several other tools ship today.

After the user authorizes:
- Token is stored in `~/.noru/auth.json` (user-only filesystem permissions)
- Auto-refresh on expiry
- AI features in the transcript browser become active
- Settings shows "Signed in as <email>" and a "Sign out" button

The AI features make calls against the Codex backend endpoint, which uses the user's ChatGPT Plus / Pro subscription quota. **No API keys, no separate billing, no friction.** From the user's perspective: click a button, sign in with ChatGPT, AI features appear.

**Honest disclosure:** OpenAI has not officially blessed third-party use of the Codex client ID. It works today (Cline, Roo Code, opencode all ship it), but OpenAI could revoke it. We label the feature "experimental" and state this clearly in the UI. If the path breaks, AI features stop working — but the rest of the product (recording, transcription, browsing) is unaffected.

**No fallback in v1.** If the OAuth path breaks, the AI features show an error message and a link to the GitHub issue tracker. We do not ship BYO API key or local LLM as alternatives in v1 — that's complexity we don't need yet. If the situation changes and we need a backup, we add it in v1.1.

### 4.7 AI features

Three one-shot LLM calls, each a button in the AI panel of the transcript viewer:

1. **Summarize** — concise paragraph summary of the meeting
2. **Action items** — bulleted list of actionable items mentioned
3. **Key decisions** — bulleted list of decisions made

Each call sends the transcript text + a focused system prompt to the OpenAI endpoint and renders the result. Results are saved alongside the transcript so the user doesn't pay (in quota terms) to re-generate them.

That's the entire AI surface for v1. No chat, no Q&A, no "ask anything" interface, no embedded agent. Three buttons.

## 5. Architecture

A **single Tauri v2 application**. One executable. One process. One window (plus the tray).

```
noru.exe
  ├── Rust backend
  │   ├── audio       (cpal + WASAPI capture, resampling, WAV)
  │   ├── transcribe  (whisper-rs, model loading + chunked transcription)
  │   ├── models      (Whisper model auto-download)
  │   ├── detect      (process/window enumeration heuristics)
  │   ├── storage     (sqlite for transcripts)
  │   ├── auth        (ChatGPT OAuth flow + token storage)
  │   ├── ai          (calls Codex backend with stored token)
  │   └── tauri cmds  (the bridge between backend and frontend)
  │
  └── Frontend (React/TS)
      ├── tray menu     (open / start recording / stop / quit)
      ├── main window   (sidebar + transcript viewer + AI panel)
      └── settings      (4 sections)
```

Modules in `src/` map directly to this structure. Each is independently testable.

## 6. Distribution

### 6.1 GitHub Releases

The only distribution channel for v1 is GitHub Releases:

- Tagged releases (`v0.1.0`, `v0.1.1`, …) on the repo
- CI builds `noru.exe` via the Windows GH Actions workflow
- The release asset is the single `.exe` file
- Users download it directly and double-click

No installer, no MSI, no auto-updater in v1. The .exe is portable — drop it anywhere and run it. Auto-update can come in v1.1 once we know what we want to update.

### 6.2 No npm, no MCP

The npm package and MCP server were designed as a developer-facing distribution path that wraps the .exe in a Node shim and exposes its capabilities to AI agents over stdio. **Both are explicitly out of v1.**

The npm shim only exists to launch the MCP server. Without an MCP server, there's no reason for the shim. Both arrive together in v1.5 as a coordinated addition — see [ROADMAP.md](./ROADMAP.md).

## 7. Build pipeline

GitHub Actions, GitHub-hosted Windows runners. Already scaffolded in `.github/workflows/build.yml`:

- `runs-on: windows-latest`
- Installs LLVM via choco (for libclang, used by `whisper-rs-sys`)
- `cargo build --release`
- Smoke-test the binary (`--help`)
- Upload `noru.exe` as an artifact
- On tagged release, attach the .exe to the GitHub release

Local Windows builds are explicitly avoided — see PLAN.md history for the dependency list. We use GH Actions for all Windows artifacts.

## 8. Platform scope

**v1 is Windows only.** macOS comes in v2 along with the Tauri UI portability work (mostly free since Tauri is cross-platform — the work is in the audio capture and detection layers). Native Linux comes later. WSL2 users can run the Windows .exe directly via WSLInterop.

## 9. Tech stack

- **Language:** Rust (2021 edition) for the backend, TypeScript + React for the frontend
- **Shell:** [Tauri v2](https://v2.tauri.app/) — single binary, system tray plugin, WebView2 on Windows
- **Whisper:** [`whisper-rs`](https://github.com/tazz4843/whisper-rs) (CPU-only in v1)
- **Audio capture:** [`cpal`](https://github.com/RustAudio/cpal) (WASAPI on Windows)
- **Resampling:** [`rubato`](https://github.com/HEnquist/rubato)
- **WAV I/O:** [`hound`](https://github.com/ruuda/hound)
- **Storage:** [`rusqlite`](https://github.com/rusqlite/rusqlite) for transcripts and metadata
- **Process / window enumeration:** [`windows-rs`](https://github.com/microsoft/windows-rs) (Microsoft official)
- **HTTP (model downloads, OAuth, AI calls):** [`ureq`](https://github.com/algesten/ureq) — sync, simple, no async runtime drama for v1's needs
- **OAuth (PKCE):** [`oauth2`](https://github.com/ramosbugs/oauth2-rs) crate for the flow, custom token storage
- **CLI (for dev / debugging):** [`clap`](https://github.com/clap-rs/clap)

## 10. Status

### Done

- [x] Cargo project scaffolded
- [x] Audio capture module (`src/audio.rs`) — cpal-based, multi-format, mono conversion, resampling to 16kHz
- [x] WAV writer + WAV loader for testing
- [x] Whisper transcription module (`src/transcribe.rs`) — model loading, chunked transcription, segment output with timestamps
- [x] Model auto-download (`src/models.rs`) — by name (`tiny`, `base`, …, `large-v3-turbo`), saved to `~/.noru/models/`, progress callback for UI integration
- [x] Working CLI for dev/debug (file mode and live capture mode)
- [x] Builds cleanly on Linux/WSL (verified with WAV file transcription)
- [x] GitHub Actions workflow scaffolded (Windows + Linux build)

### Build order for v1

1. **Verify the Windows build pipeline works end-to-end.** Push to master, GH Actions runs, `noru.exe` artifact downloaded, `noru.exe --help` works on a real Windows machine.
2. **Add Tauri v2 to the project.** Restructure as `src-tauri/` + `ui/`. The existing Rust modules (`audio`, `transcribe`, `models`) move to `src-tauri/src/` and become library modules. The CLI mode is preserved for dev/debug behind a `--cli` flag.
3. **System tray scaffold.** Tray icon with three states (idle/recording/transcribing), menu with Open / Start / Stop / Quit, single main window that opens on click.
4. **Storage layer.** sqlite schema for transcripts: `(id, started_at, ended_at, platform, audio_path, transcript_json, summary, action_items, key_decisions)`. Migrations via [`refinery`](https://github.com/rust-db/refinery) or hand-rolled.
5. **Meeting detection module.** Process enumeration via `windows-rs`, window title matching, debounced state machine (must see signal for ≥3 consecutive polls before triggering, must lose signal for ≥3 polls before stopping).
6. **Auto-record orchestration.** Detection triggers audio capture; detection loss stops capture; on stop, transcribe and save to sqlite.
7. **Frontend v0: transcript browser.** React + TypeScript. Sidebar with meeting list, viewer with transcript text + timestamps, basic styling. No AI panel yet.
8. **Settings UI.** Four sections, very simple. General + Recording + Whisper + AI Features (the AI Features section just shows the "Sign in with ChatGPT" button and a description, no functionality yet).
9. **ChatGPT OAuth flow.** PKCE OAuth against `auth.openai.com` with the Codex client ID. Token storage in `~/.noru/auth.json`. Token refresh on expiry. Sign-in / sign-out UI in Settings.
10. **AI calls.** `ai::summarize`, `ai::action_items`, `ai::key_decisions`. Each is a function that takes a transcript and returns text/list. Calls the Codex backend with the stored token.
11. **AI panel in transcript viewer.** Three buttons; results saved alongside the transcript so they're not regenerated.
12. **Polish pass.** Keyboard shortcuts, error states, empty states, the small details. Tray icon animation when recording.
13. **First release.** Tag `v0.1.0`, GH Actions builds the .exe, attached to the release. Write a short README install section pointing to the release.

### Explicitly NOT in v1

- MCP server (`--mcp` flag) → v1.5
- npm package / Node shim → v1.5 (with MCP)
- Mailbox / scheduler / message store → v1.5 (with MCP)
- SSE event stream / hook helper → v1.5 (with MCP)
- BYO API key for AI → not planned (only ChatGPT OAuth)
- Local Ollama support → not planned
- AI Q&A / chat with transcript → v1.1 maybe
- Image classifier for meeting confirmation → v1.1 quality improvement
- CUDA-enabled Whisper build → v1.1 perf improvement
- Auto-updater → v1.1
- Voice activation, attention FSM, R2-D2 beeps → v2 (see ROADMAP)
- Voice → agent dispatch → v3 (see ROADMAP)
- Computer control / full Jarvis vision → v4 (see ROADMAP)
- macOS → v2
- Native Linux → v3
