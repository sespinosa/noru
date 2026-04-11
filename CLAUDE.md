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

Spawn an agent team using the **`wshobson/agents` plugin** ([`agent-teams`](https://github.com/wshobson/agents/tree/main/plugins/agent-teams) plugin from the `claude-code-workflows` marketplace). It's the canonical, community-tested way to run parallel implementation work in Claude Code, with built-in tmux split-pane mode, slash commands (`/team-spawn`, `/team-status`, `/team-shutdown`), and 6 skills for team coordination.

#### Required setup (one-time, see below for status)

1. `tmux` must be installed (verified: tmux 3.4 ✅)
2. `~/.claude.json` must contain `"teammateMode": "tmux"` (set ✅)
3. `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` in `~/.claude/settings.json` env (set ✅)
4. The plugin must be installed in the user's Claude Code:
   ```
   /plugin marketplace add wshobson/agents
   /plugin install agent-teams@claude-code-workflows
   ```

#### How the lead spawns the team

Use the plugin's `/team-spawn` command. The recommended preset for Phase 2 work is `feature` (parallel feature development with file ownership boundaries):

```
/team-spawn feature --team-size 3 --plan-first
```

The `--plan-first` flag makes the lead draft a work decomposition plan that you approve as a human before any teammate spawns. Once approved, the team spawns into tmux panes, each teammate working in its own pane with file ownership enforced.

Alternatively, for more explicit control:

```
/team-spawn custom --name noru-phase-2 --members 3
```

Then the lead interactively configures the team, referencing this CLAUDE.md and the role definitions in `.claude/agents/`.

#### Team composition (3 teammates, consolidated from the original 5)

Per community guidance (3 teammates is the sweet spot for implementation work), the noru Phase 2 team is consolidated as follows. The role definitions still live in `.claude/agents/` as project-scope subagent definitions and the lead can reference them by name when spawning teammates.

| Teammate name | Role file (project scope) | Owns these files |
|---|---|---|
| `backend` | implements storage + detect modules together | `src-tauri/src/storage.rs`, `src-tauri/src/detect.rs`, `src-tauri/migrations/*.sql` |
| `auth-ai` | [`.claude/agents/noru-auth-ai.md`](.claude/agents/noru-auth-ai.md) | `src-tauri/src/auth.rs`, `src-tauri/src/ai.rs` |
| `frontend` | implements transcript views + settings UI together | `ui/src/views/TranscriptList.tsx`, `TranscriptViewer.tsx`, `Settings.tsx`, all `ui/src/views/settings/*.tsx`, `ui/src/components/AIPanel.tsx` |

The original 5 agent files (`noru-storage.md`, `noru-detect.md`, `noru-auth-ai.md`, `noru-transcript-ui.md`, `noru-settings-ui.md`) remain in `.claude/agents/` as **detailed role references** — they document file scopes, interface contracts, and implementation specs. The lead can pass them to teammates via the spawn prompt:

> *"Spawn the backend teammate. Read .claude/agents/noru-storage.md AND .claude/agents/noru-detect.md for the full role specs. Implement both modules. File scope: src-tauri/src/storage.rs, src-tauri/src/detect.rs, and src-tauri/migrations/*.sql. Use isolation: worktree."*

This gives us the plugin's tested orchestration infrastructure (tmux panes, slash commands, skills) AND our noru-specific role specs together.

#### Team rules (apply to every Phase 2 teammate — read this if you are one)

- **You may ONLY edit the files listed in your agent definition.** No exceptions. Do not edit `Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `ui/src/App.tsx`, `ui/src/api.ts`, or any router/layout file. The lead handles those in Phase 1 and Phase 3.
- **Implement the locked interfaces from the Phase 1 stub files.** The function signatures are the contract. Do not change them. If a signature looks wrong, send a message to the lead via `SendMessage` and wait for instructions.
- **Use the typed wrappers in `ui/src/api.ts`** for any Tauri command call from the frontend. Do not call `invoke()` directly. If a wrapper is missing, message the lead.
- **Coordinate by name via SendMessage** if you need a shared type or have a question for another teammate. Refer to teammates by their name (`storage`, `detect`, `auth-ai`, `transcript-ui`, `settings-ui`), not their UUID.
- **When your task is complete: mark it done via `TaskUpdate` and go idle.** The lead will integrate.
- **Do not spawn nested teams.** Only the lead manages the team.

#### Worktree isolation

All 5 Phase 2 agent definitions in `.claude/agents/` declare `isolation: worktree` in their frontmatter. When the lead spawns a teammate from one of these definitions, that teammate runs in its own temporary git worktree with its own working directory and branch. **Teammates cannot conflict at the filesystem level** — even if two teammates accidentally tried to edit the same file, they'd be writing to different worktrees and the changes would surface only at merge time as a clean PR conflict.

To avoid disk bloat from 5 parallel worktrees, `.claude/settings.json` configures `worktree.symlinkDirectories` to symlink `node_modules` and `.cache` from the main repo into each worktree. **`target/` is deliberately NOT symlinked** — cargo's incremental compilation can corrupt itself if multiple builds write to the same `target/` directory simultaneously. Each worktree gets its own `target/`, accepting some disk bloat (~1-2 GB × 5 = ~5-10 GB during Phase 2) for build safety. Worktrees are auto-cleaned after merge.

Worktree isolation requires Claude Code v2.1.49 or later.

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
