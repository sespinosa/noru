# noru v1 — Phase 3 plan

Sequential integration work that turns the Phase 2 module baseline into a shippable v0.1.0 release. The lead drives every load-bearing step; a single teammate is spawned for the one genuinely parallel slice (frontend polish).

## Baseline

Phase 2 landed as three commits on `master`:

| SHA | What it brings | Notes |
|---|---|---|
| `0223a3a` | storage + detect + `migrations/001_initial.sql` | rusqlite at `~/.noru/noru.db`, `detect::start<F>` event-driven state machine, Windows-only real detector |
| `2d1afa4` | auth + ai | PKCE loopback OAuth at `~/.noru/auth.json`, Codex Responses API calls, model hard-coded `gpt-5` |
| `1b13b76` | transcript views, Settings (4 sections), AIPanel | 7 settings controls shimmed via `localStorage` keyed `noru:settings.<section>.<key>` with `// TODO(phase-3)` markers |

All 5 local commits pushed at the start of Phase 3 — the first real Windows GHA build against the full Tauri stack is running as this plan is written. A `build-watcher` teammate (haiku, bounded authority, fix-branch only) is watching it.

## Hybrid model — when does a teammate help?

CLAUDE.md says Phase 3 is sequential no-team. The user's clarification: use teams when genuinely compatible, stay sequential otherwise. After auditing file ownership, only **one** Phase 3 slice is genuinely parallel and file-isolated: the frontend polish pass. Everything else touches lead-owned files (`commands.rs`, `lib.rs`, `main.rs`, `api.ts`, `Cargo.toml`) and must stay sequential to avoid merge conflicts.

The result is a hybrid:

- **Lead sequential** (main session, opus): Steps 0 → 1 → 2 → 3 → 5 → 6 → 7
- **Teammate parallel** (frontend-polish, one agent, runs during Step 4 only): Step 4 owns `ui/src/views/*.tsx` + `ui/src/components/AIPanel.tsx`, spawned after Step 3 lands the api wrappers, merged before Step 5.
- **Build-watcher** (haiku, background, current task): watches first real GHA Windows build from Step 0 push. Authority: fix branch only, never master.

## Dependency graph

```
Step 0  Pre-flight sanitation (push, first Windows build via build-watcher)
  │
Step 1  Orchestrator module  ──────┐
  │                                 │
Step 2  Backend Phase 3 polish      │
  │     (prefs, stub fills, dialog) │
  │                                 │
Step 3  api.ts wrapper additions    │
  │                                 │
  ├──────────────────────────────┐  │
  │                              ▼  │
Step 4  Tray icon polish       Step 4′ Frontend polish (teammate)
        (lead, Rust)                 (ui polish, localStorage → api swap)
  │                              │
  └──────────────┬───────────────┘
                 ▼
Step 5  Integration verification (cargo + tsc + vite + manual devtools poke)
  │
Step 6  Windows E2E smoke test   ← REAL WINDOWS MACHINE REQUIRED
  │
Step 7  Release v0.1.0 (tag + GHA release build + attach artifact)
```

---

## Step 0 — Pre-flight sanitation

**Owner:** lead sequential + build-watcher (haiku, parallel)
**Environment:** WSL-safe

### Done

- [x] 5 unpushed commits pushed to `origin/master` (`b3811be..1b13b76`)
- [x] `Cargo.lock` already tracked — earlier claim of "untracked" was a cwd-resolution error, corrected
- [x] `build-watcher` teammate spawned on haiku to watch run `24284399588` with bounded fix authority

### Pending

- [ ] First real Windows GHA build completes green (watched in background)
- [ ] If it fails: watcher applies documented fix from its bounded catalog on `claude/phase-3-build-fix`, lead reviews and fast-forwards master
- [ ] Lead confirms: `noru.exe --help` smoke test in the workflow produced clap's generated help (not a crash)

### Verification gate

Workflow run at the tip of master is green. Artifact `noru-windows-x86_64` is present and downloadable. This is the first time we prove the Tauri + whisper-rs + windows-rs + oauth2 + ureq stack actually compiles on a GitHub Windows runner — don't skip this gate.

### Risks

