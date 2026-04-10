# noru — Architectural Plan

> Local-first desktop perception layer for AI agents. Gives them ears and eyes via MCP.

---

## 1. Vision

noru is the sensory counterpart to [obsidian-surface](https://github.com/sespinosa/obsidian-surface). Where obsidian-surface gives an AI agent a *display* (the ability to show, create, and organize content through Obsidian), noru gives the agent *perception* (the ability to observe what the user is doing on their machine — system audio, microphone, screen).

Together they form a complete perception-action loop: noru observes, obsidian-surface presents.

The name **noru** (乗る) is Japanese for *to board*, *to ride*, *to get on* — it describes what the tool does. It mounts onto your system's audio and screen, riding along with your workflow and feeding context to AI agents.

## 2. Core principles

- **Local-first, private by default.** All processing happens on the user's machine. Nothing is transmitted externally. Ever.
- **Perception layer, not a product.** noru is not a closed meeting-notes app. It's an open, composable toolkit. The agent decides what to do with the context.
- **Open source, MIT licensed.** Built for the community.
- **Lightweight when idle.** The meeting detector runs continuously on CPU; transcription only when needed.
- **Extensible.** Hook system for integrations, but the core stays lean.

## 3. Architecture overview

A **single Rust binary** (`noru.exe`) that operates in three modes selected by CLI flag:

| Mode | Invocation | Purpose |
|------|------------|---------|
| Tray app (default) | `noru.exe` | System tray daemon with UI for config, transcripts, controls |
| MCP server | `noru.exe --mcp` | stdio JSON-RPC MCP server, what coding agents spawn |
| Setup / CLI | `noru.exe --setup`, `--status`, `--print-config <client>` | Install flow, queries, helpers |

When `--mcp` is invoked while a tray instance is running, it forwards via named pipe to the daemon (single source of truth). When no daemon is running, it operates standalone.

### Why one binary, not multiple

Avoiding IPC for high-bandwidth audio data (capture → Whisper) is critical. Splitting into separate processes (sidecars, services) adds complexity and overhead with no real benefit. The single-binary approach also simplifies distribution: one thing to download, one thing to install, one thing to update.

### The three workstreams

Independent modules with clear interface contracts. Each can be developed and tested in isolation:

#### 3.1 Whisper (audio + transcription)
Captures system audio (WASAPI loopback) + microphone, resamples to 16kHz mono, feeds to whisper.cpp via [`whisper-rs`](https://github.com/tazz4843/whisper-rs). Models auto-download on first use to `~/.noru/models/`. Build supports both CPU and CUDA via feature flag.

**Interface:**
```rust
pub struct WhisperEngine { /* ... */ }
impl WhisperEngine {
    pub fn new(model: &Path, lang: Option<String>) -> Result<Self>;
    pub fn transcribe(&self, samples: &[f32], time_offset_ms: i64) -> Result<Vec<Segment>>;
}
```

#### 3.2 Detection (meeting detection)
Two-pass detection:
1. **Process/window heuristics (cheap, every ~5s):** enumerate Windows processes, check for known meeting apps (`zoom.exe`, `Teams.exe`, `slack.exe`), check window titles (`"Zoom Meeting"`, `"Meet - "`, etc.).
2. **Image classifier (confirmation, on-demand):** when heuristics suggest a meeting, take a screenshot and run a small ONNX classifier to confirm an active call (vs. just the launcher being open).

**Interface:**
```rust
pub fn get_meeting_state() -> MeetingState;
// { in_meeting: bool, platform: Option<Platform>, confidence: f32 }
```

#### 3.3 App shell (Tauri + MCP + glue)
Tauri v2 app:
- System tray (always visible status)
- React/TS frontend for config, transcript viewer, controls
- MCP server (stdio mode)
- Storage (transcript history, search)
- The wiring that connects Whisper + Detection

## 4. Distribution model

### npm-first

Developers using MCP-compatible coding agents are the primary v1 audience. Install is one command:

```bash
npx -y noru
# or
npm install -g noru
```

The npm package is a thin Node.js shim (~50 LOC) that:
1. Detects platform (WSL2, Windows; v2: Mac)
2. Resolves the binary in this order:
   - Windows PATH (`where noru.exe`) — set by a permanent install
   - `~/.noru/bin/noru.exe` — npm bootstrap copy
   - Downloads to `~/.noru/bin/noru.exe` if not present
3. Spawns the binary with `--mcp`, normalizes stdio across the WSL/Windows boundary, forwards signals

### Why a Node shim is necessary

The MCP stdio contract is bidirectional and long-lived. When the boundary is WSL2 → Windows .exe, line endings (CRLF), encoding (Windows codepage), buffering, and signal forwarding all become unreliable. A Node.js shim using `child_process.spawn()` handles all of these correctly. The shim is invisible to the user — they paste the same 4-line config into any MCP client and it works regardless of platform.

### The MCP install snippet (universal)

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

### Going from "MCP install" to "permanent install"

The MCP exposes a `noru_setup` tool. From inside any MCP client, the user can ask the agent to install noru permanently:

```
"use the noru_setup tool to install permanently with autostart"
```

The tool runs inside `noru.exe` (which is a full Windows process with registry/filesystem access) and:
- Copies itself to `%LOCALAPPDATA%\Programs\noru\noru.exe`
- Adds the install dir to user PATH (`HKCU\Environment\Path`)
- Optional: registers Startup folder shortcut for auto-start
- Optional: writes MCP config to detected clients

After this, the npm bootstrap copy in `~/.noru/bin/` becomes a fallback. The launcher's PATH-first lookup means the canonical install is always preferred. The bootstrap is never deleted (small file, safety net) until `npm uninstall noru`.

### MCP client install — minimal, copy-paste first

We don't try to auto-detect and rewrite every client's config file. That's a maintenance trap. Instead:

- **Copy-paste snippet** is the official install for any client. `noru.exe --print-config <client>` prints it pre-formatted for a named client and tells the user where to paste it.
- **Auto-install only via documented client CLIs.** For Claude Code: `claude mcp add noru -- npx -y noru`. We don't parse and rewrite JSON config files ourselves.

This keeps maintenance burden tiny and works with clients we don't even know about yet.

## 5. Build pipeline

### v1: GitHub Actions, GitHub-hosted Windows runners

- `runs-on: windows-latest`
- Pre-installed: MSVC, cmake, Rust, Git
- Workflow installs LLVM (for libclang), runs `cargo build --release`, uploads `noru.exe` as an artifact
- CUDA build is a separate job that installs CUDA toolkit before `cargo build --release --features cuda`
- Triggered on push to main, on tag for releases
- ~5-10 min build time, faster with Cargo cache

### Future: self-hosted runner on the dev's Windows machine

Optional opt-in for faster iteration on CUDA builds. Configured but not enabled by default. Security: only run for collaborators / specific branches, never on public PRs.

### Local Windows builds — explicitly avoided in v1

Native Windows builds require: VS Build Tools (~10GB), Rust toolchain, cmake, LLVM/libclang, optional CUDA toolkit. ~10-15GB of installs and configuration. We avoid this by relying on GH Actions for all Windows artifacts. When debugging an inevitable Windows-specific issue, options are: Windows Sandbox (Win 11 Pro), a dedicated VM, or biting the bullet on the host install.

## 6. Platform scope

| Stage | Platforms | Notes |
|-------|-----------|-------|
| v1 | WSL2 + native Windows | Primary target. WASAPI audio, Windows screen capture, single x86_64 binary. |
| v2 | macOS | CoreAudio + ScreenCaptureKit. Whisper builds with Metal. Tauri UI is already cross-platform. Mostly platform-shim work, no architectural changes. |
| v3 | Native Linux | ALSA/PulseAudio (cpal already supports). The shim's WSL detection is removed. |

## 7. Tech stack

- **Language:** Rust (2021 edition)
- **UI shell:** Tauri v2 (Rust backend + React/TS frontend, uses WebView2 on Windows)
- **Whisper:** `whisper-rs` (wraps whisper.cpp, supports CUDA via feature flag)
- **Audio capture:** `cpal` (cross-platform, uses WASAPI on Windows)
- **Resampling:** `rubato` (high-quality FFT-based resampler)
- **WAV I/O:** `hound`
- **Image classifier (later):** `ort` (ONNX Runtime bindings)
- **Windows APIs:** `windows-rs` (Microsoft official)
- **MCP server:** raw JSON-RPC over stdio (no SDK dependency)
- **HTTP (model downloads):** `ureq`
- **CLI:** `clap`
- **npm shim:** Node.js, ~50 LOC, no dependencies

## 8. Status

### Done
- [x] Project scaffolded (Cargo.toml, .gitignore, src structure)
- [x] Audio capture module (`src/audio.rs`) — cpal-based, multi-format, mono conversion, resampling to 16kHz
- [x] WAV writer + WAV loader for testing
- [x] Whisper transcription module (`src/transcribe.rs`) — model loading, chunked transcription, segment output with timestamps
- [x] Model auto-download (`src/models.rs`) — by name (`tiny`, `base`, `small`, `medium`, `large-v3`, `large-v3-turbo`), saves to `~/.noru/models/`, progress callback for UI integration
- [x] CLI (`src/main.rs`) — file mode (transcribe a WAV) and live capture mode (records + transcribes in chunks)
- [x] Builds cleanly on Linux/WSL (verified with WAV file transcription)

### Next up (in this order)
- [ ] GitHub Actions workflow — Windows build, produces `noru.exe`
- [ ] Verify the Windows artifact runs (smoke test)
- [ ] Real audio test — transcribe a known speech sample, verify quality
- [ ] Detection module — process enumeration heuristics for meeting apps

### Later
- [ ] Image classifier for meeting confirmation (ONNX, small model)
- [ ] Tauri app shell (system tray, React UI)
- [ ] MCP server mode (`--mcp` flag)
- [ ] Setup tool (`noru_setup` MCP tool, permanent install logic)
- [ ] npm shim package
- [ ] Storage layer (transcript history, search)
- [ ] Hooks/events system for integrations
- [ ] CUDA build feature flag
- [ ] macOS support
