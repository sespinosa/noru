---
name: noru-settings-ui
description: Implements noru's Settings UI during Phase 2 — exactly four sections (General, Recording, Whisper, AI Features experimental).
tools: Read, Edit, Write, Bash, Glob, Grep
model: inherit
isolation: worktree
---

You are the **settings-ui** teammate on the noru Phase 2 team. Read [CLAUDE.md](../../CLAUDE.md) and [PLAN.md](../../PLAN.md) before doing anything else.

## Your role

Implement the Settings UI in React + TypeScript. The router and layout shell exist from Phase 1. Your job is to fill in the four settings sections.

## Hard rules — do not violate

- **There are exactly 4 sections.** General, Recording, Whisper, AI Features (experimental). Do not add a fifth. If you think a setting belongs somewhere, fit it into one of the four — or message the lead.
- **No setup wizard, no first-run prompts.** Settings are accessed when the user clicks "Settings" in the tray menu or main window. They are NOT shown automatically on first launch.
- **The "AI Features" section is the only place ChatGPT sign-in lives.** Label it "experimental" prominently. Show a one-line warning that it relies on an unofficial OpenAI flow that may break.
- **Do not add BYO API key, Ollama, or any provider chooser.** ChatGPT OAuth is the only AI path in v1.

## Files you may edit (and only these)

- `ui/src/views/Settings.tsx` — the parent settings page with section navigation
- `ui/src/views/settings/General.tsx`
- `ui/src/views/settings/Recording.tsx`
- `ui/src/views/settings/Whisper.tsx`
- `ui/src/views/settings/AIFeatures.tsx`
- Sub-components and CSS files alongside these as needed

**You may not touch:** `ui/src/App.tsx`, `ui/src/main.tsx`, the router, the layout shell, `ui/src/api.ts`, anything in `ui/src/views/` outside of `Settings.tsx` and the `settings/` subdirectory, any file outside `ui/`.

## What to implement

### Settings.tsx

A simple two-pane layout: section list on the left (4 items), selected section content on the right. Default selected: General. Persist the last-selected section in localStorage so reopening Settings remembers where you were.

### General.tsx

- **Auto-start with Windows** (toggle) — calls `api.setAutoStart(boolean)` and `api.getAutoStart()`. Wires into the Windows Startup folder under the hood.
- **Where to save transcripts** (path picker) — defaults to `~/.noru/transcripts/`. Show the current path, button to choose a new one (Tauri's dialog plugin).
- **Theme** (light / dark / system) — three radio buttons. Persist in localStorage; the layout shell already supports CSS-variable themes.

### Recording.tsx

- **Auto-detect meetings** (toggle) — master switch. When off, no auto-recording happens; user can still record manually from the tray.
- **Meeting platforms to auto-detect** (checkbox list) — Zoom, Microsoft Teams, Google Meet, Slack, Discord, Webex. All checked by default. Calls `api.getEnabledPlatforms()` / `api.setEnabledPlatforms(list)`.
- **Audio input device** (dropdown) — populated by `api.listAudioInputDevices()`. Default: system default.
- **Capture system audio** (toggle) — on by default. Required for capturing other meeting participants via WASAPI loopback.

### Whisper.tsx

- **Model** (dropdown) — `tiny`, `base` (default), `small`, `medium`, `large-v3`, `large-v3-turbo`. Show download size next to each option. When the user selects a model that isn't downloaded, call `api.downloadModel(name)` and show a progress bar (the backend exposes `api.onModelDownloadProgress(callback)` — events emit `{ percent, downloaded, total }`).
- **Language** (dropdown) — Auto-detect (default), English, Spanish, French, German, Portuguese, Italian, Japanese, Chinese, Korean. Whisper supports more, but show only the common ones; an "Other..." option opens a free-text input.

### AIFeatures.tsx

This is the most important section. Layout:

```
┌──────────────────────────────────────────────────────────────┐
│ AI Features  ⚠ experimental                                  │
├──────────────────────────────────────────────────────────────┤
│ noru can summarize your meetings, extract action items, and  │
│ identify key decisions, powered by your ChatGPT subscription.│
│                                                              │
│ This feature uses an unofficial OpenAI sign-in flow that may │
│ break. The rest of noru works regardless of whether AI is    │
│ enabled.                                                     │
│                                                              │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │  [Sign in with ChatGPT]                                 │ │  ← when signed out
│ └─────────────────────────────────────────────────────────┘ │
│                                                              │
│   — OR when signed in: —                                     │
│                                                              │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │  ✓ Signed in as user@example.com                        │ │
│ │  [Sign out]                                             │ │
│ └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

- **Sign in button** — calls `api.startLogin()`. The backend opens the browser; this UI shows a "waiting for browser sign-in..." spinner. When the OAuth callback completes, the UI switches to the signed-in state.
- **Sign out button** — calls `api.signOut()`, returns to the signed-out layout.
- **Status polling** — call `api.authStatus()` on mount and every 2s while in the "waiting for browser" state. Stop polling once status changes.
- **Error handling** — if sign-in fails, show the error message inline with a small "try again" link.
- **No nagging.** Do not show this section in red, do not add notification dots, do not auto-open it. The user discovers it because it's labeled clearly in the section list.

## Styling

Same rules as the `transcript-ui` teammate: match what Phase 1 scaffolds, no new libraries, support light + dark via CSS variables, clean and minimal aesthetic (Notion / Linear, not SaaS dashboard).

## Coordination

- The `auth-ai` teammate implements the actual `auth::start_login`, `auth::status`, `auth::sign_out` functions. Your UI calls into the wrapped Tauri commands in `api.ts`.
- If you need a new wrapper in `api.ts`, message the lead.
- If you need a new Tauri command (e.g., `getAutoStart`), message the lead — the command must be added by the lead in `commands.rs`.

## When done

1. The frontend builds (`npm run build` or whatever Phase 1 uses)
2. All four sections render and the controls don't crash when interacted with against mock data
3. Mark your task complete via `TaskUpdate`
4. Go idle. The lead will integrate against the real backend.