- **Risk E (unverified Windows build):** resolved by this step — it's the entire point. Expected cold build is 15–25 min; don't panic at 10 min in.
- **Cargo.lock resolver drift:** possible if the Windows runner pulls a different resolver version. Watcher has this in its catalog.
- **whisper-rs native build on Windows:** requires `LIBCLANG_PATH` — workflow sets it via chocolatey llvm install; watcher has this in its catalog.

---

## Step 1 — Orchestrator module

**Owner:** lead sequential
**Environment:** WSL-safe for codegen + unit tests; manual start/stop path works; real detect→capture→transcribe loop needs Windows
**Files touched:**
- `src-tauri/src/orchestrator.rs` (new)
- `src-tauri/src/lib.rs` (register mod, maybe manage State)
- `src-tauri/src/commands.rs` (replace `recording_state` / `start_recording` / `stop_recording` stubs; wire orchestrator State into handlers)
- `src-tauri/src/main.rs` (possibly inject orchestrator into Tauri State at boot)

### What the orchestrator owns

A **finite state machine** that turns detect events into saved transcripts. Not "glue" — a real FSM with explicit states and transition rules.

```
┌──────┐  Started        ┌──────────┐  Ended       ┌──────────────┐
│ Idle │ ───────────────▶│Recording │─────────────▶│ Transcribing │
└──────┘                 └──────────┘              └──────┬───────┘
   ▲                           │                         │
   │                           │ stop_recording          │ completed
   │                           ▼                         ▼
   │                     ┌──────────┐             ┌──────────┐
   └─────────────────────│Persisting│◀────────────│Persisting│
     done/error          └──────────┘             └──────────┘
```

States (matches `types::RecordingState` where possible):
- `Idle` — no recording in flight
- `Recording { meeting_id, started_at, audio_path, AudioCapture }` — cpal/WASAPI capture running, writing WAV
- `Transcribing { meeting_id }` — WhisperEngine running on the completed WAV
- `Persisting { meeting_id }` — calling `storage::save_meeting` with the final `NewMeeting`
- (on error) → back to `Idle` + error event emitted

### API shape (orchestrator module)

```rust
pub struct Orchestrator {
    state: Arc<Mutex<OrchestratorState>>,
    app: AppHandle,
    detect_handle: Option<detect::DetectHandle>,
}

impl Orchestrator {
    pub fn new(app: AppHandle) -> Self { ... }
    pub fn start_auto_detect(&mut self) -> Result<()>;  // spawns detect::start with callback
    pub fn stop_auto_detect(&mut self);                 // drops the handle
    pub fn start_recording(&self, manual: bool) -> Result<RecordingState>;
    pub fn stop_recording(&self) -> Result<RecordingState>;
    pub fn recording_state(&self) -> RecordingState;
}
```

### Transition rules (the tricky part)

1. **`Idle` → `Recording`** happens on:
   - A `detect::MeetingStateChange::Started` event, IF `prefs::auto_detect == true` AND no recording is already in flight
   - An explicit `start_recording(manual=true)` Tauri command
   Allocate `meeting_id` (32-char hex, same format as storage IDs), create `audio_path = ~/.noru/audio/<id>.wav`, start `AudioCapture`, emit `recording://state` event.

2. **`Recording` → `Transcribing`** happens on:
   - A `detect::MeetingStateChange::Ended` event, IF the current recording was auto-started
   - An explicit `stop_recording()` Tauri command
   Finalize the WAV writer, transition state, spawn a `tokio::task::spawn_blocking` running `WhisperEngine::transcribe(audio_path)`, emit `recording://state` event.

3. **`Transcribing` → `Persisting`** happens when the whisper task completes successfully. Construct a `NewMeeting` from the transcript segments and call `storage::save_meeting`.

4. **Any state → `Idle`** on error: log the error, emit `recording://error` event with the context, reset state. Never leave the FSM in an invalid state.

5. **Race: `Ended` fires during `Transcribing`** — ignore. The recording was already stopped; the detect event is stale. Document this explicitly in comments.

6. **Race: `Started` fires during `Recording`** — also ignore. Two meetings can't be recording at once in v1 (this is a `1.1` feature — "multi-meeting queue").

### Wiring into commands.rs

- `recording_state()` → `orchestrator.recording_state()`
- `start_recording(manual)` → `orchestrator.start_recording(manual)`
- `stop_recording()` → `orchestrator.stop_recording()`
- Orchestrator is a Tauri `State<Orchestrator>` managed in `lib.rs::run()` at `tauri::Builder::default().manage(Orchestrator::new(...))`. Command handlers take `State<Orchestrator>`.
- `lib.rs::run()` also calls `orchestrator.start_auto_detect()` after the Tauri setup hook fires (so `AppHandle` is available).

