# noru — Architecture & Plan

This document defines the **v1 core**: what noru is, how it's built, and what ships first. The expanded vision (voice activation, agent dispatch, full Jarvis) lives in [ROADMAP.md](./ROADMAP.md).

---

## 1. What noru is

**Open-source, local-first meeting transcription that AI agents can actually use.**

A native daemon that records your meetings, transcribes them locally with Whisper, and exposes the transcripts (plus meeting state, screenshots, and events) to any MCP-compatible AI agent. Your audio never leaves your machine.

Architecturally, noru is the **perception layer for AI agents** — meeting transcription is the first concrete use case, but the same primitives (audio, screen, events) enable broader desktop awareness. It's the sensory counterpart to [obsidian-surface](https://github.com/sespinosa/obsidian-surface): obsidian-surface gives an agent a *display*; noru gives it *senses*.

The name **noru** (乗る — Japanese for *to ride*, *to board*) describes what it does: it mounts onto your audio and screen, riding along with your workflow.

## 2. Core principles

- **Local-first, private by default.** All processing happens on your machine. Nothing is transmitted externally. Ever.
- **Perception layer, not a closed product.** noru is not another Otter/Granola. It's an open toolkit. The agent decides what to do with the data.
- **Lightweight when idle.** Detection runs cheap and continuously; transcription only when it should.
- **Open source, MIT.** Built for the community.

## 3. Architecture

A **single Rust binary** (`noru.exe`) with multiple modes selected by CLI flag:

| Mode | Invocation | Purpose |
|------|------------|---------|
| Tray app (default) | `noru.exe` | System tray daemon with UI for config, transcripts, controls |
| MCP server | `noru.exe --mcp` | stdio JSON-RPC MCP server, what coding agents spawn |
| Setup / CLI | `noru.exe --setup`, `--status`, `--print-config <client>` | Install flow, queries, helpers |

When `--mcp` is invoked while a tray instance is already running, it forwards requests via named pipe to the daemon (single source of truth). When no daemon is running, it operates standalone.

### Why one binary, not multiple processes

Avoiding IPC for high-bandwidth audio data (capture → Whisper) is critical. Splitting into sidecars or services adds complexity and overhead with no real benefit. The single-binary approach also simplifies distribution.

## 4. The three workstreams

Independent modules with clear interface contracts. Each can be developed and tested in isolation:

### 4.1 Whisper (audio + transcription)

