//! Port of `lib/data/repositories/subject_repository.dart`.

use serde_json::Value;

use crate::api::{ApiClient, ApiError};
use crate::constants::{RELOAD_INFO, SPLASH_LOGIN, SUBJECT_CREATE};
use crate::localization::tr_msg;
use crate::models::SubjectModel;
use crate::store::Store;

use std::sync::{Arc, Mutex};

pub struct SubjectFetchResult {
    pub subjects: Vec<SubjectModel>,
    pub today_total_ms: i64,
    pub today_rest_ms: i64,
}

/// Subject fetching behind a trait so tests can substitute fakes, mirroring
/// the Dart project's `FakeSubjectRepository`.
pub trait SubjectsApi: Send + Sync {
    fn fetch_subjects_data(&self, language: &str, timezone: &str, version: i64)
        -> SubjectFetchResult;
}

pub struct SubjectRepository {
    api: Arc<Mutex<ApiClient>>,
    store: Arc<Mutex<Store>>,
}

impl SubjectRepository {
    pub fn new(store: &Arc<Mutex<Store>>) -> Self {
        Self {
            api: Arc::new(Mutex::new(ApiClient::new(store.clone()))),
            store: store.clone(),
        }
    }

    fn with_client<F, T>(&self, f: F) -> Result<T, ApiError>
    where
        F: FnOnce(&ApiClient) -> Result<T, ApiError>,
    {
        let client = self.api.lock().unwrap();
        f(&client)
    }

    pub fn save_subjects_from_raw_data(store: &Arc<Mutex<Store>>, data: &Value) {
        let raw_list = data
            .get("ss")
            .cloned()
            .or_else(|| data.get("p").and_then(|p| p.get("ss")).cloned());
        let Some(list) = raw_list.as_ref().and_then(|v| v.as_array()) else { return; };
        if list.is_empty() {
            return;
        }
        let subjects: Vec<SubjectModel> = list
            .iter()
            .map(SubjectModel::from_json)
            .filter(|s| !s.is_deleted)
            .collect();
        if subjects.is_empty() {
            return;
        }
        let json_list: Vec<Value> = subjects.iter().map(|s| s.to_json()).collect();
        let mut store = store.lock().unwrap();
        store.prefs_set_string("cached_subjects_data", &serde_json::to_string(&json_list).unwrap_or_default());
        store.flush();
    }

    pub fn cache_subjects(&self, subjects: &[SubjectModel]) {
        let json_list: Vec<Value> = subjects.iter().map(|s| s.to_json()).collect();
        let mut store = self.store.lock().unwrap();
        store.prefs_set_string("cached_subjects_data", &serde_json::to_string(&json_list).unwrap_or_default());
        store.flush();
    }

    pub fn get_cached_subjects(&self) -> Vec<SubjectModel> {
        let store = self.store.lock().unwrap();
        let Some(raw) = store.prefs_get_string("cached_subjects_data") else { return vec![]; };
        if raw.is_empty() {
            return vec![];
        }
        let Ok(decoded) = serde_json::from_str::<Value>(&raw) else { return vec![]; };
        let Some(arr) = decoded.as_array() else { return vec![]; };
        arr.iter()
            .map(SubjectModel::from_json)
            .filter(|s| !s.is_deleted)
            .collect()
    }

    pub fn create_subject(&self, title: &str, color_int: i64) -> Result<SubjectModel, ApiError> {
        let body = serde_json::json!({ "title": title, "color": color_int });
        let resp = self.with_client(|c| c.post(SUBJECT_CREATE, &body, None, false))?;
        let data = resp.data;
        if data.get("s").and_then(|v| v.as_bool()).unwrap_or(false) != true {
            let msg = data.get("m").and_then(|v| v.as_str()).map(String::from)
                .unwrap_or_else(|| tr_msg("subject_create_failed", None, Some("Falha ao criar matéria")));
            return Err(ApiError::Decode(msg));
        }
        let sb = data.get("sb").or_else(|| data.get("subject")).cloned();
        let created = match sb {
            Some(sb) => SubjectModel::from_json(&sb),
            None => SubjectModel::from_json(&data),
        };
        let mut cached = self.get_cached_subjects();
        cached.retain(|s| s.id != created.id);
        cached.push(created.clone());
        self.cache_subjects(&cached);
        Ok(created)
    }