### Verification gate

1. `cargo check` clean from `src-tauri/`
2. Unit tests for FSM transitions (mock `AudioCapture` + `WhisperEngine` via traits if easy, otherwise use function-level tests)
3. On WSL: manual test path — `invoke('start_recording', { manual: true })` → check RecordingState → `invoke('stop_recording')` → check a row lands in sqlite with segments (the transcription will be empty on WSL since cpal has no audio device, but the FSM should complete cleanly not crash)
4. On Windows: same manual path with a real microphone, verify actual audio captured and Whisper segments populate

### Risks

- **Risk F (orchestrator race conditions):** mitigated by the explicit transition rules above. Don't wave hands — the stale-event cases must be coded.
- **Whisper blocks the tokio runtime:** must use `spawn_blocking`. If ever called from an async context without `spawn_blocking`, it will stall the app.
- **cpal failure on WSL:** `AudioCapture::new()` will fail cleanly on WSL because there's no audio device. The FSM must handle this as "cannot start recording, return to Idle with an error event" — not panic.
- **detect::start callback runs on a separate thread:** it needs to cross-thread-safely notify the orchestrator. Use an `Arc<Mutex<OrchestratorState>>` or a `tokio::sync::mpsc` channel from the callback into an async task. Pick one pattern and stick to it.

---

## Step 2 — Backend Phase 3 polish (prefs + stub fills + dialog)

**Owner:** lead sequential
**Environment:** WSL-safe for codegen; autostart + dialog + audio enumeration real paths need Windows
**Files touched:**
- `src-tauri/src/prefs.rs` (new)
- `src-tauri/src/lib.rs` (register mod, register tauri-plugin-autostart, register tauri-plugin-dialog)
- `src-tauri/src/commands.rs` (add prefs commands, fill autostart/audio/download_model stubs, add choose_folder command)
- `src-tauri/Cargo.toml` (add tauri-plugin-autostart, tauri-plugin-dialog)

### `prefs` module

A small JSON-backed key-value store at `<app_data_dir>/settings.json`. Use Tauri's `app.path().app_data_dir()` for cross-platform correctness even though v1 is Windows-only (future-proofing for v2).

```rust
pub fn init(app_data_dir: PathBuf) -> Result<()>;
pub fn get(key: &str) -> Result<Option<serde_json::Value>>;
pub fn set(key: &str, value: serde_json::Value) -> Result<()>;
pub fn list() -> Result<HashMap<String, serde_json::Value>>;
```

File is atomically rewritten on each `set` (write-to-temp + rename). No journaling, no locking beyond a process-level Mutex. Good enough for ~10 keys.

### Tauri commands added

```
get_preference(key: String) -> Option<JsonValue>
set_preference(key: String, value: JsonValue) -> ()
list_preferences() -> HashMap<String, JsonValue>
choose_folder(title: Option<String>) -> Option<String>   // tauri-plugin-dialog wrapper
```

### Phase 3 polish stubs to fill

- **`get_autostart` / `set_autostart`** → wire to `tauri-plugin-autostart`. On Windows this writes to `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Plugin handles it. Register the plugin in `lib.rs::run()`.
- **`list_audio_input_devices`** → enumerate via `cpal::default_host().input_devices()`, collect `{name, is_default}`. The existing `audio.rs` module already uses cpal so the dep is there.
- **`download_model`** → delegate to the existing `models::resolve` (which already downloads if missing). Wrap it to emit `models://download_progress` events via `AppHandle::emit`. Deduplicate concurrent calls with a `Mutex<HashSet<ModelName>>` to prevent two UI clicks from downloading the same model twice.

### Verification gate

1. `cargo check` clean
2. Call each new command from Tauri devtools and confirm the expected return shape
3. On WSL, `set_autostart(true)` can silently no-op or return an explanatory error (plugin is platform-gated) — document the behavior but don't block
4. On Windows, `set_autostart(true)` creates the Run key; `set_autostart(false)` removes it; re-reading `get_autostart` returns the new value
5. Restart the app after setting prefs; confirm persistence

### Risks

