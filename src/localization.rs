//! Port of `lib/core/localization/app_translation.dart` — English dictionary only.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

const EN_JSON: &str = include_str!("../assets/translations/en.json");
const ALIASES_JSON: &str = include_str!("../assets/translations/aliases.json");
const FALLBACK_JSON: &str = include_str!("../assets/translations/fallback.json");

pub struct AppTranslation {
    language_code: String,
    translations: HashMap<String, String>,
    aliases: HashMap<String, Vec<String>>,
    fallback_dict: HashMap<String, HashMap<String, String>>,
}

impl AppTranslation {
    fn new(language_code: &str) -> Self {
        let translations = parse_flat_map(EN_JSON);
        let aliases_value: Value = serde_json::from_str(ALIASES_JSON).unwrap_or(Value::Null);
        let mut aliases = HashMap::new();
        if let Value::Object(map) = aliases_value {
            for (k, v) in map {
                if let Value::Array(items) = v {
                    let list = items
                        .into_iter()
                        .filter_map(|i| i.as_str().map(str::to_string))
                        .collect();
                    aliases.insert(k.to_lowercase(), list);
                }
            }
        }
        let fallback_value: Value = serde_json::from_str(FALLBACK_JSON).unwrap_or(Value::Null);
        let mut fallback_dict = HashMap::new();
        if let Value::Object(map) = fallback_value {
            for (k, v) in map {
                if let Value::Object(inner) = v {
                    let mut entry = HashMap::new();
                    for (lk, lv) in inner {
                        if let Some(s) = lv.as_str() {
                            entry.insert(lk.to_lowercase(), s.to_string());
                        }
                    }
                    fallback_dict.insert(k.to_lowercase(), entry);
                }
            }
        }
        Self {
            language_code: language_code.to_string(),
            translations,
            aliases,
            fallback_dict,
        }
    }

    pub fn load(language_code: &str) -> Self {
        Self::new(&normalize_code(language_code))
    }

    pub fn supports_language(language_code: &str) -> bool {
        normalize_code(language_code) == "en"
    }

    fn fallback_for(&self, key: &str) -> Option<String> {
        let map = self.fallback_dict.get(&key.to_lowercase())?;
        let lang = self.language_code.to_lowercase();
        if lang == "pt" || lang.starts_with("pt-") {
            map.get(&lang).cloned().or_else(|| map.get("pt").cloned()).or_else(|| map.get("en").cloned())
        } else {
            map.get(&lang).cloned().or_else(|| map.get("en").cloned())
        }
    }

    pub fn tr(&self, key: &str, args: Option<Vec<&str>>, fallback: Option<&str>) -> String {
        let mut val = self.translations.get(key).cloned();

        if val.as_deref().map(str::is_empty).unwrap_or(true) {
            if let Some(list) = self.aliases.get(&key.to_lowercase()) {
                for alias in list {
                    if let Some(v) = self.translations.get(alias) {
                        if !v.is_empty() {
                            val = Some(v.clone());
                            break;
                        }
                    }
                }
            }
        }

        if val.as_deref().map(str::is_empty).unwrap_or(true) {
            val = self.fallback_for(key);
        }

        if val.as_deref().map(str::is_empty).unwrap_or(true) {
            if let Some(list) = self.aliases.get(&key.to_lowercase()) {
                for alias in list {
                    if let Some(v) = self.fallback_for(alias) {
                        if !v.is_empty() {
                            val = Some(v);
                            break;
                        }
                    }
                }
            }
        }

        let mut out = val.unwrap_or_else(|| fallback.map(str::to_string).unwrap_or_else(|| key.to_string()));

        if let Some(args) = args {
            for arg in args {
                if let Some(idx) = out.find("{}") {
                    out.replace_range(idx..idx + 2, arg);
                }
            }
        }
        out
    }
}

fn parse_flat_map(json: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(Value::Object(obj)) = serde_json::from_str(json) {
        for (k, v) in obj {
            match v {
                Value::String(s) => {
                    map.insert(k, s);
                }
                other => {
                    map.insert(k, other.to_string());
                }
            }
        }
    }
    map
}

fn normalize_code(code: &str) -> String {
    let lower = code.trim().to_lowercase().replace('_', "-");
    match lower.as_str() {
        "zh-hans" | "zh-cn" | "zh" => "zh-CN".to_string(),
        "zh-hant" | "zh-tw" | "zh-hk" => "zh-TW".to_string(),
        other => other.to_string(),
    }
}

static CURRENT: OnceLock<AppTranslation> = OnceLock::new();

pub fn current() -> &'static AppTranslation {
    CURRENT.get_or_init(|| AppTranslation::load("en"))
}

/// Global one-shot English dictionary. Mirrors `AppTranslation.current` with `en`.
pub fn tr_msg(key: &str, args: Option<Vec<&str>>, fallback: Option<&str>) -> String {
    current().tr(key, args, fallback)
}
