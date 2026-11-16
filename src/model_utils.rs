//! Helpers shared by model parsing: tolerant int/string/bool coercion.

use serde_json::Value;

pub fn get_str(json: &Value, key: &str) -> String {
    match json.get(key) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

pub fn get_int(json: &Value, key: &str) -> Option<i64> {
    json.get(key).and_then(int_value)
}

pub fn int_value(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub fn first_int(json: &Value, keys: &[&str]) -> Option<i64> {
    for k in keys {
        if let Some(v) = json.get(k).and_then(int_value) {
            return Some(v);
        }
    }
    None
}

pub fn first_str(json: &Value, keys: &[&str]) -> String {
    for (i, k) in keys.iter().enumerate() {
        let v = get_str(json, k);
        if !v.is_empty() || i + 1 == keys.len() {
            return v;
        }
    }
    String::new()
}

pub fn get_bool(json: &Value, key: &str) -> Option<bool> {
    json.get(key).and_then(bool_value)
}

pub fn first_bool(json: &Value, keys: &[&str]) -> Option<bool> {
    for k in keys {
        if let Some(v) = json.get(k).and_then(bool_value) {
            return Some(v);
        }
    }
    None
}

/// Bool coercion for an optional field lookup, e.g. `bool_opt(json.get("dl"))`.
pub fn bool_opt(v: Option<&Value>) -> Option<bool> {
    v.and_then(bool_value)
}

pub fn bool_value(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_f64().map(|f| f != 0.0),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub fn if_empty(s: String, fallback: &str) -> String {
    if s.is_empty() { fallback.to_string() } else { s }
}
