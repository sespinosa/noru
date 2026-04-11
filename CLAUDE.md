# noru — Working notes for Claude

This file orients new Claude Code sessions starting in this repo. **Read this first.** Then read [PLAN.md](./PLAN.md) for the v1 architecture and [ROADMAP.md](./ROADMAP.md) only if you need context on phases beyond v1.

---

## Hard rules

These are decisions already made. Do not relitigate them mid-session.

1. **The v1 scope is locked.** PLAN.md defines what ships in v1. New ideas go to ROADMAP.md as v1.1, v1.5, v2, v3, or v4. Do not expand v1 mid-session.
2. **No setup wizard, no first-run prompts.** The app must just work when the user opens the .exe.
3. **ChatGPT OAuth is the only AI provider in v1.** No BYO API key, no Ollama, no provider chooser. Lives in Settings → "AI Features (experimental)".
4. **Settings has 4 sections only:** General, Recording, Whisper, AI Features (experimental). Don't add a fifth.
5. **No npm package, no MCP server in v1.** Both arrive together in v1.5.
6. **No first-run wizard, no installer in v1.** Single .exe from GitHub Releases.
7. **Windows-only in v1.** macOS = v2. Native Linux = v3.
8. **Default branch is `master`,** not `main`. The user's GitHub account is configured this way.
9. **Commit author** is "Sebastián Espinosa <sespinosar@gmail.com>". Set per-repo, not globally.
10. **Don't include "Generated with Claude Code" or "Co-Authored-By: Claude"** in commit messages. The user's preference is clean, unbranded commits.

---

## Where we are

| | Status |
|---|---|
| Whisper transcription core (`src/audio.rs`, `src/transcribe.rs`, `src/models.rs`) | ✅ Done — verified on Linux/WSL with WAV input |
| GitHub Actions Windows build workflow (`.github/workflows/build.yml`) | ✅ Scaffolded, not yet verified end-to-end |
| Tauri restructure (`src-tauri/` + `ui/`) | ⏳ Not yet started — Phase 1 |
| Module stubs with locked interfaces | ⏳ Not yet started — Phase 1 |
| Storage / Detection / Auth+AI / Transcript UI / Settings UI | ⏳ Phase 2 — agent team |
| Auto-record orchestration / AI panel wiring / polish / release | ⏳ Phase 3 |

---

## Build phases

The v1 work breaks into three phases. Do them in order. **Do not skip ahead.**

### Phase 1 — Sequential, no agent team

The lead session does this work alone. The output is a building, running, empty Tauri app where every module compiles and every UI screen renders placeholder text.

1. **Verify the Windows build pipeline.** Trigger GH Actions on master, download the artifact, smoke-test `noru.exe --help` on a real Windows machine. If it fails, fix it before doing anything else.
2. **Add Tauri v2 to the project** (single coordinated change):
   - Restructure `src/` → `src-tauri/src/`
   - Add `ui/` directory with React + Vite scaffold
   - Add **all** v1 dependencies to `Cargo.toml` in one go: `tauri`, `tauri-plugin-tray`, `tauri-plugin-shell`, `tauri-plugin-opener`, `rusqlite`, `windows-rs`, `oauth2`, `serde`, `serde_json`, `tokio`, `tracing`
   - Add the TypeScript dependencies: React, Vite, the Tauri JS API
3. **Module stubs and the integration layer** — write empty implementations with locked function signatures:
   - `src-tauri/src/storage.rs` — pub fns with `unimplemented!()`, full type signatures
   - `src-tauri/src/detect.rs` — same pattern
   - `src-tauri/src/auth.rs` — same pattern
   - `src-tauri/src/ai.rs` — same pattern
   - `src-tauri/src/types.rs` — shared types referenced by multiple modules
   - `src-tauri/src/commands.rs` — Tauri command registry, all `#[tauri::command]` functions defined and wired into the Tauri builder, each delegating to its module
   - `src-tauri/src/lib.rs` — declares all modules, exposes the Tauri builder
   - `src-tauri/src/main.rs` — boots the app, sets up tray, opens window
   - `ui/src/App.tsx`, router, layout shell, placeholder components for `<TranscriptList />`, `<TranscriptViewer />`, `<Settings />`, `<AIPanel />`
   - `ui/src/api.ts` — typed wrappers around Tauri's `invoke()` for every command
4. **Commit Phase 1 as a baseline.** The build compiles. The app runs. Every module is `unimplemented!()`. Every UI component renders placeholder text. **No agents have run yet.**

After Phase 1, the parallel agents have non-overlapping playgrounds and stable interface contracts.

### Phase 2 — Agent team, parallel

Spawn an agent team with 5 teammates. Each owns a specific set of files and implements the stub interfaces from Phase 1. Teammates can message each other directly via `SendMessage` if they need to coordinate on shared types.

#### Team plan (use this exact structure when calling TeamCreate + Agent)

| Teammate name | `subagent_type` | Owns these files (and only these) | Implements |
|---|---|---|---|
| `storage` | general-purpose | `src-tauri/src/storage.rs`, `src-tauri/migrations/` | sqlite-backed transcript storage; CRUD for the meetings table; the function signatures already in the stub |
| `detect` | general-purpose | `src-tauri/src/detect.rs` | Windows process + window enumeration via `windows-rs`; debounced state machine; returns `MeetingState` |
| `auth-ai` | general-purpose | `src-tauri/src/auth.rs`, `src-tauri/src/ai.rs` | ChatGPT OAuth (PKCE flow against `auth.openai.com` using Codex CLI public client ID `app_EMoamEEZ73f0CkXaXp7hrann`); token storage at `~/.noru/auth.json`; the three AI calls (`summarize`, `extract_action_items`, `extract_key_decisions`) calling the Codex backend |
| `transcript-ui` | general-purpose | `ui/src/views/TranscriptList.tsx`, `ui/src/views/TranscriptViewer.tsx`, `ui/src/components/AIPanel.tsx` | Sidebar transcript list + transcript viewer with timestamps + AI panel with three buttons (Summarize / Action items / Key decisions) |
| `settings-ui` | general-purpose | `ui/src/views/Settings.tsx`, `ui/src/views/settings/General.tsx`, `Recording.tsx`, `Whisper.tsx`, `AIFeatures.tsx` | The 4-section Settings UI; AI Features section calls `auth::start_login` via the Tauri command bridge |

