//! JSON-backed key-value preferences at `<app_data_dir>/settings.json`.
//!
//! Designed for ~10 keys. Atomically rewrites the file on each `set` (write
//! to temp + rename). Process-level Mutex serializes concurrent writes from
//! different Tauri command handlers.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

static STATE: OnceLock<Mutex<PrefsState>> = OnceLock::new();

struct PrefsState {
    path: PathBuf,
    data: HashMap<String, Value>,
}

/// Must be called once at boot with Tauri's `app_data_dir`. Creates the
/// directory and loads any existing settings file.
pub fn init(app_data_dir: PathBuf) -> Result<()> {
    if STATE.get().is_some() {
        return Ok(());
    }
    std::fs::create_dir_all(&app_data_dir)
        .with_context(|| format!("creating {}", app_data_dir.display()))?;

    let path = app_data_dir.join("settings.json");
    let data = if path.exists() {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let _ = STATE.set(Mutex::new(PrefsState { path, data }));
    Ok(())
}

pub fn get(key: &str) -> Result<Option<Value>> {
    let guard = lock()?;
    Ok(guard.data.get(key).cloned())
}

pub fn set(key: &str, value: Value) -> Result<()> {
    let mut guard = lock()?;
    guard.data.insert(key.to_string(), value);
    flush(&guard)
}

pub fn list() -> Result<HashMap<String, Value>> {
    let guard = lock()?;
    Ok(guard.data.clone())
}

fn lock() -> Result<std::sync::MutexGuard<'static, PrefsState>> {
    STATE
        .get()
        .ok_or_else(|| anyhow!("prefs not initialized; call prefs::init first"))?
        .lock()
        .map_err(|_| anyhow!("prefs mutex poisoned"))
}

fn flush(state: &PrefsState) -> Result<()> {
    let json = serde_json::to_string_pretty(&state.data)?;
    let tmp = state.path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)
        .with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, &state.path)
        .with_context(|| format!("renaming {} → {}", tmp.display(), state.path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn roundtrip_get_set_list() {
        let dir = std::env::temp_dir().join(format!("noru-prefs-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        // Use a local state instead of the global for test isolation.
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let state = Mutex::new(PrefsState {
            path: path.clone(),
            data: HashMap::new(),
        });

        {
            let mut g = state.lock().unwrap();
            g.data.insert("theme".into(), Value::String("dark".into()));
            flush(&g).unwrap();
        }

        let raw = fs::read_to_string(&path).unwrap();
        let loaded: HashMap<String, Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(loaded.get("theme").unwrap(), "dark");

        let _ = fs::remove_dir_all(&dir);
    }
}
