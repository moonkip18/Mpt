//! Port of `lib/data/models/*`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model_utils::{
    bool_opt, first_int, first_str, get_int, get_str, if_empty, int_value,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SubjectModel {
    pub id: i64,
    pub title: String,
    pub study_ms: i64,
    pub order: i64,
    pub color_int: i64,
    pub is_deleted: bool,
    pub is_archived: bool,
}

impl SubjectModel {
    pub fn from_json(json: &Value) -> Self {
        Self {
            id: get_int(json, "id").unwrap_or(0),
            title: if_empty(get_str(json, "tt"), "Sem nome"),
            study_ms: get_int(json, "sm").unwrap_or(0),
            order: get_int(json, "or").unwrap_or(100),
            color_int: get_int(json, "co").unwrap_or(4292557552),
            is_deleted: bool_opt(json.get("dl")).unwrap_or(false),
            is_archived: bool_opt(json.get("ia")).unwrap_or(false),
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "id": self.id, "tt": self.title, "sm": self.study_ms,
            "or": self.order, "co": self.color_int, "dl": self.is_deleted, "ia": self.is_archived,
        })
    }

    pub fn copy_with(&self, study_ms: Option<i64>) -> Self {
        Self { study_ms: study_ms.unwrap_or(self.study_ms), ..self.clone() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupModel {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub daily_goal_hours: i64,
    pub members_count: i64,
    pub max_capacity: i64,
    pub is_private: bool,
    pub leader_name: String,
    pub leader_user_id: i64,
    pub notice: Option<String>,
    pub is_cam_study: bool,
}

impl GroupModel {
    pub fn from_json(json: &Value) -> Self {
        Self {
            id: first_int(json, &["id", "groupID", "gd", "gid"]).unwrap_or(0),
            name: if_empty(first_str(json, &["t", "n", "name", "title"]), "Grupo"),
            category: if_empty(first_str(json, &["c", "category", "ct"]), "Geral"),
            daily_goal_hours: first_int(json, &["gt", "g", "dailyGoalHours"]).unwrap_or(8),
            members_count: first_int(json, &["jc", "personnel", "membersCount", "members_count"]).unwrap_or(1),
            max_capacity: first_int(json, &["mc", "mp", "maxPersonnel", "maxCapacity"]).unwrap_or(50),
            is_private: bool_opt(json.get("ip")).or_else(|| bool_opt(json.get("p"))).or_else(|| bool_opt(json.get("hp"))).unwrap_or(false),
            leader_name: first_str(json, &["on", "ln", "leader", "owner_name"]),
            leader_user_id: first_int(json, &["od", "ou", "oid", "leaderID", "uid", "leader_user_id"]).unwrap_or(0),
            notice: json.get("sn").or_else(|| json.get("nt")).or_else(|| json.get("notice"))
                .and_then(|v| v.as_str().map(String::from)),
            is_cam_study: bool_opt(json.get("cam")).or_else(|| bool_opt(json.get("isCam"))).unwrap_or(false),
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "id": self.id, "n": self.name, "c": self.category, "gt": self.daily_goal_hours,
            "jc": self.members_count, "mp": self.max_capacity, "ip": self.is_private,
            "on": self.leader_name, "ou": self.leader_user_id, "sn": self.notice, "cam": self.is_cam_study,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserModel {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub status_message: String,
    pub category_name: String,
    pub category_id: i64,
    pub studicon_id: i64,
    pub profile_photo_url: Option<String>,
    pub jwt_token: String,
    pub user_groups: Vec<GroupModel>,
    pub flames_balance: i64,
    pub owned_studicon_ids: Vec<i64>,
}

impl UserModel {
    pub fn from_json(json: &Value, token: &str) -> Self {
        let p = json.get("p").and_then(|v| v.as_object());
        let user_id = first_int(json, &["id", "ud", "userId"]).unwrap_or(0);
        let studicon_id = extract_studicon_id(p, Some(json));
        let has_custom_avatar = bool_opt(json.get("hasCustomAvatar")).unwrap_or(false);

        let groups_raw = json.get("gs").or_else(|| json.get("groups")).or_else(|| json.get("userGroups"));
        let user_groups: Vec<GroupModel> = groups_raw
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(GroupModel::from_json).collect())
            .unwrap_or_default();

        let scs_raw = json.get("scs").or_else(|| json.get("my")).or_else(|| json.get("ownedStudiconIds"));
        let mut owned_studicons: Vec<i64> = Vec::new();
        if let Some(arr) = scs_raw.and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(n) = item.as_i64() {
                    if n > 0 && !owned_studicons.contains(&n) { owned_studicons.push(n); }
                } else if let Some(obj) = item.as_object() {
                    let id = first_int(&Value::Object(obj.clone()), &["id", "sc", "sd", "studiconId"]).unwrap_or(0);
                    if id > 0 && !owned_studicons.contains(&id) { owned_studicons.push(id); }
                } else if let Some(s) = item.as_str() {
                    if let Ok(id) = s.parse::<i64>() {
                        if id > 0 && !owned_studicons.contains(&id) { owned_studicons.push(id); }
                    }
                }
            }
        }

        let mut parsed_name = get_str(json, "n");
        if parsed_name.is_empty() {
            parsed_name = get_str(json, "name");
        }
        if parsed_name.is_empty() {
            if let Some(profile) = p {
                parsed_name = ["n", "name", "nickname"]
                    .iter()
                    .find_map(|k| profile.get(*k).and_then(|v| v.as_str()).map(str::to_string))
                    .unwrap_or_default();
            }
        }

        Self {
            id: user_id,
            name: parsed_name,
            email: first_str(json, &["e", "email"]),
            status_message: first_str(json, &["stm", "statusMsg"]),
            category_name: if_empty(first_str(json, &["ct", "categoryName"]), "Geral"),
            category_id: first_int(json, &["ci", "categoryId"]).unwrap_or(0),
            studicon_id,
            profile_photo_url: if has_custom_avatar {
                Some(format!("https://alicdn.tgclab.com/user/profile/{user_id}.jpg"))
            } else { None },
            jwt_token: token.to_string(),
            user_groups,
            flames_balance: first_int(json, &["fl", "flames", "flameBalance"]).unwrap_or(100),
            owned_studicon_ids: owned_studicons,
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "id": self.id, "n": self.name, "e": self.email, "stm": self.status_message,
            "ct": self.category_name, "ci": self.category_id, "sd": self.studicon_id,
            "studiconId": self.studicon_id, "jwtToken": self.jwt_token, "fl": self.flames_balance,
            "gs": self.user_groups.iter().map(|g| g.to_json()).collect::<Vec<_>>(),
            "scs": self.owned_studicon_ids,
        })
    }

    pub fn copy_with(&self, studicon_id: Option<i64>, user_groups: Option<Vec<GroupModel>>) -> Self {
        Self {
            studicon_id: studicon_id.unwrap_or(self.studicon_id),
            user_groups: user_groups.unwrap_or_else(|| self.user_groups.clone()),
            ..self.clone()
        }
    }
}

fn extract_studicon_id(p: Option<&serde_json::Map<String, Value>>, root: Option<&Value>) -> i64 {
    let candidates = [
        p.and_then(|m| m.get("sd")), p.and_then(|m| m.get("csd")), p.and_then(|m| m.get("ssd")), p.and_then(|m| m.get("studiconId")),
        root.and_then(|v| v.get("sd")), root.and_then(|v| v.get("csd")), root.and_then(|v| v.get("ssd")), root.and_then(|v| v.get("studiconId")),
    ];
    for c in candidates {
        if let Some(v) = c {
            if let Some(val) = int_value(v) {
                if val > 0 { return val; }
            }
        }
    }
    -1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OfflineSessionModel {
    pub id: String,
    pub subject_id: i64,
    pub subject_title: String,
    pub start_at: i64,
    pub stop_at: i64,
    pub study_ms: i64,
    pub created_at: i64,
    pub retry_count: i64,
    pub last_error: Option<String>,
}

impl OfflineSessionModel {
    pub fn from_json(json: &Value) -> Self {
        Self {
            id: get_str(json, "id"),
            subject_id: get_int(json, "subjectId").unwrap_or(0),
            subject_title: get_str(json, "subjectTitle"),
            start_at: get_int(json, "startAt").unwrap_or(0),
            stop_at: get_int(json, "stopAt").unwrap_or(0),
            study_ms: get_int(json, "studyMs").unwrap_or(0),
            created_at: get_int(json, "createdAt").unwrap_or(0),
            retry_count: get_int(json, "retryCount").unwrap_or(0),
            last_error: json.get("lastError").and_then(|v| v.as_str().map(String::from)),
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "id": self.id, "subjectId": self.subject_id, "subjectTitle": self.subject_title,
            "startAt": self.start_at, "stopAt": self.stop_at, "studyMs": self.study_ms,
            "createdAt": self.created_at, "retryCount": self.retry_count, "lastError": self.last_error,
        })
    }

    pub fn copy_with(&self, retry_count: Option<i64>, last_error: Option<Option<String>>) -> Self {
        Self {
            retry_count: retry_count.unwrap_or(self.retry_count),
            last_error: last_error.unwrap_or_else(|| self.last_error.clone()),
            ..self.clone()
        }
    }
}