#### Team rules (include in every spawn prompt)

- **You may ONLY edit the files listed for your role.** No exceptions. Do not edit `Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `ui/src/App.tsx`, or any router/layout file. The lead handles those in Phase 1 and Phase 3.
- **Implement the locked interfaces from the stub files.** The function signatures are the contract. Do not change them — if you think a signature is wrong, send a message to the lead and wait for instructions.
- **Use the typed wrappers in `ui/src/api.ts`** for any Tauri command call from the frontend. Do not call `invoke()` directly.
- **When you complete your task, mark it done via TaskUpdate** and go idle. The lead will integrate.
- **Coordinate via SendMessage if you need a shared type defined.** If `auth-ai` needs a type from `storage`, message the `storage` teammate by name. Don't guess and don't duplicate.

#### Spawn prompt template

When the lead spawns a teammate, use a prompt structured like this:

```
You are the "{name}" teammate on the noru team.

Read CLAUDE.md and PLAN.md for project context. Read the stub file(s) you
own to understand the locked interface contract.

Your role: {one-sentence role description}
Your files (you may ONLY edit these): {file list}
Your interface contract: implement the function signatures already in the
stub files. Do not change the signatures.

Project rules:
- Open source meeting recording app, Windows-only in v1
- Local Whisper transcription, no audio leaves the machine
- ChatGPT OAuth is the only AI provider in v1
- No setup wizard, no first-run prompts

When done: mark your task complete via TaskUpdate and go idle. The lead will
integrate your work.

Coordinate with other teammates by name via SendMessage if you need cross-
cutting types or have questions about shared interfaces.
```

### Phase 3 — Sequential, no agent team

After all 5 teammates finish and the lead has merged their work, the lead does:

1. **Auto-record orchestration** (`src-tauri/src/orchestrator.rs`) — the glue that listens to detect events, starts/stops audio capture, runs Whisper, saves to storage. Sequential because it consumes 4 of the parallel module outputs.
2. **AI panel wiring** — make sure the AI panel in the transcript viewer correctly invokes the AI module via Tauri commands and displays results.
3. **End-to-end smoke test** — record a fake meeting, verify it shows up in the UI, sign in to ChatGPT, click summarize, see a summary.
4. **Polish** — error states, empty states, keyboard shortcuts, the small details. Tray icon animation when recording.
5. **First release v0.1.0** — tag, GH Actions builds the .exe, attach to GitHub Release.

---

## Architecture quick reference

Single Tauri v2 application. One executable. One process. One window plus the tray.

```
noru.exe
  ├── Rust backend (src-tauri/src/)
  │   ├── audio          cpal + WASAPI capture, resampling, WAV
  │   ├── transcribe     whisper-rs, model loading + chunked transcription
  │   ├── models         Whisper model auto-download
  │   ├── detect         process/window enumeration heuristics  ← Phase 2
  │   ├── storage        sqlite for transcripts                  ← Phase 2
  │   ├── auth           ChatGPT OAuth flow + token storage      ← Phase 2
  │   ├── ai             calls Codex backend with stored token   ← Phase 2
  │   ├── orchestrator   glue between detect, audio, transcribe  ← Phase 3
  │   └── commands       Tauri command registry (lead writes in Phase 1)
  │
  └── Frontend (ui/, React + TypeScript)
      ├── tray menu
      ├── main window (sidebar + transcript viewer + AI panel)   ← Phase 2
      └── settings (4 sections)                                  ← Phase 2
```

The `audio`, `transcribe`, and `models` modules already exist (the v1 dev work that's already done). The rest are the Phase 1 stubs and Phase 2 implementations.

---

## Conventions

- **Branch names** for agent team work: `claude/phase-2-{name}` (e.g., `claude/phase-2-storage`)
- **PR titles**: short, imperative, match the work
- **PR body**: include "Closes #N" if there's a tracking issue
- **Commits**: clean, focused, no Claude attribution
- **No `git add -A` or `git add .`** — always specify files by name
- **Always run `git status --short` before committing** to verify only intended files are staged
- **Don't commit secrets, .env files, or generated data**

---

## Tools available in this repo

- **Rust toolchain** (rustc + cargo, installed via rustup at `~/.cargo/`)
- **LLVM** at `/usr/lib/llvm-18/lib` — set `LIBCLANG_PATH=/usr/lib/llvm-18/lib` when building locally
- **cmake**, **build-essential**, **libasound2-dev**, **pkg-config** — installed system-wide
- **gh CLI** — authenticated as `sespinosa`
- **Whisper model `tiny`** already downloaded at `~/.noru/models/ggml-tiny.bin` for testing

---

## When not to use an agent team

Do NOT spawn a team for:
- Phase 1 work (sequential project restructure)
- Phase 3 work (sequential integration)
- Bug fixes touching multiple files
- Anything where the modules are not file-isolated
- Small changes that fit in one focused session

Use a team only when the work is genuinely parallel and file-isolated. Phase 2 of v1 is the canonical case.