- **`app_data_dir` not writable** on first run: the directory must be created before first write. `prefs::init` is responsible.
- **Concurrent `set` calls** from different Tauri command handlers: the process-level Mutex in prefs handles this.
- **tauri-plugin-autostart version mismatch** with Tauri 2.x: pin to a known-good version, don't use a floating minor.
- **tauri-plugin-dialog on Windows:** should just work but the first "Choose folder" click is a Windows native COM dialog — document any quirks if they appear.

---

## Step 3 — api.ts wrappers for Step 2 commands

**Owner:** lead sequential
**Environment:** WSL-safe
**Files touched:**
- `ui/src/api.ts` (add wrappers for the new commands)

### Wrappers to add

```typescript
// Preferences (low-level)
getPreference: <T = unknown>(key: string) => Promise<T | null>
setPreference: (key: string, value: unknown) => Promise<void>
listPreferences: () => Promise<Record<string, unknown>>

// Folder picker
chooseFolder: (title?: string) => Promise<string | null>
```

The frontend-polish teammate in Step 4′ will use these to swap the 7 localStorage-backed settings, keyed with the SAME `noru:settings.<section>.<key>` names the frontend already uses. The key namespace maps 1:1 from localStorage to the prefs store — nothing to migrate at first launch, just read from the new source.

### Verification gate

1. `tsc --noEmit` clean from `ui/`
2. `vite build` clean
3. Manually invoke each wrapper from the devtools console to confirm the shapes match the Rust side

### Risks

- **Type drift between `prefs::get` returning `serde_json::Value` and TS `unknown`:** the caller in Step 4′ is responsible for runtime validation (same pattern frontend already uses for localStorage reads). Document this in the wrapper doc comment.

---

## Step 4 — Tray icon polish (lead, Rust)

