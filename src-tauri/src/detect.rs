use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::types::{MeetingState, MeetingStateChange, Platform};

const POLL_INTERVAL_SECS: u64 = 5;
const DEBOUNCE_COUNT: u32 = 3;

/// Handle returned by `start` — dropping it (or calling `stop`) halts the
/// background polling loop.
pub struct DetectHandle {
    stop: Arc<AtomicBool>,
    #[allow(dead_code)]
    pub(crate) _private: (),
}

impl DetectHandle {
    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for DetectHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// One-shot detection poll. Reads the current process / window snapshot and
/// returns a `MeetingState` without debouncing.
pub fn poll() -> Result<MeetingState> {
    #[cfg(windows)]
    {
        Ok(platform::poll_windows())
    }
    #[cfg(not(windows))]
    {
        Ok(idle_state())
    }
}

/// List of platforms this module can detect.
pub fn known_platforms() -> &'static [Platform] {
    &[
        Platform::Zoom,
        Platform::Meet,
        Platform::Teams,
        Platform::Slack,
        Platform::Discord,
        Platform::Webex,
    ]
}

/// Start a background polling loop. The callback fires only on state
/// transitions (meeting started / ended), after the ≥3-poll debounce.
/// The polling interval is implementation-defined (~5s in v1).
pub fn start<F>(callback: F) -> Result<DetectHandle>
where
    F: Fn(MeetingStateChange) + Send + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);

    thread::Builder::new()
        .name("noru-detect".to_string())
        .spawn(move || {
            let mut last_in_meeting = false;
            let mut positive_streak: u32 = 0;
            let mut negative_streak: u32 = 0;

            while !stop_clone.load(Ordering::Relaxed) {
                let current = poll().unwrap_or_else(|_| idle_state());

                if current.in_meeting {
                    positive_streak = positive_streak.saturating_add(1);
                    negative_streak = 0;
                } else {
                    negative_streak = negative_streak.saturating_add(1);
                    positive_streak = 0;
                }

                if !last_in_meeting && positive_streak >= DEBOUNCE_COUNT {
                    let state = MeetingState {
                        in_meeting: true,
                        platform: current.platform,
                        confidence: 1.0,
                        since: Some(now_iso8601()),
                    };
                    callback(MeetingStateChange::Started { state });
                    last_in_meeting = true;
                } else if last_in_meeting && negative_streak >= DEBOUNCE_COUNT {
                    let state = MeetingState {
                        in_meeting: false,
                        platform: None,
                        confidence: 0.0,
                        since: Some(now_iso8601()),
                    };
                    callback(MeetingStateChange::Ended { state });
                    last_in_meeting = false;
                }

                for _ in 0..POLL_INTERVAL_SECS {
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_secs(1));
                }
            }
        })?;

    Ok(DetectHandle {
        stop,
        _private: (),
    })
}

/// Pure-function test hook for the window-title matching logic. Given a raw
/// window title and its owning process name, returns the detected platform if
/// the title matches a "meeting active" pattern for that process.
///
/// Patterns are intentionally tight: *both* a known process and an active-call
/// title fragment must line up, so an idle Zoom launcher or an open Slack
/// workspace does not count as "in meeting".
pub fn parse_window_title_for_meeting(title: &str, process_name: &str) -> Option<Platform> {
    let title_lc = title.to_lowercase();
    let proc_lc = process_name.to_lowercase();
    let proc_stem = proc_lc.strip_suffix(".exe").unwrap_or(&proc_lc);

    // --- Zoom ---
    // Zoom uses the literal title "Zoom Meeting" (optionally with a suffix
    // like "Zoom Meeting - Free Account") once you are in an active call.
    // The launcher window is titled "Zoom" or "Zoom Workplace", which must
    // NOT match.
    if proc_stem == "zoom" && title_lc.contains("zoom meeting") {
        return Some(Platform::Zoom);
    }

    // --- Microsoft Teams ---
    // Classic Teams: "Meeting | Microsoft Teams" / "Meeting in 'Channel' | ...".
    // New Teams: "ms-teams.exe".
    if proc_stem == "teams" || proc_stem == "ms-teams" {
        if title_lc.contains("| microsoft teams")
            && (title_lc.contains("meeting") || title_lc.contains("call"))
        {
            return Some(Platform::Teams);
        }
        if title_lc.contains("microsoft teams meeting") {
            return Some(Platform::Teams);
        }
    }

    // --- Webex ---
    if proc_stem == "webex" || proc_stem == "webexmta" || proc_stem == "cisco webex meetings" {
        if title_lc.contains("meeting") || title_lc.contains("webex call") {
            return Some(Platform::Webex);
        }
    }

    // --- Slack huddles ---
    if proc_stem == "slack" && title_lc.contains("huddle") {
        return Some(Platform::Slack);
    }

    // --- Discord voice / video calls ---
    // Discord shows "Voice Connected" in the tray-state title and includes
    // the channel in the window title during a call.
    if proc_stem == "discord" && title_lc.contains("voice connected") {
        return Some(Platform::Discord);
    }

    // --- Google Meet in a browser ---
    // The browser window title during an active Meet call looks like
    // "Meet - abc-defg-hij" or "Meet – Google Chrome".
    let is_browser = matches!(
        proc_stem,
        "chrome" | "firefox" | "msedge" | "edge" | "brave" | "arc" | "opera" | "vivaldi"
    );
    if is_browser {
        if title_lc.contains("meet - ")
            || title_lc.contains("meet – ")
            || title_lc.contains("meet.google.com")
        {
            return Some(Platform::Meet);
        }
    }

    None
}

