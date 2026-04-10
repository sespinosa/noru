# noru — Roadmap

This document captures the **expanded vision** for noru beyond the v1 standalone app ([PLAN.md](./PLAN.md)). Each phase is shippable on its own and builds on the previous one — we never bet the whole vision on a single release.

The summary: **noru becomes the user's senses *and* voice for an AI agent collective. It listens, it observes, it dispatches, it responds — without ever sending audio off the user's machine.**

---

## v1 — Standalone meeting recording app *(current scope)*

> *We can look at what you're doing and help you proactively when needed.* The first proof: auto-record meetings, transcribe locally, summarize via ChatGPT.

The whole spec lives in [PLAN.md](./PLAN.md). One-line summary: a Windows tray app that auto-detects meetings, records audio, transcribes locally with Whisper, browses the transcripts in a simple UI, and offers three AI features (summarize / action items / key decisions) via an opt-in "Sign in with ChatGPT" flow in Settings labeled experimental.

**v1 alone is a useful product.** A normie can install it, never touch Settings, and get value (auto-recording + local transcripts). Everything below is additive.

---

## v1.1 — Quality and convenience

Small improvements after v1 ships, no scope expansion:

- **Auto-updater** — check GitHub releases, prompt user, download and replace
- **Image classifier for meeting confirmation** — small ONNX model that distinguishes "Zoom is open" from "Zoom is in an active call" — reduces false positives in detection
- **CUDA-enabled Whisper build** — opt-in faster build for users with NVIDIA GPUs
- **AI Q&A** — a small chat box in the transcript viewer to ask free-form questions about the transcript (still ChatGPT OAuth only)
- **Multilingual transcription polish** — language detection, mixed-language handling
- **Export** — markdown, plain text, JSON

---

## v1.5 — Make noru speak MCP

> Now any AI agent can read your transcripts and use noru's data programmatically.

This is where the *technical user* persona starts getting first-class value. The standalone app from v1 stays exactly as it is — v1.5 is purely additive.

### What's new

- **MCP server mode.** `noru.exe --mcp` runs the same binary as a stdio JSON-RPC MCP server. Reads from the same sqlite store the UI uses. No new state, no new daemon — the MCP face is just another way to expose what's already there.

- **MCP tools and resources:**
  ```
  Resources:
    noru://meetings/current             current meeting state
    noru://meetings/<id>                a specific meeting
    noru://transcripts/<id>             a specific transcript
    noru://transcripts/recent           N most recent transcripts

  Tools:
    noru.list_meetings(filters)
    noru.get_transcript(meeting_id)
    noru.search_transcripts(query)
    noru.start_recording()
    noru.stop_recording()
    noru.get_meeting_state()
  ```

- **The mailbox primitive.** Agent-to-agent (and noru-to-agent) message bus on top of sqlite. This is the spine that lets noru become reactive without needing the LLM client to support push.

  ```
  noru.inbox.check()                  → unread messages
  noru.inbox.send(to, body, [meta])   → put a message in someone else's inbox
  noru.inbox.subscribe(topics)        → register interest in event types
  ```

  A message has `(id, from, to, ts, topic, body, priority, ack_required)`. Multiple agents can read each other's messages and react. Noru is the post office.

- **The time dimension: scheduler.** The mailbox supports scheduled and recurring delivery — turning noru from a passive sensor into a **proactive scheduler**.

  ```
  noru.schedule(message, when)              → deliver a message at a future time
  noru.schedule.recurring(message, cron)    → recurring delivery
  noru.schedule.list([owner])               → see what's scheduled
  noru.schedule.cancel(id)                  → cancel a scheduled delivery
  ```

  Use cases: reminders, recurring routines ("every weekday at 5pm summarize today's meetings"), watchers ("check meeting state every 5 min and tell me when it ends"), trigger chains. The agent never has to maintain its own scheduler — it delegates time entirely to noru.