Captures system audio (WASAPI loopback) + microphone, resamples to 16kHz mono, feeds to whisper.cpp via [`whisper-rs`](https://github.com/tazz4843/whisper-rs). Models auto-download on first use to `~/.noru/models/`. Build supports both CPU and CUDA via feature flag.

```rust
pub struct WhisperEngine { /* ... */ }
impl WhisperEngine {
    pub fn new(model: &Path, lang: Option<String>) -> Result<Self>;
    pub fn transcribe(&self, samples: &[f32], time_offset_ms: i64) -> Result<Vec<Segment>>;
}
```

### 4.2 Detection (meeting detection)

Two-pass detection:

1. **Process/window heuristics (cheap, every ~5s).** Enumerate Windows processes, check for known meeting apps (`zoom.exe`, `Teams.exe`, `slack.exe`), check window titles (`"Zoom Meeting"`, `"Meet - "`, etc.).
2. **Image classifier (confirmation, on-demand).** When heuristics suggest a meeting, take a screenshot and run a small ONNX classifier to confirm an active call (vs. just the launcher being open).

```rust
pub fn get_meeting_state() -> MeetingState;
// { in_meeting: bool, platform: Option<Platform>, confidence: f32 }
```

### 4.3 App shell (Tauri + MCP + glue)

[Tauri v2](https://v2.tauri.app/) app:
- System tray (always visible status)
- React/TS frontend for config, transcript viewer, controls
- MCP server (stdio mode)
- Storage (transcript history, search)
- The wiring that connects Whisper + Detection + the messaging spine

## 5. The messaging spine: mailbox + three interfaces

noru is not just a sensor — it's a **message bus** that agents read from and write to. This is the spine that ties everything together.

### 5.1 The mailbox model

Instead of trying to push events to agents (which MCP doesn't support well today, see [ROADMAP](./ROADMAP.md)), noru exposes a **mailbox** that agents check on their own schedule:

```
Tools exposed by noru MCP:
  noru.inbox.check()                       → returns pending messages, marks read
  noru.inbox.peek()                        → same but doesn't mark read
  noru.inbox.send(to, body, [meta])        → put a message in someone else's inbox
  noru.inbox.subscribe(topics)             → register interest in event types

  noru.schedule(message, when)             → deliver a message at a future time
  noru.schedule.recurring(message, cron)   → deliver on a recurring schedule
  noru.schedule.list([owner])              → see what's scheduled
  noru.schedule.cancel(id)                 → cancel a scheduled delivery

Resources:
  noru://inbox/<agent_id>                  → live view of messages
  noru://meetings/current                  → current meeting state
  noru://transcripts/<id>                  → specific transcript
  noru://schedule                          → all pending scheduled deliveries
```

A message:

```json
{
  "id": "msg_01H...",
  "from": "noru.detector" | "agent:claude-code-session-42" | ...,
  "to": "agent:current",
  "ts": "2026-04-10T14:23:00Z",
  "topic": "meeting.started" | "transcript.segment" | "user.message" | ...,
  "body": { ... },
  "priority": "normal" | "high",
  "ack_required": false
}
```

The mailbox isn't just for noru events — it's a generic agent-to-agent message bus. Multiple agents (Claude Code, Codex, agents inside Nexus) can read each other's messages and react. Noru is the post office.

### 5.1.1 The time dimension

Messages don't just flow now — they flow *across time*. The same mailbox primitive supports **scheduled and recurring delivery**, which turns noru from a passive sensor into a **proactive scheduler**.

A scheduled message is just a normal message with a `scheduled_for` timestamp. Until that time, it sits in the schedule store; at that time, it lands in the recipient's inbox like any other message. Recurring messages are stored with a cron expression and re-queued after each delivery.

This is a small implementation change (one extra column in sqlite, a background loop that polls for due messages) but it unlocks a category of use cases that's otherwise impossible:

- **Reminders**: *"remind me about the standup at 10am tomorrow"* — agent calls `noru.schedule({topic: "reminder", body: "standup"}, "tomorrow 10:00")`. At 10am, the message appears in the inbox; the hook helper or SSE stream wakes the agent.
- **Recurring routines**: *"every weekday at 5pm, summarize today's meetings and post the result"* — agent calls `noru.schedule.recurring(...)` once. From then on, noru handles it.
- **Watchers**: *"check meeting state every 5 minutes for the next hour and tell me when it ends"* — agent schedules a recurring check; the messages appear in the inbox; agent responds when the trigger condition is met.
- **Trigger chains**: *"when this transcript segment matches X, schedule a follow-up message in 10 minutes"* — agents can compose temporal logic out of mailbox primitives.

The agent never needs to maintain its own scheduler. It delegates time entirely to noru. From the LLM's perspective, time becomes a resource it can allocate ("send me X at time T") instead of something it has to keep track of mid-session.

**Implementation:** sqlite table with `(id, message_json, scheduled_for, recurring_cron, owner_agent_id, created_at)`. A tokio task polls every second for due messages and inserts them into the live inbox. Cron parsing via the [`cron`](https://crates.io/crates/cron) crate. Total: maybe 200 LOC.

**Why this lives in v1:** because every other capability we add later (voice activation, dispatch, computer control) becomes more powerful when it can be scheduled. A voice command "summarize tomorrow's meetings at 6pm" needs the scheduler; an attention state machine that decays over time needs internal scheduling; a watcher that wakes the agent when something happens needs scheduled poll messages. Time is foundational, not a feature.

### 5.2 Three interfaces, one message store

```
                    ┌─────────────────────────┐
                    │   noru daemon (Rust)    │
                    │                         │
                    │  ┌─────────────────┐   │
                    │  │  Internal bus   │◄──┼── detector events
                    │  │  (mpsc channel) │◄──┼── transcript segments
                    │  └────────┬────────┘   │
                    │           │             │
                    │  ┌────────▼────────┐   │
                    │  │  Message store  │   │  ← persistent inbox
                    │  │  (sqlite)       │   │     per agent_id
                    │  └────────┬────────┘   │
                    │           │             │
                    │  ┌────────┴────────┐   │
                    │  │  Three faces    │   │
                    │  │  - MCP (pull)   │◄──┼── any MCP client
                    │  │  - SSE (push)   │◄──┼── nexus / harnesses
                    │  │  - Hook helper  │◄──┼── claude-code hook
                    │  └─────────────────┘   │
                    └─────────────────────────┘
```

The **message store** (sqlite) is the source of truth. The three faces all read from it:

1. **MCP server (pull)** — `noru.exe --mcp`. Standard MCP tools and resources. Works in any MCP client. The default way agents interact with noru.

2. **Event stream (push)** — SSE/WebSocket endpoint at `localhost:<port>/events`. For harnesses (like [Nexus](#)) that want to react to events as they happen. The harness owns the routing — it can inject events into agent sessions however it likes. This sidesteps the MCP push limitation entirely by giving harnesses a clean stream to subscribe to.

3. **Hook helper (Claude Code specific)** — `noru.exe --hook-context`. A one-shot command that prints recent unread events. Install it as a Claude Code `UserPromptSubmit` hook and you get pseudo-push: every time the user submits a prompt, the hook runs, the agent sees "you have N new messages in your inbox" injected into its context, and decides whether to call `noru.inbox.check()`.

All three are reading the same data. They're transports, not features.

## 6. Distribution

### 6.1 npm-first

Developers using MCP-compatible coding agents are the primary v1 audience. Install is one command:

```bash
npx -y noru
```

The npm package is a thin Node.js shim (~50 LOC) that:
1. Detects platform (WSL2 or Windows for v1)
2. Resolves the binary in this order:
   - Windows PATH (`where noru.exe`) — set by a permanent install
   - `~/.noru/bin/noru.exe` — npm bootstrap copy
   - Downloads to `~/.noru/bin/noru.exe` if not present
3. Spawns the binary with `--mcp`, normalizes stdio across the WSL/Windows boundary, forwards signals

### 6.2 Why a Node shim is necessary

The MCP stdio contract is bidirectional and long-lived. When the boundary is WSL2 → Windows .exe, line endings (CRLF), encoding (Windows codepage), buffering, and signal forwarding all become unreliable. A Node.js shim using `child_process.spawn()` handles all of these correctly. The shim is invisible to the user — they paste the same 4-line config into any MCP client and it works regardless of platform.

### 6.3 The MCP install snippet (universal)

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

Four lines. Works in any MCP client (Claude Code, Claude Desktop, Cursor, Codex CLI, OpenCode, Continue, Cline, Zed, Windsurf, …).

### 6.4 Promotion to permanent install

The MCP exposes a `noru_setup` tool. From inside any MCP client, the user can ask the agent:

> *"use the noru_setup tool to install permanently with autostart"*

The tool (running as a Windows process with full filesystem/registry access):
- Copies the binary to `%LOCALAPPDATA%\Programs\noru\noru.exe`
- Adds the install dir to user PATH (`HKCU\Environment\Path`)
- Optional: registers Startup folder shortcut for auto-start
- Optional: writes MCP config to detected clients

After this, the npm bootstrap copy in `~/.noru/bin/` becomes a fallback (the launcher's PATH-first lookup means the canonical install is preferred). The bootstrap is never deleted (small file, safety net) until `npm uninstall noru`.

### 6.5 Auto-install into MCP clients — minimal, copy-paste first

We don't try to auto-detect and rewrite every client's config file. That's a maintenance trap. Instead:

- **Copy-paste snippet** is the official install for any client. `noru.exe --print-config <client>` prints it pre-formatted and tells the user where to paste it.
- **Auto-install only via documented client CLIs.** For Claude Code: `claude mcp add noru -- npx -y noru`. We don't parse and rewrite JSON config files ourselves.

This keeps maintenance burden tiny and works with clients we don't even know about yet.

## 7. Build pipeline

### v1: GitHub Actions, GitHub-hosted Windows runners

- `runs-on: windows-latest`
- Pre-installed: MSVC, cmake, Rust, Git
- Workflow installs LLVM (for libclang), runs `cargo build --release`, uploads `noru.exe` as an artifact
- CUDA build is a separate job that installs CUDA toolkit before `cargo build --release --features cuda`
- Triggered on push to main, on tag for releases
- ~5-10 min build time, faster with Cargo cache

### Future: self-hosted runner

Optional opt-in for faster CUDA iteration on the dev's Windows machine. Configured but not enabled by default. Security: only run for collaborators / specific branches, never on public PRs.

### Local Windows builds — explicitly avoided in v1

Native Windows builds require: VS Build Tools (~10GB), Rust toolchain, cmake, LLVM/libclang, optional CUDA toolkit. ~10-15GB of installs. We avoid this by relying on GH Actions for all Windows artifacts. When debugging an inevitable Windows-specific issue, options are: Windows Sandbox, a dedicated VM, or biting the bullet on the host install.

## 8. Platform scope

| Stage | Platforms | Notes |
|-------|-----------|-------|
| v1 | WSL2 + native Windows | Primary target. WASAPI audio, Windows screen capture, single x86_64 binary. |
| v2 | macOS | CoreAudio + ScreenCaptureKit. Whisper builds with Metal. Tauri UI is already cross-platform. Mostly platform-shim work, no architectural changes. |
| v3 | Native Linux | ALSA/PulseAudio (cpal already supports). The shim's WSL detection is removed. |

## 9. Tech stack

- **Language:** Rust (2021 edition)
- **UI shell:** Tauri v2 (Rust backend + React/TS frontend, uses WebView2 on Windows)
- **Whisper:** [`whisper-rs`](https://github.com/tazz4843/whisper-rs) (wraps whisper.cpp, supports CUDA via feature flag)
- **Audio capture:** [`cpal`](https://github.com/RustAudio/cpal) (cross-platform, uses WASAPI on Windows)
- **Resampling:** [`rubato`](https://github.com/HEnquist/rubato) (high-quality FFT-based resampler)
- **WAV I/O:** [`hound`](https://github.com/ruuda/hound)
- **Image classifier (later):** [`ort`](https://github.com/pykeio/ort) (ONNX Runtime bindings)
- **Windows APIs:** [`windows-rs`](https://github.com/microsoft/windows-rs) (Microsoft official)
- **MCP server:** raw JSON-RPC over stdio (no SDK dependency)
- **Message store:** sqlite via [`rusqlite`](https://github.com/rusqlite/rusqlite)
- **HTTP (model downloads):** [`ureq`](https://github.com/algesten/ureq)
- **CLI:** [`clap`](https://github.com/clap-rs/clap)
- **npm shim:** Node.js, ~50 LOC, no dependencies

## 10. Status

### Done

- [x] Project scaffolded (Cargo.toml, .gitignore, src structure)
- [x] Audio capture module (`src/audio.rs`) — cpal-based, multi-format, mono conversion, resampling to 16kHz
- [x] WAV writer + WAV loader for testing
- [x] Whisper transcription module (`src/transcribe.rs`) — model loading, chunked transcription, segment output with timestamps
- [x] Model auto-download (`src/models.rs`) — by name (`tiny`, `base`, `small`, `medium`, `large-v3`, `large-v3-turbo`), saves to `~/.noru/models/`, progress callback for UI integration
- [x] CLI (`src/main.rs`) — file mode (transcribe a WAV) and live capture mode (records + transcribes in chunks)
- [x] Builds cleanly on Linux/WSL (verified with WAV file transcription)
- [x] GitHub Actions workflow for Windows + Linux builds

### Next up (in this order)

- [ ] Verify the Windows GH Actions build produces a working `noru.exe`
- [ ] Real audio test — transcribe a known speech sample on Linux, verify quality
- [ ] Detection module skeleton — process enumeration heuristics
- [ ] Mailbox / message store skeleton (sqlite, message types)
- [ ] Scheduler — scheduled and recurring message delivery on top of the mailbox
- [ ] MCP server mode (`--mcp` flag) — minimal first version
- [ ] npm shim package (the bootstrap launcher)

### Then

- [ ] Tauri app shell (system tray, basic React UI)
- [ ] Setup tool (`noru_setup` MCP tool, permanent install logic)
- [ ] SSE event stream interface
- [ ] Hook helper (`--hook-context`)
- [ ] Image classifier for meeting confirmation
- [ ] Storage layer (transcript history, search)
- [ ] CUDA build feature flag

### Beyond v1

See [ROADMAP.md](./ROADMAP.md) for the phased vision: voice activation, attention FSM, voice-to-agent dispatch, computer control.