fn idle_state() -> MeetingState {
    MeetingState {
        in_meeting: false,
        platform: None,
        confidence: 0.0,
        since: None,
    }
}

fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    epoch_to_iso8601(secs)
}

/// Convert a Unix timestamp (seconds) to an RFC-3339 / ISO-8601 string in UTC.
/// Uses Howard Hinnant's `civil_from_days` algorithm so we do not need a
/// dedicated date crate (keeps Phase-1 dep lock intact).
fn epoch_to_iso8601(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let secs_of_day = timestamp.rem_euclid(86_400);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day / 60) % 60;
    let second = secs_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y_raw = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y_raw + 1 } else { y_raw };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hour, minute, second
    )
}

#[cfg(windows)]
mod platform {
    use std::collections::HashMap;

    use windows::core::BOOL;
    use windows::Win32::Foundation::{CloseHandle, HMODULE, HWND, LPARAM};
    use windows::Win32::System::ProcessStatus::{EnumProcesses, GetModuleBaseNameW};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };

    use super::{idle_state, parse_window_title_for_meeting};
    use crate::types::MeetingState;

    pub(super) fn poll_windows() -> MeetingState {
        let processes = enumerate_processes();
        let windows_snapshot = enumerate_windows();

        for (pid, title) in windows_snapshot {
            if title.is_empty() {
                continue;
            }
            if let Some(proc_name) = processes.get(&pid) {
                if let Some(platform) = parse_window_title_for_meeting(&title, proc_name) {
                    return MeetingState {
                        in_meeting: true,
                        platform: Some(platform),
                        confidence: 1.0,
                        since: None,
                    };
                }
            }
        }

        idle_state()
    }

    fn enumerate_processes() -> HashMap<u32, String> {
        let mut pids: Vec<u32> = vec![0; 4096];
        let mut bytes_returned: u32 = 0;

        let ok = unsafe {
            EnumProcesses(
                pids.as_mut_ptr(),
                (pids.len() * std::mem::size_of::<u32>()) as u32,
                &mut bytes_returned,
            )
        };
        if ok.is_err() {
            return HashMap::new();
        }

        let count = bytes_returned as usize / std::mem::size_of::<u32>();
        pids.truncate(count);

        let mut map = HashMap::with_capacity(count);
        for pid in pids {
            if pid == 0 {
                continue;
            }
            unsafe {
                let handle = match OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
                    false,
                    pid,
                ) {
                    Ok(h) => h,
                    Err(_) => continue,
                };
                if handle.is_invalid() {
                    continue;
                }
                let mut buf = [0u16; 260];
                let len = GetModuleBaseNameW(handle, HMODULE::default(), &mut buf);
                if len > 0 {
                    let name = String::from_utf16_lossy(&buf[..len as usize]);
                    map.insert(pid, name);
                }
                let _ = CloseHandle(handle);
            }
        }
        map
    }

    fn enumerate_windows() -> Vec<(u32, String)> {
        struct Ctx {
            out: Vec<(u32, String)>,
        }

        extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            unsafe {
                let ctx = &mut *(lparam.0 as *mut Ctx);
                if !IsWindowVisible(hwnd).as_bool() {
                    return BOOL(1);
                }
                let len = GetWindowTextLengthW(hwnd);
                if len <= 0 {
                    return BOOL(1);
                }
                let mut buf = vec![0u16; (len + 1) as usize];
                let copied = GetWindowTextW(hwnd, &mut buf);
                if copied <= 0 {
                    return BOOL(1);
                }
                let title = String::from_utf16_lossy(&buf[..copied as usize]);
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid != 0 {
                    ctx.out.push((pid, title));
                }
                BOOL(1)
            }
        }

        let mut ctx = Ctx { out: Vec::new() };
        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
        }
        ctx.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zoom_meeting() {
        assert_eq!(
            parse_window_title_for_meeting("Zoom Meeting", "Zoom.exe"),
            Some(Platform::Zoom)
        );
        assert_eq!(
            parse_window_title_for_meeting("Zoom Meeting - Free Account", "Zoom.exe"),
            Some(Platform::Zoom)
        );
    }

    #[test]
    fn ignores_idle_zoom_launcher() {
        assert_eq!(parse_window_title_for_meeting("Zoom", "Zoom.exe"), None);
        assert_eq!(
            parse_window_title_for_meeting("Zoom Workplace", "Zoom.exe"),
            None
        );
    }

    #[test]
    fn detects_teams_meeting() {
        assert_eq!(
            parse_window_title_for_meeting(
                "Meeting | Microsoft Teams",
                "Teams.exe"
            ),
            Some(Platform::Teams)
        );
        assert_eq!(
            parse_window_title_for_meeting(
                "Microsoft Teams meeting in progress",
                "ms-teams.exe"
            ),
            Some(Platform::Teams)
        );
    }

    #[test]
    fn ignores_idle_teams() {
        assert_eq!(
            parse_window_title_for_meeting("Chat | Microsoft Teams", "Teams.exe"),
            None
        );
    }

    #[test]
    fn detects_slack_huddle() {
        assert_eq!(
            parse_window_title_for_meeting("Huddle in #engineering", "slack.exe"),
            Some(Platform::Slack)
        );
        assert_eq!(
            parse_window_title_for_meeting("noru — Slack", "slack.exe"),
            None
        );
    }

    #[test]
    fn detects_google_meet_in_browsers() {
        assert_eq!(
            parse_window_title_for_meeting(
                "Meet - abc-defg-hij — Google Chrome",
                "chrome.exe"
            ),
            Some(Platform::Meet)
        );
        assert_eq!(
            parse_window_title_for_meeting(
                "meet.google.com/abc-defg-hij - Mozilla Firefox",
                "firefox.exe"
            ),
            Some(Platform::Meet)
        );
        assert_eq!(
            parse_window_title_for_meeting(
                "Meet - abc-defg-hij - Microsoft Edge",
                "msedge.exe"
            ),
            Some(Platform::Meet)
        );
    }

    #[test]
    fn ignores_meet_in_non_browser() {
        assert_eq!(
            parse_window_title_for_meeting("Meet - abc", "notepad.exe"),
            None
        );
    }

    #[test]
    fn detects_discord_voice() {
        assert_eq!(
            parse_window_title_for_meeting(
                "#general — Voice Connected — Discord",
                "Discord.exe"
            ),
            Some(Platform::Discord)
        );
    }

    #[test]
    fn known_platforms_list_non_empty() {
        assert!(!known_platforms().is_empty());
    }

    #[test]
    fn epoch_unix_zero_formats_as_1970() {
        assert_eq!(epoch_to_iso8601(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn epoch_known_date_formats_correctly() {
        // 2026-04-11T00:00:00Z = 1775865600
        assert_eq!(epoch_to_iso8601(1_775_865_600), "2026-04-11T00:00:00Z");
        // 2000-02-29T12:34:56Z exercises a leap-day in a century divisible by 400
        assert_eq!(epoch_to_iso8601(951_827_696), "2000-02-29T12:34:56Z");
    }
}
