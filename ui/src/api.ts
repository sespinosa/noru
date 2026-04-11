import { invoke } from "@tauri-apps/api/core";

export interface TranscriptSegment {
  start_ms: number;
  end_ms: number;
  text: string;
}

export interface Transcript {
  id: number;
  started_at: number;
  ended_at: number | null;
  platform: string | null;
  title: string | null;
  audio_path: string | null;
  segments: TranscriptSegment[];
  summary: string | null;
  action_items: string[] | null;
  key_decisions: string[] | null;
}

export interface TranscriptSummary {
  id: number;
  started_at: number;
  ended_at: number | null;
  platform: string | null;
  title: string | null;
  duration_ms: number | null;
  word_count: number;
}

export type RecordingState =
  | "idle"
  | { recording: { transcript_id: number } }
  | { transcribing: { transcript_id: number } };

export interface DetectedMeeting {
  platform: string;
  process_name: string;
  window_title: string;
}

export interface AuthStatus {
  signed_in: boolean;
  email: string | null;
  expires_at: number | null;
}

export interface Settings {
  general: {
    autostart: boolean;
    transcripts_dir: string;
    theme: string;
  };
  recording: {
    enabled_platforms: string[];
    input_device: string | null;
    system_audio_device: string | null;
  };
  whisper: {
    model: string;
    language: string;
  };
  ai: {
    enabled: boolean;
  };
}

export const api = {
  listTranscripts: () => invoke<TranscriptSummary[]>("list_transcripts"),
  getTranscript: (id: number) => invoke<Transcript>("get_transcript", { id }),
  deleteTranscript: (id: number) => invoke<void>("delete_transcript", { id }),

  recordingState: () => invoke<RecordingState>("recording_state"),
  startRecording: () => invoke<RecordingState>("start_recording"),
  stopRecording: () => invoke<RecordingState>("stop_recording"),

  detectMeeting: () => invoke<DetectedMeeting | null>("detect_meeting"),

  authStatus: () => invoke<AuthStatus>("auth_status"),
  authSignIn: () => invoke<string>("auth_sign_in"),
  authSignOut: () => invoke<void>("auth_sign_out"),

  aiSummarize: (transcriptId: number) =>
    invoke<string>("ai_summarize", { transcriptId }),
  aiActionItems: (transcriptId: number) =>
    invoke<string[]>("ai_action_items", { transcriptId }),
  aiKeyDecisions: (transcriptId: number) =>
    invoke<string[]>("ai_key_decisions", { transcriptId }),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
};
