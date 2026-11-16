//! Port of `lib/data/daily_stats.dart`.

use serde_json::Value;

use crate::model_utils::get_int;
use crate::models::SubjectModel;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SubjectStat {
    pub id: Option<i64>,
    pub title: String,
    pub ms: i64,
    pub color_int: i64,
}

impl SubjectStat {
    pub fn copy_with(&self, ms: Option<i64>, id: Option<i64>) -> Self {
        Self {
            id: id.or(self.id),
            title: self.title.clone(),
            ms: ms.unwrap_or(self.ms),
            color_int: self.color_int,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DailyStats {
    pub date: String,
    pub total_ms: i64,
    pub rest_ms: i64,
    pub subjects: Vec<SubjectStat>,
}

impl DailyStats {
    pub fn empty(date: &str) -> Self {
        Self { date: date.to_string(), total_ms: 0, rest_ms: 0, subjects: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.total_ms == 0 && self.subjects.is_empty()
    }

    pub fn ms_for(&self, id: Option<i64>, title: &str) -> i64 {
        for s in &self.subjects {
            if let (Some(idv), Some(sid)) = (id, s.id) {
                if idv == sid {
                    return s.ms;
                }
            }
            if s.title == title {
                return s.ms;
            }
        }
        0
    }

    pub fn from_daily_log(
        dl: Option<&Value>,
        date: &str,
        known_subjects: &[SubjectModel],
    ) -> Self {
        let Some(dl) = dl else {
            return Self::empty(date);
        };

        let total_ms = get_int(dl, "sm")
            .or_else(|| get_int(dl, "tp"))
            .unwrap_or(0);
        let rest_raw = get_int(dl, "rm").or_else(|| get_int(dl, "rt")).unwrap_or(0);
        let rest_ms = if rest_raw > 0 && rest_raw < 500_000 { rest_raw * 1000 } else { rest_raw };

        let colour_by_name: std::collections::HashMap<String, i64> = known_subjects
            .iter()
            .map(|s| (s.title.clone(), s.color_int))
            .collect();
        let colour_by_id: std::collections::HashMap<i64, i64> = known_subjects
            .iter()
            .map(|s| (s.id, s.color_int))
            .collect();

        let mut ss_totals: std::collections::HashMap<String, SubjectStat> = Default::default();
        let mut ls_totals: std::collections::HashMap<String, SubjectStat> = Default::default();

        let add_to = |into: &mut std::collections::HashMap<String, SubjectStat>,
                      raw_title: String,
                      id: Option<i64>,
                      ms: i64,
                      colour: Option<i64>| {
            let title = raw_title.trim().to_string();
            if title.is_empty() || ms <= 0 {
                return;
            }
            let resolved = colour
                .or_else(|| id.and_then(|i| colour_by_id.get(&i).copied()))
                .or_else(|| colour_by_name.get(&title).copied())
                .unwrap_or_else(|| fallback_colour(&title));
            match into.get_mut(&title) {
                Some(existing) => {
                    existing.id = existing.id.or(id);
                    existing.ms += ms;
                }
                None => {
                    into.insert(title.clone(), SubjectStat { id, title, ms, color_int: resolved });
                }
            }
        };

        if let Some(ss) = dl.get("ss").and_then(|v| v.as_array()) {
            for item in ss {
                let title = get_str_local(item, "tt").if_empty_then(get_str_local(item, "sb"));
                add_to(
                    &mut ss_totals,
                    title,
                    get_int(item, "id"),
                    get_int(item, "sm").unwrap_or(0),
                    get_int(item, "co"),
                );
            }
        }

        if let Some(ls) = dl.get("ls").and_then(|v| v.as_array()) {
            for item in ls {
                let title = get_str_local(item, "sb").if_empty_then(get_str_local(item, "tt"));
                let id = get_int(item, "id").or_else(|| get_int(item, "subjectId"));
                add_to(
                    &mut ls_totals,
                    title,
                    id,
                    get_int(item, "sm").unwrap_or(0),
                    get_int(item, "co"),
                );
            }
        }

        let mut merged: std::collections::HashMap<String, SubjectStat> = Default::default();
        for entry in ss_totals.into_values().chain(ls_totals.into_values()) {
            match merged.get_mut(&entry.title) {
                Some(existing) => {
                    existing.id = existing.id.or(entry.id);
                    existing.ms = existing.ms.max(entry.ms);
                }
                None => {
                    merged.insert(entry.title.clone(), entry);
                }
            }
        }

        let mut stats: Vec<SubjectStat> = merged.into_values().collect();
        stats.sort_by(|a, b| b.ms.cmp(&a.ms));

        let sum_subjects: i64 = stats.iter().map(|s| s.ms).sum();
        Self {
            date: date.to_string(),
            total_ms: total_ms.max(sum_subjects),
            rest_ms,
            subjects: stats,
        }
    }

    pub fn with_live_delta(
        &self,
        subject_id: Option<i64>,
        subject_title: Option<&str>,
        live_ms: i64,
    ) -> Self {
        if live_ms <= 0 || (subject_id.is_none() && subject_title.is_none()) {
            return self.clone();
        }

        let mut matched = false;
        let mut updated: Vec<SubjectStat> = Vec::with_capacity(self.subjects.len() + 1);
        for s in &self.subjects {
            let is_same = subject_id.is_some_and(|id| s.id == Some(id))
                || subject_title.is_some_and(|t| s.title == t);
            if is_same && !matched {
                matched = true;
                updated.push(s.copy_with(Some(s.ms + live_ms), None));
            } else {
                updated.push(s.clone());
            }
        }
        if !matched {
            updated.push(SubjectStat {
                id: subject_id,
                title: subject_title.unwrap_or("Study").to_string(),
                ms: live_ms,
                color_int: fallback_colour(subject_title.unwrap_or("Study")),
            });
        }
        updated.sort_by(|a, b| b.ms.cmp(&a.ms));
        Self {
            date: self.date.clone(),
            total_ms: self.total_ms + live_ms,
            rest_ms: self.rest_ms,
            subjects: updated,
        }
    }
}

fn get_str_local(json: &Value, key: &str) -> String {
    json.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_default()
}

trait IfEmpty {
    fn if_empty_then(self, f: String) -> String;
}

impl IfEmpty for String {
    fn if_empty_then(self, f: String) -> String {
        if self.is_empty() { f } else { self }
    }
}

pub fn fallback_colour(title: &str) -> i64 {
    let mut hash: i64 = 0;
    for unit in title.encode_utf16() {
        hash = (hash * 31 + unit as i64) & 0x7fff_ffff;
    }
    const PALETTE: [i64; 7] = [
        0xFF8FA6B8,
        0xFFB89B8F,
        0xFF8FB89B,
        0xFFB8B08F,
        0xFFA68FB8,
        0xFF8FA6B8,
        0xFFB88FA6,
    ];
    PALETTE[(hash % PALETTE.len() as i64) as usize]
}