**Owner:** lead sequential
**Environment:** Windows-only for full verification (tray plugin's runtime behavior differs on Linux/macOS/WSL)
**Files touched:**
- `src-tauri/src/main.rs` or `src-tauri/src/lib.rs` (wherever tray is set up)
- `src-tauri/icons/` (add `tray-recording.ico` if using icon swap path)

### Investigation

Before coding, verify what `tauri-plugin-tray` actually supports for runtime icon changes. Check `TrayIconBuilder::set_icon` / `TrayIcon::set_icon` availability in the version pinned in `Cargo.toml`. **This is a 10-minute doc read; do it first before committing to a design.**

- If runtime icon swap is supported → two icons (`tray-idle.ico`, `tray-recording.ico`), swap on orchestrator state transitions
- If not supported → fall back to tray **tooltip** change ("noru (recording...)" vs "noru"), which is definitely supported

### Verification gate

1. On Windows: launch app, observe tray icon/tooltip is idle-state
2. Trigger `start_recording(manual=true)` via devtools
3. Tray icon/tooltip reflects recording state
4. Trigger `stop_recording()`; tray returns to idle
5. No crashes when toggling rapidly

### Risks

- **Tray plugin API surface:** don't assume anything — read the actual docs. Fallback to tooltip is always safe.

---

## Step 4′ — Frontend polish (teammate, parallel to Step 4)

**Owner:** `frontend-polish` teammate, opus (same work quality as phase 2 frontend teammate)
**Spawn condition:** After Step 3 is pushed to master
**Environment:** WSL-safe
**File ownership (strict):**
- `ui/src/views/TranscriptList.tsx`
- `ui/src/views/TranscriptViewer.tsx`
- `ui/src/views/Settings.tsx`
- `ui/src/views/settings/General.tsx`
- `ui/src/views/settings/Recording.tsx`
- `ui/src/views/settings/Whisper.tsx`
- `ui/src/views/settings/AIFeatures.tsx`
- `ui/src/views/settings/widgets.tsx`
- `ui/src/components/AIPanel.tsx`

**Forbidden:** `ui/src/App.tsx`, `ui/src/api.ts`, `ui/src/main.tsx`, any Rust file, any config/lockfile.

### Tasks

1. **Swap localStorage → api.\***. Every `// TODO(phase-3)` marker in the owned files gets swapped to the corresponding `api.getPreference` / `api.setPreference` wrapper. Use the SAME namespaced keys (`noru:settings.general.theme`, etc.) so the swap is mechanical. Keep the same runtime validation (fall back to default if the stored value is garbage).
2. **Error states.** When a Tauri command rejects, show an inline error banner with the message and a "retry" button. Applies to:
   - `TranscriptList` (list_meetings failed)
   - `TranscriptViewer` (get_meeting failed)
   - `AIPanel` (ai_summarize / ai_extract_* failed — especially the "not signed in" case)
   - `Settings → Recording → list_audio_input_devices` (enumerate failed)
   - `Settings → Whisper → download_model` (download failed mid-stream)
3. **Empty states.**
   - `TranscriptList` with zero rows → "No meetings recorded yet. Start one from the tray menu, or enable auto-detect in Settings."
   - `AIPanel` with `auth_status().state === 'signed_out'` → "Sign in to ChatGPT in Settings → AI Features (experimental) to summarize this transcript."
   - `Settings → Whisper` with no model downloaded → "Download a Whisper model to enable transcription."
4. **Keyboard shortcuts.**
   - `Esc` in Settings → close settings dialog/back to transcript view
   - `Ctrl+,` (or `Cmd+,` on future macOS) → open Settings
   - `/` in TranscriptList → focus search (if search is present; otherwise skip)
   - Enter in tray menu items is handled by the OS; no frontend work needed
5. **Coordinate via SendMessage** with the lead if any wrapper is missing, if any Phase 1 signature looks wrong, or if the `localStorage` key namespace doesn't exactly match what Step 3 wired on the Rust side.

### Verification gate

1. `tsc --noEmit` clean
2. `vite build` clean
3. Manual check in the dev server: toggle a setting, reload the app, setting persists (proves it's going to prefs not localStorage)
4. Manual check: sign out, open AIPanel, see the signed-out empty state
5. grep for `// TODO(phase-3)` in owned files → zero matches

### Risks

- **Keyboard shortcut collisions** with browser devtools: `Ctrl+,` is usually free; verify on Windows
- **Empty state wording** that sounds patronizing: keep it short and factual, no "oops" or "looks like"

---

## Step 5 — Integration verification

**Owner:** lead sequential
**Environment:** WSL for compile-level verification; Windows for a full behavioral smoke
**Files touched:** none (verification only)

### Checks

1. `cd src-tauri && cargo check` — clean
2. `cd src-tauri && cargo test --lib` — all Phase 2 tests still pass (15/15 from backend); any new orchestrator tests pass
3. `cd ui && npx tsc --noEmit` — clean
4. `cd ui && npx vite build` — clean
5. Launch `cargo tauri dev` from repo root (or the equivalent `cargo run` with Vite running) — the app opens, tray appears, settings can be opened, all 4 sections render
6. From devtools: `invoke('list_meetings', { limit: 100, offset: 0 })` returns `[]` on first run
7. From devtools: `invoke('auth_status')` returns `{ state: 'signed_out' }` on first run
8. From devtools: every command in `commands.rs` is callable and returns either a success shape or a documented error (no panics, no "not implemented")

### Verification gate

Every command in the registry works. No `unimplemented!()` anywhere in the Rust codebase (grep for it explicitly). No `// TODO(phase-3)` comments anywhere (grep in both `src-tauri/` and `ui/`).

---

## Step 6 — Windows E2E smoke test  ⚠️ WINDOWS ONLY

**Owner:** lead sequential
**Environment:** **REAL WINDOWS MACHINE — NOT WSL.** detect uses `EnumProcesses`/`EnumWindows`, WASAPI capture via cpal needs real audio devices, OAuth loopback needs a real loopback socket the browser can actually reach.

### If no Windows machine is immediately available

Plan A — wait for it. Don't fake this step.
Plan B — use a free-tier Windows VM (Azure / AWS / Parallels / VMware). The setup cost is ~1 hour; the smoke test runs in 15 min.
Plan C — fall back to GitHub Actions with a `workflow_dispatch` test job that runs the binary headlessly and verifies `--help` + maybe a canned `--cli` smoke path. This doesn't verify OAuth or the UI, but it catches catastrophic failures.

### The smoke test

1. Download the GHA artifact `noru-windows-x86_64` from the latest green build, OR build locally with `cargo tauri build --release`
2. Launch `noru.exe`. Tray icon appears. No crash window.
3. Open the main window from the tray. All 4 settings sections render. No placeholder text.
4. **Manual recording path:**
   - Open a Zoom/Teams/Meet meeting (real or fake — doesn't matter, just get a window title that matches the detector patterns)
   - Wait for detect to fire `Started` (~15s at the debounced cadence)
   - Verify tray shows recording state
   - Say a few words out loud (microphone)
   - End the meeting window
   - Wait for detect `Ended` (~15s debounce + transcribe time)
   - Verify a new transcript appears in the sidebar with real segments
   - Verify the transcript viewer shows what you said
5. **ChatGPT OAuth path (FIRST REAL TEST):**
   - Open Settings → AI Features (experimental)
   - Click "Sign in with ChatGPT"
   - Browser opens to auth.openai.com; log in with a real ChatGPT Plus account
   - Browser redirects to `localhost:<port>`, loopback receives code
   - `~/.noru/auth.json` is written
   - Settings shows `Signed in as: <email>`
6. **AI calls path:**
   - Go back to the transcript
   - Click "Summarize" → wait → summary appears
   - Click "Action items" → wait → bullet list appears
   - Click "Key decisions" → wait → bullet list appears
7. **Restart persistence:**
   - Close `noru.exe`
   - Reopen it
   - The transcript is still there
   - Auth is still signed in (no re-login required)
   - Settings prefs are still their last values

### Verification gate

Every bullet above passes. **If OAuth or the AI call shapes don't match what auth.rs/ai.rs assume**, this is where we find out — capture the raw HTTP request/response, compare against the EvanZhouDev/openai-oauth reference, iterate.

### Risks

- **Risk A (gpt-5 model rejected):** if the Codex backend returns "model not found" or similar, change `DEFAULT_MODEL` in `ai.rs` to whatever it accepts (try `gpt-4-turbo`, `gpt-4o`, or whatever the reference uses at the time of smoke test). Recompile, retry.
- **Risk B (OAuth flow shape mismatch):** the handoff notes from `auth-ai` explicitly flagged this as untested. Have the openai-oauth reference open in a tab. Log raw HTTP via `ureq` tracing. Diff requests against the reference.
- **Risk C (no Windows machine):** covered above by Plans A/B/C.
- **Firewall prompt** on first OAuth loopback bind: Windows will ask to allow the app on first run. Accept it once; subsequent runs don't re-prompt.
- **Codex backend returns HTTP 401 with a valid token:** the `chatgpt-account-id` header might be wrong. The parsing of `account_id` from the id_token JWT claim `https://api.openai.com/auth.chatgpt_account_id` is untested — if it's wrong, token exchange works but AI calls 401. Log the decoded claim and verify against the reference.

---

## Step 7 — Release v0.1.0

**Owner:** lead sequential
**Environment:** WSL + GitHub
**Files touched:**
- Possibly `.github/workflows/release.yml` (new, if the build.yml isn't release-aware)
- Tag `v0.1.0` on master
- GitHub Release body (markdown)

### Release sequence

1. **Verify gate state:** master is green in GHA, Step 5 integration checks pass, Step 6 smoke test passed
2. **Tag locally:** `git tag -a v0.1.0 -m "noru v0.1.0 — first release"`
3. **Push tag:** `git push origin v0.1.0`
4. **Trigger release build:** if `build.yml` has a release trigger, the tag push does it automatically. If not, either
   - Add a minimal `release.yml` that runs on tag push and uploads the Windows .exe to a GitHub Release via `softprops/action-gh-release`, OR
   - Run `gh release create v0.1.0 --generate-notes` and manually attach the artifact from the latest master build
5. **Smoke-test the downloaded release binary** on Windows one more time — download from the release page, unzip, run. This is the user's experience; do it once before telling anyone the release exists.
6. **Release notes** (short, factual):
   - Supported: Windows 10/11 x86_64
   - Features: auto-detect meetings (Zoom/Teams/Meet/Slack/Discord/Webex), local Whisper transcription, sqlite transcript browser, optional ChatGPT OAuth summaries
   - Known limits: Windows-only (macOS v2, Linux v3), ChatGPT OAuth only for AI features, no auto-updater yet (v1.1)

### Verification gate

A GitHub user can click "Download noru.exe" from the release page, run it on a clean Windows machine, and complete Step 6's flow without any extra setup.

### Risks

- **Release build uses debug profile by accident:** the workflow must build `--release`; the current `build.yml` already does this
- **Unsigned binary triggers Windows SmartScreen warnings:** expected for v1. Document in the release notes that users will see a SmartScreen prompt and how to proceed. Code signing is a v1.1+ concern.
- **Tag pushes don't trigger the workflow** because `build.yml` only triggers on branch push: verify this before tagging. If needed, add `on: push: tags: [v*]`.

---

## Non-goals — things that LOOK like Phase 3 but aren't

From `ROADMAP.md`, these are explicitly deferred and must NOT be pulled into Phase 3:

- **Auto-updater** — v1.1
- **AI Q&A chat box** in the transcript viewer — v1.1
- **Export formats** (markdown, plain text, JSON) — v1.1
- **Image classifier** for meeting confirmation (reduces detect false positives) — v1.1
- **CUDA Whisper build** opt-in — v1.1
- **Multilingual transcription polish** / mixed-language handling — v1.1
- **MCP server mode** (`noru.exe --mcp`) — v1.5
- **Mailbox / scheduler** — v1.5
- **npm package shim** — v1.5
- **Voice activation / wake word / VAD / R2-D2 beeps** — v2
- **Intent LLM / agent registry / router** — v3
- **Computer control / cua integration / Anthropic Computer Use** — v4
- **Nexus integration** — v4

Also NOT in Phase 3 but sometimes mistaken for polish:
- Tray icon animation sequences (loading spinner in tray) — keep it to a simple state swap
- Dark mode theming polish — the Phase 2 UI supports `theme` pref but visual polish is v1.1
- In-app "about" dialog with version and license — v1.1
- First-run welcome screen — **explicitly forbidden** by CLAUDE.md hard rule #2

If during Phase 3 execution any of these seem tempting, the answer is **no, it goes to ROADMAP**. The v1 scope is locked.

---

## Risk catalog (reference)

| ID | Risk | Severity | Mitigation step |
|---|---|---|---|
| A | `gpt-5` model rejected by Codex backend | Medium | Try alt model names in Step 6; code is one constant change |
| B | OAuth shape differs from openai-oauth reference | Medium-High | Log raw HTTP in Step 6, iterate |
| C | No Windows machine available | Low (we have one) | Plans A/B/C in Step 6 |
| D | Tray icon runtime swap unsupported | Low | Tooltip fallback in Step 4 |
| E | GHA Windows build has never run against Tauri | Resolved | Step 0 build-watcher |
| F | Orchestrator race conditions | Medium | Explicit transition rules in Step 1 |
| G | `app_data_dir` not writable on first run | Low | `prefs::init` creates it |
| H | Whisper blocks tokio runtime | Medium | `spawn_blocking` in Step 1 |
| I | cpal fails on WSL (no audio device) | Low | FSM handles cleanly; Step 5 validation |
| J | tauri-plugin-autostart / dialog / tray version drift | Low | Pin exact minor versions |
| K | Unsigned .exe SmartScreen warnings | Expected | Document in Step 7 release notes |

---

## Team coordination — how Step 4′ teammate is spawned

After Step 3 is pushed, the lead spawns:

```
Agent({
  description: "Spawn frontend-polish teammate",
  subagent_type: "general-purpose",
  team_name: "noru-phase-3",
  name: "frontend-polish",
  model: "opus",          // quality > cost for user-facing polish
  run_in_background: true,
  prompt: "<brief referencing this PHASE3_PLAN.md Step 4′>"
})
```

The teammate reads `PHASE3_PLAN.md § Step 4′` directly instead of duplicating the brief inline — saves prompt tokens and keeps the single source of truth in the plan. Its file ownership list, task list, and verification gate are all in that section.

---

## Execution order — quick reference

1. **Step 0** — Pre-flight: push (done) + build-watcher (running) + gate on green Windows build
2. **Step 1** — Orchestrator module + FSM + commands.rs wiring
3. **Step 2** — prefs module + polish stub fills + tauri-plugin-dialog
4. **Step 3** — api.ts wrappers for Step 2 commands
5. **Step 4** (parallel **Step 4′**) — Tray polish (lead) ‖ Frontend polish (teammate)
6. **Step 5** — Integration verification (WSL + Windows)
7. **Step 6** — Windows E2E smoke test ⚠️ Real Windows
8. **Step 7** — Release v0.1.0

Nothing in Phase 3 parallelizes further without causing merge conflicts on lead-owned files. This is the correct granularity for the hybrid model.
