//! Port of `lib/core/config/env_config.dart`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Default)]
pub struct EnvConfig {
    map: HashMap<String, String>,
    initialized: bool,
}

impl EnvConfig {
    fn new() -> Self {
        let mut map = HashMap::new();
        if let Some(file) = find_env_file() {
            if let Ok(lines) = std::fs::read_to_string(&file) {
                for line in lines.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    if let Some(idx) = trimmed.find('=') {
                        if idx > 0 {
                            let key = trimmed[..idx].trim().to_string();
                            let value = trimmed[idx + 1..].trim().to_string();
                            map.insert(key, value);
                        }
                    }
                }
            }
        }
        Self { map, initialized: true }
    }

    pub fn get(key: &str) -> Option<String> {
        let env = env_instance();
        env.map.get(key).cloned()
    }

    pub fn get_google_client_id(&self) -> String {
        client_id_from_env("GOOGLE_CLIENT_ID", &self.map)
    }

    pub fn get_google_client_secret(&self) -> String {
        client_id_from_env("GOOGLE_CLIENT_SECRET", &self.map)
    }

    pub fn get_discord_client_id(&self) -> String {
        let from_dart_define = sanitize_snowflake(option_env!("DISCORD_CLIENT_ID"));
        if let Some(v) = from_dart_define {
            return v;
        }
        if let Some(v) = sanitize_snowflake(self.map.get("DISCORD_CLIENT_ID").map(|s| s.as_str())) {
            return v;
        }
        if let Ok(v) = std::env::var("DISCORD_CLIENT_ID") {
            if let Some(v2) = sanitize_snowflake(Some(&v)) {
                return v2;
            }
        }
        String::new()
    }
}

fn client_id_from_env(key: &str, map: &HashMap<String, String>) -> String {
    if let Some(v) = option_env!("GOOGLE_CLIENT_ID") {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    if let Some(v) = map.get(key) {
        if !v.is_empty() {
            return v.clone();
        }
    }
    if let Ok(v) = std::env::var(key) {
        if !v.is_empty() {
            return v;
        }
    }
    String::new()
}

fn sanitize_snowflake(value: Option<&str>) -> Option<String> {
    let value = value?;
    let cleaned = value.trim().trim_matches('"').trim_matches('\'').to_string();
    let digits = cleaned.chars().all(|c| c.is_ascii_digit()) && cleaned.len() >= 17 && cleaned.len() <= 20;
    if digits {
        Some(cleaned)
    } else {
        None
    }
}

static ENV: OnceLock<EnvConfig> = OnceLock::new();

pub fn env_instance() -> &'static EnvConfig {
    ENV.get_or_init(EnvConfig::new)
}

fn find_env_file() -> Option<PathBuf> {
    // 1. Relative ./.env
    if let Ok(cwd) = std::env::current_dir() {
        let direct = cwd.join(".env");
        if direct.is_file() {
            return Some(direct);
        }
    }
    // 2. Search upwards from current directory (5 levels)
    if let Ok(mut dir) = std::env::current_dir() {
        for _ in 0..5 {
            let candidate = dir.join(".env");
            if candidate.is_file() {
                return Some(candidate);
            }
            if !dir.pop() {
                break;
            }
        }
    }
    // 3. Search upwards from executable directory (6 levels)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(mut dir) = exe.parent().map(PathBuf::from) {
            for _ in 0..6 {
                let candidate = dir.join(".env");
                if candidate.is_file() {
                    return Some(candidate);
                }
                if !dir.pop() {
                    break;
                }
            }
        }
    }
    None
}