    pub fn fetch_subjects_data(
        &self,
        language: &str,
        timezone: &str,
        version: i64,
    ) -> SubjectFetchResult {
        if let Ok(resp) = self.with_client(|c| {
            c.post(RELOAD_INFO, &serde_json::json!({
                "pv": 0,
                "version": version,
                "language": language,
                "timezone": timezone,
                "deviceModel": "Desktop",
                "cd": {"su": null, "sbu": null, "cu": null, "eu": null, "du": null, "tu": null},
            }), None, false)
        }) {
            if let Value::Object(_) = &resp.data {
                let result = Self::parse_splash_data(&resp.data);
                if !result.subjects.is_empty() {
                    self.cache_subjects(&result.subjects);
                    return result;
                }
            }
        }

        // Attempt 2: /user/v2/splash-login
        if let Ok(resp) = self.with_client(|c| {
            c.post(SPLASH_LOGIN, &serde_json::json!({
                "version": version,
                "pushToken": "",
                "timezone": timezone,
                "deviceType": crate::constants::default_device_type(),
                "osVersion": 10,
                "deviceModel": "Desktop",
                "pv": 24,
                "language": language,
            }), None, false)
        }) {
            if let Value::Object(_) = &resp.data {
                let result = Self::parse_splash_data(&resp.data);
                if !result.subjects.is_empty() {
                    self.cache_subjects(&result.subjects);
                    return result;
                } else {
                    let cached = self.get_cached_subjects();
                    if !cached.is_empty() {
                        let dl = resp.data.get("dl").cloned();
                        let merged = Self::apply_times_to_subjects(&cached, dl.as_ref());
                        return SubjectFetchResult {
                            subjects: merged,
                            today_total_ms: result.today_total_ms,
                            today_rest_ms: result.today_rest_ms,
                        };
                    }
                }
            }
        }

        // Attempt 3: cache only
        let cached = self.get_cached_subjects();
        if !cached.is_empty() {
            return SubjectFetchResult {
                subjects: cached,
                today_total_ms: 0,
                today_rest_ms: 0,
            };
        }

        SubjectFetchResult { subjects: vec![], today_total_ms: 0, today_rest_ms: 0 }
    }

