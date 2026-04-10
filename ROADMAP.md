# noru — Roadmap

This document captures the **expanded vision** for noru beyond the v1 core ([PLAN.md](./PLAN.md)). It's organized in phases. Each phase is shippable on its own and builds on the previous one — so we never have to bet the whole vision on a single release.

The summary: **noru becomes the user's senses *and* voice for an AI agent collective. It listens, it observes, it dispatches, it responds — without ever sending a byte off the user's machine.**

---

## v1 — Foundation (current scope)

> Open-source, local-first meeting transcription that AI agents can read via MCP.

Already defined in [PLAN.md](./PLAN.md). The minimal useful product:

- Records meetings (mic + system audio)
- Transcribes locally with Whisper
- Detects meetings via process/window heuristics
- Exposes everything via MCP (pull) + SSE (push) + hook helper
- Mailbox primitive for agent-to-agent messaging
- Tauri tray app for control and visibility
- npm-first install, promotes to permanent install via setup tool

**v1 alone is already useful.** Everything below is additive.

---

## v1.5 — Voice activation

> noru gains ears that *listen for you*, not just for meetings. Beep-beep.

The first move beyond pure perception. noru becomes voice-aware: it can be activated by voice and respond with non-intrusive audio cues.

### What's new

- **Wake word detection.** Always-on, runs continuously on CPU.
  - Stack: [openWakeWord](https://github.com/dscripka/openWakeWord) (Apache 2.0) as the default; [Picovoice Porcupine](https://picovoice.ai/products/porcupine/) as an opt-in for higher reliability.
  - Custom wake word so noru isn't tied to "hey google" / "alexa." Default candidate: **"noru"** (short, distinctive, two syllables, low collision risk).
  - Reality check: HA community reports DIY wake words are ~50% reliable in real-world noise. We accept this and document it; users who want production-grade reliability can opt into Porcupine.

- **VAD (voice activity detection).** Cheap, gates everything downstream.
  - [Silero VAD](https://github.com/snakers4/silero-vad). <1ms per 30ms chunk on CPU. Industry default.

- **Streaming low-latency STT.** Different from the meeting transcription pipeline (which is chunk-based for accuracy). For voice commands, we need partial hypotheses in <500ms.
  - Default: **NVIDIA Parakeet TDT** streaming (RTFx >2000, sub-second). This is what HA Voice and Meetily moved to in 2026 for streaming workloads.
  - Fallback: faster-whisper-tiny after VAD endpointing (acceptable for short utterances).

- **Attention state machine.** Sleep → Priming → High → Decaying → Sleep.
  - **Sleep:** wake word listener only. No transcription, no LLM, no UI feedback. Default state.
  - **Priming:** wake word fired. Brief window (~1s) where noru waits for speech to start. If no speech, decay back to sleep.
  - **High:** user is speaking commands. Streaming STT active, intent LLM ready.
  - **Decaying:** silence for N seconds. Tray icon dims. After M seconds, back to sleep.
  - **Manual dismiss by voice.** "stop", "nevermind", "done" — instant return to sleep.
  - **Decay timeline configurable.** Default: 10s of silence → decay, 30s total → sleep.

- **R2-D2 style audio feedback.** Non-intrusive, charming, cheap.
  - **No TTS** — TTS is expensive (compute), distracting (verbose), and copies what every other voice assistant does. We do *beeps* instead.
  - **80s vaporwave / sci-fi computer aesthetic.** Short bleeps and bloops with personality but distinct from R2-D2 (no copyright concerns, our own sound design).
  - **Sound vocabulary:** "wake" (when entering Priming), "listening" (entering High), "thinking" (intent being parsed), "ack" (command accepted), "nack" (command rejected/misheard), "done" (task completed), "decay" (returning to Sleep).
  - **Synthesized at runtime,** not bundled WAVs — small Rust synth (FM, square waves, envelopes). ~200 LOC, no external dependencies.
  - **Tray icon flashes in sync** with the beeps. Visual + audio feedback together. Different colors for different states (green = listening, yellow = thinking, red = error).

- **Voice commands populate the inbox.** Each transcribed command becomes a message in `noru.inbox` with topic `user.voice.command`. Agents can read it via the same MCP tools they already use. No new agent integration needed for v1.5 — the existing inbox handles it.

### What's NOT in v1.5

- Voice → spawn coding agent (that's v2)
- TTS responses (we deliberately avoid it; beeps are the language)
- Multi-language support (English first, others later)

### Technical risks

1. **Wake word reliability** — biggest risk. Mitigation: dual-factor (wake word + short confirmation phrase like "noru, listen"), explicit Porcupine opt-in, document the limitation honestly.
2. **Privacy of meeting participants** when attention activates mid-meeting. Need explicit policy: meeting audio is captured per the meeting policy; command audio is processed by intent LLM; the two don't mix.
3. **Latency budget.** Wake → endpoint → STT → inbox must feel <1s end-to-end.

---

## v2 — Voice → agent dispatch

> Talk to noru. Noru spawns the right agent. Agent does the thing.

This is the **novel piece**. Nobody is shipping this publicly as of April 2026: always-listening daemon + dual-source capture + attention FSM + voice → multi-agent dispatch on a local machine.

### What's new

- **Intent LLM.** A small fast local LLM that parses voice commands into structured actions.
  - Default: **Qwen2.5-Coder-3B** or **Llama-3.2-3B** via [llama.cpp](https://github.com/ggerganov/llama.cpp) with JSON schema constrained decoding.
  - Why local 3B and not Mercury 2: Mercury 2 is incredible for *long* generations but has high TTFT (~3.8s). For sub-100ms intent classification, local Qwen 3B beats it. (Mercury 2 may show up in v2.5 for the *planning* step in multi-step intents.)
  - <100ms TTFT on RTX 4070 for short prompts.

- **Agent registry.** noru knows which agents are installed and how to invoke them.
  - **Discovery on Windows side:** check `where claude`, `where codex`, `where opencode`, etc.
  - **Discovery on WSL side:** if WSL is installed (`wsl.exe -l`), check `wsl.exe which claude`, etc.
  - **Both? Configurable preference**, or ask the user the first time.
  - **Example registry entry:**
    ```toml
    [agents.claude-code]
    name = "Claude Code"
    locations = [
      { platform = "windows", command = "C:\\Users\\dev\\AppData\\Roaming\\npm\\claude.cmd" },
      { platform = "wsl", command = "claude", distro = "Ubuntu" },
    ]
    invoke_pattern = "{command} -p {prompt}"
    capabilities = ["code", "files", "terminal"]
    ```

- **Router.** Maps parsed intents to agents.
  - Intent has a target (`code`, `email`, `web`, `file`, `meeting`) and an urgency (`now`, `queue`).
  - Router looks up which registered agent handles the target and invokes it with the intent body.
  - **Context injection:** if the intent references the current meeting ("summarize what we just discussed", "draft an email about that"), the relevant transcript is automatically attached to the spawned agent's prompt. **This is the killer feature.** It only works because noru already has the meeting context.

- **Confirmation gates for destructive actions.**
  - Some intents are safe (read, summarize, query) — execute immediately.
  - Some intents have side effects (send email, delete file, push commit) — require confirmation. The agent gets the intent but waits for an "acknowledged" message in its inbox before executing.
  - User can confirm by voice ("yes", "go", "do it") or by clicking the tray notification.
  - **Misheard destructive commands are the #1 risk** — this gate is non-negotiable.

### Example flows

**"noru, summarize what we just talked about and put it in my notes"**
1. Wake word → attention High
2. Streaming STT → command captured
3. Intent LLM → `{ target: "summarize", source: "current_meeting", destination: "obsidian-notes" }`
4. Router → spawn Claude Code with the meeting transcript + obsidian-surface MCP
5. Agent runs, writes the summary, posts a "done" message to noru.inbox
6. R2-D2 "done" beep, tray icon green flash

**"noru, ask Claude to fix that bug we just discussed"**
1. Same flow, but intent target is `code`
2. Router → spawn Claude Code with transcript context
3. Claude Code runs in your terminal (or in Nexus, if you're on the dev VPS)
4. R2-D2 "ack" beep when Claude starts; "done" beep when it finishes

**"noru, send Sebastián an email saying I'll be late"**
1. Intent has side effect (`send_email`) → goes through confirmation gate
2. R2-D2 "thinking" beep, tray shows pending action
3. User says "yes" → R2-D2 "ack" beep, agent runs
4. If user says "stop" or doesn't confirm in 30s → R2-D2 "decay" beep, action discarded

### What's NOT in v2

- Computer control (clicking, typing into windows) — that's v3
- Bidirectional TTS feedback (still beeps only) — beeps stay; v3 might add optional TTS for long results
- Multi-step plans / agent collaboration — single-agent dispatch in v2

### Technical risks

1. **Latency.** End-to-end (wake → spawn → first agent token) must feel <2s for short commands. Tight budget.
2. **Misheard commands** — already noted; the confirmation gate is the answer.
3. **Agent invocation across the WSL boundary.** Spawning a WSL `claude` command from a Windows process is doable (`wsl.exe -d Ubuntu claude -p "..."`) but the prompt escaping gets hairy. Need careful argument handling.
4. **Context injection bloat.** Long meeting transcripts could blow out the agent's context window. Need a "relevant excerpt" extraction step (probably via the intent LLM with a different prompt) before passing to the spawned agent.

---

## v3 — Full Jarvis

> noru extends from senses to action. The agent doesn't just talk back — it does things on your computer.

### What's new

- **Computer control via [trycua/cua](https://github.com/trycua/cua).** Open infrastructure for desktop control across Linux/Win/macOS, MCP-compatible. noru integrates cua so spawned agents can click, type, navigate windows.
- **Anthropic Computer Use** integration for agents that support it (Claude). The router decides whether the agent's intent needs GUI interaction and routes accordingly.
- **Bidirectional TTS feedback.** Optional. For long results ("here's a summary of the 30-minute meeting"), the user can opt into spoken output via [Piper](https://github.com/rhasspy/piper) or [Kokoro](https://huggingface.co/hexgrad/Kokoro-82M). Beeps remain the default; TTS is opt-in per command type.
- **Echo cancellation / barge-in.** WebRTC AEC or speexdsp so the user can interrupt noru while it's speaking, and so noru's own TTS doesn't get re-captured by its always-on microphone.
- **Multi-agent orchestration.** noru becomes a router for *agent collectives*, not just single agents. A single voice command might spawn multiple agents in parallel (research agent, coding agent, doc agent), coordinate them via the inbox, and synthesize results.
- **Nexus integration.** First-class support for [Nexus](#) (the user's harness for hierarchical agent orchestration). noru pushes events to Nexus over an authenticated tunnel; Nexus injects them into the right agent sessions. The combined system gives the agent collective in Nexus full physical-world awareness via noru's senses, and noru gains access to Nexus's hierarchical agent spawning.

### What's NOT in v3

- Wearable hardware (no Bee/Friend competition)
- Cloud sync of any kind (still local-first, even when integrating with Nexus over a tunnel)
- "Memory" in the lifelogging sense (no Limitless / Rewind clone) — noru is meant for *context for AI agents*, not human memory augmentation. Adjacent but different.

### Technical risks

1. **Echo cancellation.** Non-trivial, especially with system audio loopback active. Mitigation: optional, can disable mic capture during TTS playback.
2. **Computer use safety.** Misclicked actions are worse than misheard commands. Strong confirmation gates, undo where possible, operate in restricted scopes.
3. **Scope creep into "complete personal AI."** The risk is that noru tries to be everything and ends up being mediocre at all of it. Mitigation: strict separation of *senses* (noru) from *cognition* (the spawned agents). noru never tries to be the smart one — it routes to the agents that are.

---

## What noru is NOT (and won't become)

- Not a meeting notes SaaS. Otter/Granola/Fireflies own that market and are welcome to it.
- Not a wearable. Bee/Friend/Limitless own that.
- Not a personal memory app. Rewind/Limitless own that.
- Not a closed product. Always open source, always MIT.
- Not cloud-based. Audio never leaves the machine. The only network calls noru itself makes are model downloads (from Hugging Face).
- Not a dependency of any specific agent or AI provider. Works with Claude Code, Codex, OpenCode, Cursor, Continue, Cline, Zed, Windsurf, anything that speaks MCP.

## What noru *is*

- The senses for AI agents on your computer. Audio, screen, events, voice commands.
- A message bus that any agent can read and write to.
- A router that turns voice into agent actions, with the meeting context already attached.
- A daemon that runs quietly, beeps charmingly, and keeps your data local.

---

## A note on phase ordering and time

These phases are not time-boxed. Each one is "shippable when ready, used by the next." v1 has to land cleanly before v1.5 makes sense. v1.5 has to be reliable before v2 routing on top of it makes sense. Don't skip ahead.

The phases are also not all-or-nothing. v1 might evolve through v1.1, v1.2, v1.3 (more transcription quality, better detection, polished UI) before v1.5 is even started. Each release tag is a checkpoint, not a deadline.

The point of this document is not to commit to a schedule. It's to make sure that when we add a new feature, we know where it sits in the bigger picture — and we don't accidentally rebuild something that should live in a different phase.
