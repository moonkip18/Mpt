//! Persistent storage. Replaces `shared_preferences` + `flutter_secure_storage`
//! with JSON files under `%APPDATA%\Mpt` (mirrors the Flutter app's own
//! dual-write of the JWT into both backends).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{Map, Value};

const PREFS_FILE: &str = "prefs.json";
const SECURE_FILE: &str = "secure.json";

pub struct Store {
    prefs: Map<String, Value>,
    secure: Map<String, Value>,
    prefs_path: PathBuf,
    secure_path: PathBuf,
    prefs_dirty: bool,
    secure_dirty: bool,
}

impl Store {
    pub fn load() -> Self {
        let dir = app_data_dir();
        let _ = fs::create_dir_all(&dir);
        let prefs_path = dir.join(PREFS_FILE);
        let secure_path = dir.join(SECURE_FILE);
        let prefs = load_json(&prefs_path).unwrap_or_default();
        let secure = load_json(&secure_path).unwrap_or_default();
        Self {
            prefs,
            secure,
            prefs_path,
            secure_path,
            prefs_dirty: false,
            secure_dirty: false,
        }
    }

    pub fn flush(&mut self) {
        if self.prefs_dirty {
            let _ = fs::write(&self.prefs_path, serde_json::to_string_pretty(&self.prefs).unwrap_or_default());
            self.prefs_dirty = false;
        }
        if self.secure_dirty {
            let _ = fs::write(&self.secure_path, serde_json::to_string_pretty(&self.secure).unwrap_or_default());
            self.secure_dirty = false;
        }
    }

    // -- shared prefs surface ------------------------------------------------

    pub fn prefs_get_string(&self, key: &str) -> Option<String> {
        self.prefs.get(key).and_then(|v| v.as_str()).map(String::from)
    }

    pub fn prefs_get_int(&self, key: &str) -> Option<i64> {
        self.prefs.get(key).and_then(|v| v.as_i64())
    }

    pub fn prefs_get_string_list(&self, key: &str) -> Vec<String> {
        self.prefs
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    pub fn prefs_set_string(&mut self, key: &str, value: &str) {
        self.prefs.insert(key.to_string(), Value::String(value.to_string()));
        self.prefs_dirty = true;
    }

    pub fn prefs_set_int(&mut self, key: &str, value: i64) {
        self.prefs.insert(key.to_string(), Value::Number(value.into()));
        self.prefs_dirty = true;
    }

    pub fn prefs_set_string_list(&mut self, key: &str, values: &[String]) {
        self.prefs.insert(
            key.to_string(),
            Value::Array(values.iter().map(|v| Value::String(v.clone())).collect()),
        );
        self.prefs_dirty = true;
    }

    pub fn prefs_remove(&mut self, key: &str) {
        if self.prefs.remove(key).is_some() {
            self.prefs_dirty = true;
        }
    }

    // -- secure storage surface ----------------------------------------------

    pub fn secure_get(&self, key: &str) -> Option<String> {
        self.secure.get(key).and_then(|v| v.as_str()).map(String::from)
    }

    pub fn secure_set(&mut self, key: &str, value: &str) {
        self.secure.insert(key.to_string(), Value::String(value.to_string()));
        self.secure_dirty = true;
    }

    pub fn secure_delete(&mut self, key: &str) {
        if self.secure.remove(key).is_some() {
            self.secure_dirty = true;
        }
    }
}

fn load_json(path: &std::path::Path) -> Option<Map<String, Value>> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&raw).ok().and_then(|v| v.as_object().cloned())
}

pub(crate) fn app_data_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("Mpt");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config").join("mpt");
    }
    PathBuf::from(".mpt")
}

pub type SharedStore = Mutex<Store>;