    fn parse_splash_data(data: &Value) -> SubjectFetchResult {
        let raw_list = data
            .get("ss")
            .cloned()
            .or_else(|| data.get("p").and_then(|p| p.get("ss")).cloned());
        let dl = data.get("dl").and_then(|v| v.as_object()).cloned();

        let mut today_total_ms = 0i64;
        let mut today_rest_ms = 0i64;
        if let Some(dl) = &dl {
            let dl_value = Value::Object(dl.clone());
            if let Some(sm) = dl_value.get("sm").or_else(|| dl_value.get("tp")).and_then(|v| v.as_i64()) {
                today_total_ms = sm;
            }
            let rm = dl_value.get("rm")
                .or_else(|| dl_value.get("rt"))
                .or_else(|| dl_value.get("rest"))
                .or_else(|| dl_value.get("restMs"))
                .or_else(|| dl_value.get("rest_ms"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if rm > 0 && rm < 500_000_000 {
                today_rest_ms = rm * 1000;
            } else {
                today_rest_ms = rm;
            }
        }

        let mut subject_times_by_name: std::collections::HashMap<String, i64> = Default::default();
        let mut subject_times_by_id: std::collections::HashMap<i64, i64> = Default::default();

        if let Some(dl) = &dl {
            let dl_value = Value::Object(dl.clone());
            if let Some(ls) = dl_value.get("ls").and_then(|v| v.as_array()) {
                for item in ls {
                    let name = item.get("sb").and_then(|v| v.as_str()).unwrap_or("");
                    let id = item.get("id").and_then(|v| v.as_i64()).or_else(|| item.get("subjectId").and_then(|v| v.as_i64()));
                    let ms = item.get("sm").and_then(|v| v.as_i64()).unwrap_or(0);
                    if !name.is_empty() {
                        *subject_times_by_name.entry(name.to_string()).or_insert(0) += ms;
                    }
                    if let Some(id) = id {
                        *subject_times_by_id.entry(id).or_insert(0) += ms;
                    }
                }
            }
            if let Some(ss) = dl_value.get("ss").and_then(|v| v.as_array()) {
                for item in ss {
                    let id = item.get("id").and_then(|v| v.as_i64());
                    let name = item.get("tt").and_then(|v| v.as_str()).unwrap_or("");
                    let ms = item.get("sm").and_then(|v| v.as_i64()).unwrap_or(0);
                    if let Some(id) = id {
                        *subject_times_by_id.entry(id).or_insert(0) += ms;
                    }
                    if !name.is_empty() {
                        *subject_times_by_name.entry(name.to_string()).or_insert(0) += ms;
                    }
                }
            }
        }

        let mut subjects: Vec<SubjectModel> = Vec::new();
        if let Some(list) = raw_list.as_ref().and_then(|v| v.as_array()) {
            subjects = list
                .iter()
                .map(SubjectModel::from_json)
                .map(|s| {
                    let ms = subject_times_by_id.get(&s.id).copied()
                        .or_else(|| subject_times_by_name.get(&s.title).copied())
                        .unwrap_or(0);
                    s.copy_with(Some(ms))
                })
                .filter(|s| !s.is_deleted)
                .collect();
        }

        if today_total_ms == 0 && !subjects.is_empty() {
            today_total_ms = subjects.iter().map(|s| s.study_ms).sum();
        }

        SubjectFetchResult { subjects, today_total_ms, today_rest_ms }
    }

    fn apply_times_to_subjects(subjects: &[SubjectModel], dl: Option<&Value>) -> Vec<SubjectModel> {
        let Some(dl) = dl else { return subjects.to_vec(); };
        let mut times_by_id: std::collections::HashMap<i64, i64> = Default::default();
        let mut times_by_name: std::collections::HashMap<String, i64> = Default::default();

        if let Some(ls) = dl.get("ls").and_then(|v| v.as_array()) {
            for item in ls {
                let name = item.get("sb").and_then(|v| v.as_str()).unwrap_or("");
                let id = item.get("id").and_then(|v| v.as_i64()).or_else(|| item.get("subjectId").and_then(|v| v.as_i64()));
                let ms = item.get("sm").and_then(|v| v.as_i64()).unwrap_or(0);
                if !name.is_empty() { *times_by_name.entry(name.to_string()).or_insert(0) += ms; }
                if let Some(id) = id { *times_by_id.entry(id).or_insert(0) += ms; }
            }
        }
        if let Some(ss) = dl.get("ss").and_then(|v| v.as_array()) {
            for item in ss {
                let id = item.get("id").and_then(|v| v.as_i64());
                let name = item.get("tt").and_then(|v| v.as_str()).unwrap_or("");
                let ms = item.get("sm").and_then(|v| v.as_i64()).unwrap_or(0);
                if let Some(id) = id { *times_by_id.entry(id).or_insert(0) += ms; }
                if !name.is_empty() { *times_by_name.entry(name.to_string()).or_insert(0) += ms; }
            }
        }

        subjects
            .iter()
            .map(|s| {
                let ms = times_by_id.get(&s.id).copied()
                    .or_else(|| times_by_name.get(&s.title).copied())
                    .unwrap_or(0);
                s.copy_with(Some(ms))
            })
            .collect()
    }

    pub fn delete_subject(&self, subject_id: i64) -> bool {
        let body = serde_json::json!({ "id": subject_id });
        match self.with_client(|c| c.post("/user/subject/hard-delete", &body, None, false)) {
            Ok(resp) => resp.data.get("s").and_then(|v| v.as_bool()).unwrap_or(false),
            _ => false,
        }
    }

    pub fn update_subject(&self, subject: &SubjectModel) -> bool {
        let body = serde_json::json!({
            "id": subject.id,
            "title": subject.title,
            "color": subject.color_int,
            "is_archived": subject.is_archived,
        });
        match self.with_client(|c| c.post("/user/subject/edit", &body, None, false)) {
            Ok(resp) => resp.data.get("s").and_then(|v| v.as_bool()).unwrap_or(false),
            _ => false,
        }
    }

    pub fn archive_subject(&self, subject_id: i64, is_archived: bool) -> bool {
        let body = serde_json::json!({ "id": subject_id, "is_archived": is_archived, "new": true });
        match self.with_client(|c| c.post("/user/subject/archive/change", &body, None, false)) {
            Ok(resp) => resp.data.get("s").and_then(|v| v.as_bool()).unwrap_or(false),
            _ => false,
        }
    }

    pub fn reorder_subjects(&self, subject_ids: &[i64]) -> bool {
        let orders: Vec<Value> = subject_ids
            .iter()
            .enumerate()
            .map(|(i, id)| serde_json::json!({ "id": id, "order": (i + 1) as i64 }))
            .collect();
        let body = serde_json::json!({ "orders": orders });
        match self.with_client(|c| c.post("/user/subject/orders", &body, None, false)) {
            Ok(resp) => resp.data.get("s").and_then(|v| v.as_bool()).unwrap_or(false),
            _ => false,
        }
    }
}

impl SubjectsApi for SubjectRepository {
    fn fetch_subjects_data(
        &self,
        language: &str,
        timezone: &str,
        version: i64,
    ) -> SubjectFetchResult {
        SubjectRepository::fetch_subjects_data(self, language, timezone, version)
    }
}
