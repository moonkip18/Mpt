//! Port of `lib/core/utils/*` and `lib/shared/format.dart`.

pub mod json_utils {
    pub fn safe_int(val: Option<serde_json::Value>, fallback: i64) -> i64 {
        match val {
            None => fallback,
            Some(serde_json::Value::Number(n)) => n
                .as_i64()
                .or_else(|| n.as_f64().map(|v| v.round() as i64))
                .unwrap_or(fallback),
            Some(serde_json::Value::String(s)) => s.trim().parse::<i64>().unwrap_or(fallback),
            Some(v) => v.as_i64().unwrap_or(fallback),
        }
    }

    pub fn safe_string(val: Option<serde_json::Value>, fallback: &str) -> String {
        match val {
            None => fallback.to_string(),
            Some(serde_json::Value::String(s)) => s,
            Some(v) => v.to_string(),
        }
    }

    pub fn safe_bool(val: Option<serde_json::Value>, fallback: bool) -> bool {
        match val {
            None => fallback,
            Some(serde_json::Value::Bool(b)) => b,
            Some(serde_json::Value::Number(n)) => n.as_f64().map(|v| v != 0.0).unwrap_or(fallback),
            Some(serde_json::Value::String(s)) => {
                let lower = s.trim().to_lowercase();
                match lower.as_str() {
                    "true" | "1" => true,
                    "false" | "0" => false,
                    _ => fallback,
                }
            }
            _ => fallback,
        }
    }

    pub fn clamp_ms_u64(ms: i64) -> u64 {
        if ms < 0 { 0 } else { ms as u64 }
    }
}

pub mod study_date {
    use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike};

    pub const DEFAULT_RESET_HOUR: u32 = 5;

    pub fn get_study_date(now: DateTime<Local>, reset_hour: u32) -> DateTime<Local> {
        let reset = reset_hour.min(23);
        if now.hour() < reset {
            let prev = now - Duration::days(1);
            return Local
                .with_ymd_and_hms(prev.year(), prev.month(), prev.day(), 0, 0, 0)
                .single()
                .unwrap_or(now);
        }
        Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .unwrap_or(now)
    }

    pub fn get_study_date_string(now: DateTime<Local>, reset_hour: u32) -> String {
        let date = get_study_date(now, reset_hour);
        format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
    }

    pub fn is_same_study_day(a: DateTime<Local>, b: DateTime<Local>, reset_hour: u32) -> bool {
        get_study_date(a, reset_hour).date_naive() == get_study_date(b, reset_hour).date_naive()
    }

    pub fn get_next_reset_time(now: DateTime<Local>, reset_hour: u32) -> DateTime<Local> {
        let reset = reset_hour.min(23);
        if now.hour() < reset {
            return Local
                .with_ymd_and_hms(now.year(), now.month(), now.day(), reset, 0, 0)
                .single()
                .unwrap_or(now);
        }
        let next = now + Duration::days(1);
        Local
            .with_ymd_and_hms(next.year(), next.month(), next.day(), reset, 0, 0)
            .single()
            .unwrap_or(now)
    }
}

pub mod format {
    pub fn format_clock(ms: i64) -> String {
        let total = (ms / 1000).max(0);
        let hours = total / 3600;
        let minutes = (total % 3600) / 60;
        let seconds = total % 60;
        let two = |v: i64| format!("{v:02}");
        if hours > 0 {
            format!("{}:{}:{}", two(hours), two(minutes), two(seconds))
        } else {
            format!("{}:{}", two(minutes), two(seconds))
        }
    }

    pub fn format_hm(ms: i64) -> String {
        let total = (ms / 1000).max(0);
        let hours = total / 3600;
        let minutes = (total % 3600) / 60;
        if hours == 0 && minutes == 0 {
            return if total > 0 { "<1m".to_string() } else { "0m".to_string() };
        }
        if hours == 0 {
            return format!("{minutes}m");
        }
        if minutes == 0 {
            return format!("{hours}h");
        }
        format!("{hours}h {minutes}m")
    }

    pub fn format_hm_compact(ms: i64) -> String {
        let total = ms.max(0);
        let hours = total as f64 / 3_600_000.0;
        if hours >= 10.0 {
            return format!("{hours:.0}h");
        }
        if hours >= 1.0 {
            return format!("{hours:.1}h");
        }
        let minutes = total / 60_000;
        format!("{minutes}m")
    }

    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    pub fn format_day_label(date: chrono::NaiveDate) -> String {
        use chrono::Datelike;
        let weekday = WEEKDAYS[date.weekday().num_days_from_monday() as usize];
        let month = MONTHS[(date.month() - 1) as usize];
        format!("{weekday} {} {month}", date.day())
    }
}

pub mod color {
    /// Palette YPT offers when a subject is created.
    pub const DEFAULT_SUBJECT_COLORS: &[u32] = &[
        0xFFE53935, 0xFFF44336, 0xFFFF5252, 0xFFFF6B6B, 0xFFFF7675, 0xFFE17055,
        0xFFD81B60, 0xFFE91E63, 0xFFFF4081, 0xFFF06292, 0xFFFD79A8, 0xFFBA68C8,
        0xFF8E24AA, 0xFF9C27B0, 0xFF673AB7, 0xFF7E57C2, 0xFFA29BFE, 0xFF5B6AF0,
        0xFF1976D2, 0xFF2196F3, 0xFF42A5F5, 0xFF74B9FF, 0xFF00ACC1, 0xFF00BCD4,
        0xFF26C6DA, 0xFF4ECDC4, 4284513675,
        0xFF2E7D32, 0xFF4CAF50, 0xFF81C784, 0xFF8BC34A, 0xFF009688, 0xFF00B894,
        0xFF55EFC4,
        0xFFFFD54F, 0xFFFFC107, 0xFFFF9800, 0xFFFB8C00, 0xFFFF5722, 0xFF795548,
        0xFF8D6E63, 0xFF607D8B, 0xFF455A64,
    ];

    pub fn from_argb_int(argb: u32) -> u32 {
        if argb == 0 {
            0xFFA78BFA
        } else {
            argb
        }
    }

    pub fn to_argb_int(a: u8, r: u8, g: u8, b: u8) -> u32 {
        ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }
}
