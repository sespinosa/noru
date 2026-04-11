import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type MeetingId = string;

export type Platform =
  | "zoom"
  | "meet"
  | "teams"
  | "slack"
  | "discord"
  | "webex"
  | "manual";

export interface TranscriptSegment {
  start_ms: number;
  end_ms: number;
  text: string;
}

export interface Meeting {
  id: MeetingId;
  started_at: string;
  ended_at: string | null;
  platform: Platform | null;
  audio_path: string | null;
  segments: TranscriptSegment[];
  summary: string | null;
  action_items: string[] | null;
  key_decisions: string[] | null;
  created_at: string;
}

export interface MeetingSummary {
  id: MeetingId;
  started_at: string;
  ended_at: string | null;
  platform: Platform | null;
  duration_ms: number | null;
  word_count: number;
  has_summary: boolean;
}

export interface MeetingState {
  in_meeting: boolean;
  platform: Platform | null;
  confidence: number;
  since: string | null;
}

export type AuthStatus =
  | { state: "signed_out" }
  | { state: "refreshing" }
  | { state: "signed"; account_email: string };

export interface AuthFlowHandle {
  flow_id: string;
  authorize_url: string;
}

export type RecordingState =
  | { state: "idle" }
  | { state: "recording"; meeting_id: MeetingId }
  | { state: "transcribing"; meeting_id: MeetingId };

export interface AudioDevice {
  name: string;
  is_default: boolean;
}

export interface ModelDownloadProgress {
  model: string;
  percent: number;
  downloaded: number;
  total: number | null;
}

export const api = {
  // Meetings
  listMeetings: (limit = 100, offset = 0) =>
    invoke<MeetingSummary[]>("list_meetings", { limit, offset }),
  getMeeting: (id: MeetingId) => invoke<Meeting | null>("get_meeting", { id }),
  deleteMeeting: (id: MeetingId) => invoke<void>("delete_meeting", { id }),

  // Detection
  detectPoll: () => invoke<MeetingState>("detect_poll"),
  knownPlatforms: () => invoke<Platform[]>("known_platforms"),

  // Recording
  recordingState: () => invoke<RecordingState>("recording_state"),
  startRecording: (manual = true) =>
    invoke<RecordingState>("start_recording", { manual }),
  stopRecording: () => invoke<RecordingState>("stop_recording"),

  // Auth
  authStatus: () => invoke<AuthStatus>("auth_status"),
  startLogin: () => invoke<AuthFlowHandle>("auth_start_login"),
  signOut: () => invoke<void>("auth_sign_out"),

  // AI
  aiSummarize: (meetingId: MeetingId) =>
    invoke<string>("ai_summarize", { meetingId }),
  aiExtractActionItems: (meetingId: MeetingId) =>
    invoke<string[]>("ai_extract_action_items", { meetingId }),
  aiExtractKeyDecisions: (meetingId: MeetingId) =>
    invoke<string[]>("ai_extract_key_decisions", { meetingId }),

  // Settings — general
  getAutoStart: () => invoke<boolean>("get_autostart"),
  setAutoStart: (enabled: boolean) =>
    invoke<void>("set_autostart", { enabled }),

  // Settings — recording
  listAudioInputDevices: () =>
    invoke<AudioDevice[]>("list_audio_input_devices"),

  // Settings — whisper
  downloadModel: (model: string) => invoke<void>("download_model", { model }),

  // Events
  onAuthStatusChange: (cb: (status: AuthStatus) => void): Promise<UnlistenFn> =>
    listen<AuthStatus>("auth://status", (e) => cb(e.payload)),
  onRecordingStateChange: (
    cb: (state: RecordingState) => void,
  ): Promise<UnlistenFn> =>
    listen<RecordingState>("recording://state", (e) => cb(e.payload)),
  onMeetingDetected: (
    cb: (state: MeetingState) => void,
  ): Promise<UnlistenFn> =>
    listen<MeetingState>("detect://change", (e) => cb(e.payload)),
  onModelDownloadProgress: (
    cb: (progress: ModelDownloadProgress) => void,
  ): Promise<UnlistenFn> =>
    listen<ModelDownloadProgress>("models://download_progress", (e) =>
      cb(e.payload),
    ),
};