- **Three interfaces, one message store.** The mailbox is exposed via three transports, all reading from the same sqlite tables:
  1. **MCP server (pull)** — `--mcp`, the default agent-facing interface
  2. **SSE event stream (push)** — for harnesses (like [Nexus](#)) that subscribe to events as they happen
  3. **Hook helper (Claude Code specific)** — `--hook-context` one-shot, installed as a `UserPromptSubmit` hook to inject "you have N unread messages" into every prompt

- **npm package** — thin Node shim (~50 LOC) that downloads `noru.exe`, spawns it with `--mcp`, normalizes stdio across the WSL/Windows boundary, forwards signals. The whole purpose of the shim is to be the launcher for the MCP server. Universal install snippet:
  ```json
  { "mcpServers": { "noru": { "command": "npx", "args": ["-y", "noru"] } } }
  ```

- **`noru.exe --setup` / `noru_setup` MCP tool** — automated MCP install into Claude Code via `claude mcp add`, copy-paste snippets via `--print-config <client>` for everything else. No JSON config rewriting.

### What's NOT in v1.5

- Voice activation
- Agent dispatch
- Anything from v2+

---

## v2 — Voice activation

> noru gains ears that *listen for you*, not just for meetings. Beep-beep.

The first move beyond pure perception. noru becomes voice-aware: it can be activated by voice and respond with non-intrusive audio cues.

### What's new

- **Wake word detection.** Always-on, runs continuously on CPU.
  - Stack: [openWakeWord](https://github.com/dscripka/openWakeWord) (Apache 2.0) as the default; [Picovoice Porcupine](https://picovoice.ai/products/porcupine/) as an opt-in for higher reliability.
  - Custom wake word: **"noru"** (short, distinctive, two syllables, low collision risk).
  - Reality check: HA community reports DIY wake words are ~50% reliable in real-world noise. We accept this and document it; users who want production-grade reliability can opt into Porcupine.

- **VAD (voice activity detection).** [Silero VAD](https://github.com/snakers4/silero-vad). <1ms per 30ms chunk on CPU. Industry default.

- **Streaming low-latency STT.** Different from the meeting transcription pipeline (chunk-based for accuracy). Voice commands need partial hypotheses in <500ms.
  - Default: **NVIDIA Parakeet TDT** streaming (RTFx >2000, sub-second). What HA Voice and Meetily moved to in 2026.
  - Fallback: faster-whisper-tiny after VAD endpointing.

- **Attention state machine.** Sleep → Priming → High → Decaying → Sleep.
  - **Sleep:** wake word listener only. No transcription, no LLM, no UI feedback. Default state.
  - **Priming:** wake word fired. Brief window (~1s) where noru waits for speech to start.
  - **High:** user is speaking commands. Streaming STT active.
  - **Decaying:** silence for N seconds. Tray icon dims.
  - **Manual dismiss by voice.** "stop", "nevermind", "done" — instant return to sleep.
  - Decay timeline configurable. Default: 10s of silence → decay, 30s total → sleep.

- **R2-D2 style audio feedback.** Non-intrusive, charming, cheap.
  - **No TTS** — too expensive (compute), too verbose, copies what every other voice assistant does. We do *beeps* instead.
  - **80s vaporwave / sci-fi computer aesthetic.** Short bleeps and bloops with personality but distinct from R2-D2 (no copyright concerns, our own sound design).
  - **Sound vocabulary:** "wake", "listening", "thinking", "ack", "nack", "done", "decay".
  - **Synthesized at runtime** — small Rust synth (FM, square waves, envelopes). ~200 LOC, no external dependencies, no bundled WAVs.
  - **Tray icon flashes in sync** with the beeps. Different colors for different states.

- **Voice commands populate the inbox.** Each transcribed command becomes a message in `noru.inbox` with topic `user.voice.command`. Agents can read it via the same MCP tools they already use from v1.5.

### Technical risks

1. **Wake word reliability** — biggest risk. Mitigation: dual-factor (wake word + short confirmation phrase), explicit Porcupine opt-in, document the limitation honestly.
2. **Privacy of meeting participants** when attention activates mid-meeting. Need explicit policy: meeting audio is captured per the meeting policy; command audio is processed by intent LLM; the two don't mix.
3. **Latency budget.** Wake → endpoint → STT → inbox must feel <1s end-to-end.

---

## v3 — Voice → agent dispatch

> Talk to noru. Noru spawns the right agent. Agent does the thing.

This is the **novel piece**. Nobody is shipping this publicly as of April 2026: always-listening daemon + dual-source capture + attention FSM + voice → multi-agent dispatch on a local machine.

### What's new

- **Intent LLM.** A small fast local LLM that parses voice commands into structured actions.
  - Default: **Qwen2.5-Coder-3B** or **Llama-3.2-3B** via [llama.cpp](https://github.com/ggerganov/llama.cpp) with JSON schema constrained decoding.
  - Why local 3B: <100ms TTFT on RTX 4070 for short prompts. Mercury 2 has high TTFT (~3.8s) — fast for long generation, slow to first token, wrong tier for intent classification.

- **Agent registry.** noru knows which agents are installed and how to invoke them.
  - **Discovery on Windows:** `where claude`, `where codex`, `where opencode`, etc.
  - **Discovery in WSL:** if WSL is installed (`wsl.exe -l`), check `wsl.exe which claude`, etc.
  - **Both? Configurable preference**, or ask the user the first time.
  - **Use the official CLIs in print mode:** `claude -p`, `codex -p`, etc. — uses the user's existing subscription, officially supported, no OAuth bridges, no API key juggling.

- **Router.** Maps parsed intents to agents.
  - Intent has a target (`code`, `email`, `web`, `file`, `meeting`) and an urgency (`now`, `queue`).
  - Router looks up which registered agent handles the target and invokes it.
  - **Context injection:** if the intent references the current meeting ("summarize what we just discussed", "draft an email about that"), the relevant transcript is automatically attached to the spawned agent's prompt. **This is the killer feature.**

- **Confirmation gates for destructive actions.** Some intents are safe (read, summarize, query) — execute immediately. Some have side effects (send email, delete file, push commit) — require confirmation in the inbox before executing. User can confirm by voice ("yes", "go") or by clicking the tray notification. **Misheard destructive commands are the #1 risk.**

### Example flows

**"noru, summarize what we just talked about and put it in my notes"**
1. Wake word → attention High
2. Streaming STT → command captured
3. Intent LLM → `{ target: "summarize", source: "current_meeting", destination: "obsidian-notes" }`
4. Router → spawn `claude -p` with the meeting transcript + obsidian-surface MCP
5. Agent runs, writes the summary, posts a "done" message to noru.inbox
6. R2-D2 "done" beep, tray icon green flash

**"noru, ask Claude to fix that bug we just discussed"**
1. Same flow, but intent target is `code`
2. Router → spawn full `claude` interactively with transcript context
3. R2-D2 "ack" beep when Claude starts; "done" beep when it finishes

**"noru, send Sebastián an email saying I'll be late"**
1. Intent has side effect → confirmation gate
2. R2-D2 "thinking" beep, tray shows pending action
3. User says "yes" → R2-D2 "ack" beep, agent runs
4. If user says "stop" or doesn't confirm in 30s → R2-D2 "decay" beep, action discarded

### Technical risks

1. **Latency.** End-to-end (wake → spawn → first agent token) must feel <2s for short commands. Tight budget.
2. **Misheard commands** — already noted; the confirmation gate is the answer.
3. **Agent invocation across the WSL boundary.** Spawning a WSL `claude` from a Windows process is doable (`wsl.exe -d Ubuntu claude -p "..."`) but the prompt escaping gets hairy.
4. **Context injection bloat.** Long meeting transcripts could blow out the agent's context window. Need a "relevant excerpt" extraction step before passing to the spawned agent.

---

## v4 — Full Jarvis

> noru extends from senses to action. The agent doesn't just talk back — it does things on your computer.

### What's new

- **Computer control via [trycua/cua](https://github.com/trycua/cua).** Open infrastructure for desktop control across Linux/Win/macOS, MCP-compatible. noru integrates cua so spawned agents can click, type, navigate windows.
- **Anthropic Computer Use** integration for agents that support it (Claude). The router decides whether the agent's intent needs GUI interaction and routes accordingly.
- **Bidirectional TTS feedback.** Optional. For long results ("here's a summary of the 30-minute meeting"), the user can opt into spoken output via [Piper](https://github.com/rhasspy/piper) or [Kokoro](https://huggingface.co/hexgrad/Kokoro-82M). Beeps remain the default; TTS is opt-in per command type.
- **Echo cancellation / barge-in.** WebRTC AEC or speexdsp so the user can interrupt noru while it's speaking, and so noru's own TTS doesn't get re-captured.
- **Multi-agent orchestration.** noru becomes a router for *agent collectives*, not just single agents. A single voice command might spawn multiple agents in parallel (research, coding, doc), coordinate them via the inbox, and synthesize results.
- **Nexus integration.** First-class support for [Nexus](#) (the user's harness for hierarchical agent orchestration). noru pushes events to Nexus over an authenticated tunnel; Nexus injects them into the right agent sessions.

### Technical risks

1. **Echo cancellation.** Non-trivial, especially with system audio loopback active.
2. **Computer use safety.** Misclicked actions are worse than misheard commands. Strong confirmation gates, undo where possible, operate in restricted scopes.
3. **Scope creep into "complete personal AI."** Mitigation: strict separation of *senses* (noru) from *cognition* (the spawned agents). noru never tries to be the smart one — it routes to the agents that are.

---

## What noru is NOT (and won't become)

- Not a meeting notes SaaS. Otter/Granola/Fireflies own that market and are welcome to it.
- Not a wearable. Bee/Friend/Limitless own that.
- Not a personal memory app. Rewind/Limitless own that.
- Not a closed product. Always open source, always MIT.
- Not cloud-based. Audio never leaves the machine. The only network calls noru itself makes are: model downloads (Hugging Face), and ChatGPT API calls *only when the user has explicitly opted in*.
- Not an LLM proxy / subscription bridge. We use the user's ChatGPT subscription via the same de-facto OAuth flow Cline ships, but it's one feature, not the product. Codex CLI's official flow can replace it any day without changing what noru is.
- Not a dependency of any specific agent or AI provider. v1.5+ works with Claude Code, Codex, OpenCode, Cursor, Continue, Cline, Zed, Windsurf, anything that speaks MCP.

## What noru *is*

- v1: a meeting recorder + local transcriber that just works on first launch, with optional ChatGPT-powered AI summaries.
- v1.5: + an MCP server, a mailbox, and a scheduler so any AI agent can read from and act on noru's data.
- v2: + voice activation with charming R2-D2 audio feedback.
- v3: + a router that turns voice commands into agent dispatches with the meeting context already attached.
- v4: + full computer control and multi-agent orchestration.

---

## A note on phase ordering

These phases are not time-boxed. Each one is "shippable when ready, used by the next." v1 has to land cleanly before v1.5 makes sense. v1.5 has to be reliable before v2 routing on top of it makes sense. **Don't skip ahead.**

The phases are also not all-or-nothing. v1 might evolve through v1.1, v1.2, v1.3 (more transcription quality, AI Q&A, polished UI) before v1.5 is even started. Each release tag is a checkpoint, not a deadline.

The point of this document is not to commit to a schedule. It's to make sure that when we add a new feature, we know where it sits in the bigger picture — and we don't accidentally rebuild something that should live in a different phase.
