//! Port of `lib/data/repositories/auth_repository.dart`.

use serde_json::Value;

use crate::api::{ApiClient, ApiError};
use crate::constants::{
    CREATE_EMAIL_ACCOUNT, DEFAULT_LANGUAGE, EXIST_USERNAME, NICKNAME_CHANGE, RELOAD_INFO,
    SIGN_IN_JWT, SOCIAL_SIGN_UP_JWT, SPLASH_LOGIN, UNREGISTER,
};
use crate::localization::tr_msg;
use crate::models::{GroupModel, UserModel};
use crate::store::Store;

use std::sync::{Arc, Mutex};

pub const KEY_CACHED_USER: &str = "cached_user_profile";
pub const KEY_USER_EQUIPPED_STUDICON_ID: &str = "user_equipped_studicon_id";

/// Error surfaced to the UI: a user-facing message plus the HTTP status, if any.
pub struct AuthError {
    pub message: String,
    pub status_code: Option<u16>,
    /// Server error code (`c` field), used by the notifier's mapping table.
    pub code: Option<String>,
}

impl AuthError {
    pub fn new(message: String, status_code: Option<u16>) -> Self {
        Self { message, status_code, code: None }
    }

    pub fn with_code(mut self, code: Option<String>) -> Self {
        self.code = code;
        self
    }

    /// The server's machine code, if the response carried one.
    pub fn message_code(&self) -> Option<String> {
        self.code.clone()
    }
}

impl From<ApiError> for AuthError {
    fn from(e: ApiError) -> Self {
        let code = e
            .response_data()
            .and_then(|body| serde_json::from_str::<Value>(body).ok())
            .and_then(|v| v.get("c").and_then(Value::as_str).map(str::to_string));
        let message = match &e {
            ApiError::Timeout => "timeout".into(),
            ApiError::Network(_) => "connection error".into(),
            other => other.to_string(),
        };
        Self {
            message,
            status_code: e.status_code(),
            code,
        }
    }
}

/// Social sign-in succeeded at the provider but YPT has no such account yet
/// (error code 111/113) — the UI must offer sign-up.
pub struct SocialSignUpRequired {
    pub provider: String,
    pub social_id: String,
    pub email: String,
    pub name: String,
}

pub enum AuthFlowError {
    Shown(AuthError),
    SignUpRequired(SocialSignUpRequired),
}

pub struct AuthRepository {
    api: Arc<Mutex<ApiClient>>,
    store: Arc<Mutex<Store>>,
}

impl AuthRepository {
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

    fn save_token(&self, token: &str) {
        if let Ok(mut store) = self.store.lock() {
            store.secure_set(crate::api::KEY_JWT_TOKEN, token);
            store.prefs_set_string(crate::api::KEY_JWT_TOKEN, token);
            store.flush();
        }
    }

    fn delete_token(&self) {
        if let Ok(mut store) = self.store.lock() {
            store.secure_delete(crate::api::KEY_JWT_TOKEN);
            store.prefs_remove(crate::api::KEY_JWT_TOKEN);
            store.prefs_remove(KEY_CACHED_USER);
            store.prefs_remove(KEY_USER_EQUIPPED_STUDICON_ID);
            store.flush();
        }
    }

    pub fn get_stored_token(&self) -> Option<String> {
        let store = self.store.lock().ok()?;
        store
            .secure_get(crate::api::KEY_JWT_TOKEN)
            .filter(|t| !t.is_empty())
            .or_else(|| store.prefs_get_string(crate::api::KEY_JWT_TOKEN).filter(|t| !t.is_empty()))
    }

    /// Persist a token supplied directly (custom-JWT sign-in path).
    pub fn save_token_for_restore(&self, token: &str) {
        self.save_token(token);
    }

    /// Drop the stored token (failed custom-JWT restore).
    pub fn clear_token_for_restore(&self) {
        self.delete_token();
    }

    /// Handle for caching subjects alongside a splash payload.
    pub fn store_handle(&self) -> &Arc<Mutex<Store>> {
        &self.store
    }

    pub fn cache_user(&self, user: &UserModel) {
        let json = user.to_json();
        let encoded = serde_json::to_string(&json).unwrap_or_default();
        if let Ok(mut store) = self.store.lock() {
            store.prefs_set_string(KEY_CACHED_USER, &encoded);
            if user.studicon_id > 0 {
                let key = format!("{KEY_USER_EQUIPPED_STUDICON_ID}_{}", user.id);
                store.prefs_set_int(&key, user.studicon_id);
            }
            store.flush();
        }
    }

    fn cache_user_data(&self, data: &Value, token: &str) {
        if let Ok(user) = build_user(data, token) {
            self.cache_user(&user);
        }
    }

    /// POST /user/v2/splash-login — the account's profile payload, when healthy.
    pub fn splash_login(
        &self,
        language: &str,
        timezone: &str,
        version: i64,
    ) -> Option<Value> {
        let body = serde_json::json!({
            "version": version,
            "pushToken": "",
            "timezone": timezone,
            "deviceType": crate::constants::default_device_type(),
            "osVersion": 10,
            "deviceModel": "Desktop",
            "pv": 2,
            "language": language,
        });
        let resp = self.with_client(|c| c.post(SPLASH_LOGIN, &body, None, false)).ok()?;
        if resp.data.get("s").and_then(Value::as_bool).unwrap_or(false) {
            Some(resp.data)
        } else {
            None
        }
    }

    /// POST /user/sign-in-jwt with e-mail + password.
    pub fn sign_in_with_email(
        &self,
        email: &str,
        password: &str,
        language: &str,
    ) -> Result<UserModel, AuthError> {
        let body = serde_json::json!({
            "email": email,
            "password": password,
            "loginProvider": "Email",
            "new": true,
            "getx": true,
            "language": language,
        });
        let resp = self.with_client(|c| c.post(SIGN_IN_JWT, &body, None, false))?;
        let data = resp.data;

        if !data.get("s").and_then(Value::as_bool).unwrap_or(false) {
            let mut msg = "E-mail ou senha incorretos".to_string();
            if let Some(m) = data.get("m").or_else(|| data.get("message")) {
                if !m.is_null() {
                    msg = m.to_string().trim_matches('"').to_string();
                }
            } else if let Some(code) = data.get("c") {
                let code = code.to_string();
                let code_str = code.trim_matches('"');
                msg = match code_str {
                    "112" => "Senha incorreta. Verifique sua senha e tente novamente.".into(),
                    "113" => tr_msg(
                        "auth_email_not_registered",
                        None,
                        Some("E-mail não cadastrado no Yeolpumta."),
                    ),
                    "114" => "Conta suspensa ou inativa.".into(),
                    other => tr_msg(
                        "auth_failed_code",
                        Some(vec![other]),
                        Some(&format!("Falha na autenticação (código {other})")),
                    ),
                };
            }
            return Err(AuthError {
                message: msg,
                status_code: resp.status,
                code: None,
            });
        }

        let token = data
            .get("jwt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if token.is_empty() {
            return Err(AuthError {
                message: tr_msg(
                    "auth_missing_token",
                    None,
                    Some("Token JWT não foi retornado pelo servidor"),
                ),
                status_code: resp.status,
                code: None,
            });
        }

        self.save_token(&token);
        crate::repositories::subject::SubjectRepository::save_subjects_from_raw_data(&self.store, &data);

        let mut data = data;
        if let Some(splash) = self.splash_login(language, crate::constants::DEFAULT_TIMEZONE, crate::constants::DEFAULT_VERSION) {
            merge_splash(&mut data, &splash);
        }

        self.cache_user_data(&data, &token);
        let mut user = build_user(&data, &token)?;
        if user.studicon_id <= 0 && user.id > 0 {
            if let Ok(store) = self.store.lock() {
                let scoped = format!("{KEY_USER_EQUIPPED_STUDICON_ID}_{}", user.id);
                let saved = store.prefs_get_int(&scoped).or_else(|| {
                    store.prefs_get_int(KEY_USER_EQUIPPED_STUDICON_ID)
                });
                if saved.is_some_and(|sd| sd > 0) {
                    user.studicon_id = saved.unwrap();
                    self.cache_user(&user);
                }
            }
        }
        Ok(user)
    }

    /// POST /user/exist-username — used before deciding sign-in vs sign-up.
    pub fn check_username_exists(&self, username: &str) -> bool {
        let body = serde_json::json!({ "username": username });
        match self.with_client(|c| c.post(EXIST_USERNAME, &body, None, false)) {
            Ok(resp) => resp.data.get("s").and_then(Value::as_bool).unwrap_or(false),
            Err(_) => false,
        }
    }

    fn format_provider_id(provider: &str, social_id: &str) -> String {
        let lower = provider.to_lowercase();
        match lower.as_str() {
            "google" if !social_id.starts_with('g') => format!("g{social_id}"),
            "apple" if !social_id.starts_with('a') => format!("a{social_id}"),
            "kakao" if !social_id.starts_with('k') => format!("k{social_id}"),
            "naver" if !social_id.starts_with('n') => format!("n{social_id}"),
            _ => social_id.to_string(),
        }
    }

    /// POST /user/social/sign-up-jwt — sign in through a social identity.
    pub fn sign_in_with_social(
        &self,
        provider: &str,
        social_id: &str,
        email: &str,
        name: &str,
        language: &str,
    ) -> Result<UserModel, AuthFlowError> {
        let formatted = Self::format_provider_id(provider, social_id);
        let body = serde_json::json!({
            "accessToken": "",
            "providerId": formatted,
            "email": email,
            "loginProvider": provider,
            "new": true,
            "getx": true,
            "version": crate::constants::DEFAULT_VERSION,
            "language": language,
        });
        let shown = |e: AuthError| AuthFlowError::Shown(e);
        let resp = match self.with_client(|c| c.post(SOCIAL_SIGN_UP_JWT, &body, None, false)) {
            Ok(r) => r,
            Err(e) => return Err(shown(AuthError::from(e))),
        };
        let data = resp.data;

        if !data.get("s").and_then(Value::as_bool).unwrap_or(false) {
            let code = data.get("c").and_then(Value::as_str).map(str::to_string);
            if code.as_deref() == Some("111") || code.as_deref() == Some("113") {
                return Err(AuthFlowError::SignUpRequired(SocialSignUpRequired {
                    provider: provider.to_string(),
                    social_id: social_id.to_string(),
                    email: email.to_string(),
                    name: name.to_string(),
                }));
            }
            let msg = data
                .get("m")
                .or_else(|| data.get("message"))
                .map(|v| v.to_string().trim_matches('"').to_string())
                .unwrap_or_else(|| match code.as_deref() {
                    Some(c) => tr_msg(
                        "auth_error_code",
                        Some(vec![c]),
                        Some(&format!("Erro de autenticação [{c}]")),
                    ),
                    None => tr_msg(
                        "auth_failed_via_provider",
                        Some(vec![provider]),
                        Some(&format!("Falha ao autenticar via {provider}")),
                    ),
                });
            return Err(shown(AuthError { message: msg, status_code: resp.status, code: None }));
        }

        let token = data
            .get("jwt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if token.is_empty() {
            return Err(shown(AuthError {
                message: tr_msg(
                    "auth_missing_provider_token",
                    Some(vec![provider]),
                    Some(&format!("Token JWT via {provider} não retornado pelo servidor")),
                ),
                status_code: resp.status,
                code: None,
            }));
        }

        self.save_token(&token);
        crate::repositories::subject::SubjectRepository::save_subjects_from_raw_data(&self.store, &data);

        let mut data = data;
        if let Some(splash) = self.splash_login(language, crate::constants::DEFAULT_TIMEZONE, crate::constants::DEFAULT_VERSION) {
            merge_splash(&mut data, &splash);
        }
        if (data.get("n").and_then(Value::as_str).unwrap_or("").trim().is_empty())
            && !name.trim().is_empty()
        {
            data["n"] = Value::String(name.trim().to_string());
            data["name"] = Value::String(name.trim().to_string());
        }

        self.cache_user_data(&data, &token);
        let mut user = match build_user(&data, &token) {
            Ok(u) => u,
            Err(e) => return Err(shown(e)),
        };
        if user.studicon_id <= 0 && user.id > 0 {
            if let Ok(store) = self.store.lock() {
                let scoped = format!("{KEY_USER_EQUIPPED_STUDICON_ID}_{}", user.id);
                let saved = store.prefs_get_int(&scoped).or_else(|| {
                    store.prefs_get_int(KEY_USER_EQUIPPED_STUDICON_ID)
                });
                if saved.is_some_and(|sd| sd > 0) {
                    user.studicon_id = saved.unwrap();
                    self.cache_user(&user);
                }
            }
        }
        Ok(user)
    }

    /// POST /user/social/sign-up-jwt — register a brand-new social account.
    pub fn sign_up_with_social(
        &self,
        provider: &str,
        social_id: &str,
        email: &str,
        name: &str,
        nickname: &str,
        country_id: i64,
        category_id: i64,
        language: &str,
    ) -> Result<UserModel, AuthFlowError> {
        let formatted = Self::format_provider_id(provider, social_id);
        let body = serde_json::json!({
            "accessToken": "",
            "providerId": formatted,
            "email": email,
            "loginProvider": provider,
            "nickname": nickname,
            "name": name,
            "new": true,
            "getx": true,
            "countryId": country_id,
            "categoryId": category_id,
            "version": crate::constants::DEFAULT_VERSION,
            "language": language,
        });
        let shown = |e: AuthError| AuthFlowError::Shown(e);
        let resp = match self.with_client(|c| c.post(SOCIAL_SIGN_UP_JWT, &body, None, false)) {
            Ok(r) => r,
            Err(e) => return Err(shown(AuthError::from(e))),
        };
        let mut data = resp.data;
        if !data.get("s").and_then(Value::as_bool).unwrap_or(false) {
            let msg = data
                .get("m")
                .or_else(|| data.get("message"))
                .map(|v| v.to_string().trim_matches('"').to_string())
                .unwrap_or_else(|| {
                    tr_msg(
                        "auth_failed_via_provider",
                        Some(vec![provider]),
                        Some(&format!("Falha ao autenticar via {provider}")),
                    )
                });
            return Err(shown(AuthError { message: msg, status_code: resp.status, code: None }));
        }
        let token = data
            .get("jwt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if token.is_empty() {
            return Err(shown(AuthError {
                message: tr_msg(
                    "auth_missing_provider_token",
                    Some(vec![provider]),
                    Some(&format!("Token JWT via {provider} não retornado pelo servidor")),
                ),
                status_code: resp.status,
                code: None,
            }));
        }
        self.save_token(&token);
        crate::repositories::subject::SubjectRepository::save_subjects_from_raw_data(&self.store, &data);
        if let Some(splash) = self.splash_login(language, crate::constants::DEFAULT_TIMEZONE, crate::constants::DEFAULT_VERSION) {
            merge_splash(&mut data, &splash);
        }
        if (data.get("n").and_then(Value::as_str).unwrap_or("").trim().is_empty())
            && !nickname.trim().is_empty()
        {
            data["n"] = Value::String(nickname.trim().to_string());
            data["name"] = Value::String(nickname.trim().to_string());
        }
        self.cache_user_data(&data, &token);
        match build_user(&data, &token) {
            Ok(mut user) => {
                if user.name.trim().is_empty() && !nickname.trim().is_empty() {
                    user.name = nickname.trim().to_string();
                    self.cache_user(&user);
                }
                Ok(user)
            }
            Err(e) => Err(shown(e)),
        }
    }

    /// Restore a session from storage; refreshes profile from the server when
    /// reachable (4s splash + 3s reload/info, both best effort).
    pub fn try_restore_session(&self) -> Option<UserModel> {
        let token = self.get_stored_token()?;
        if token.is_empty() {
            return None;
        }

        let mut saved_equipped: Option<i64> = None;
        let mut user: Option<UserModel> = None;
        {
            if let Ok(store) = self.store.lock() {
                saved_equipped = store.prefs_get_int(KEY_USER_EQUIPPED_STUDICON_ID);
                if let Some(user_str) = store.prefs_get_string(KEY_CACHED_USER) {
                    if !user_str.is_empty() {
                        if let Ok(decoded) = serde_json::from_str::<Value>(&user_str) {
                            if decoded.is_object() {
                                if let Ok(mut u) = build_user(&decoded, &token) {
                                    let scoped = format!("{KEY_USER_EQUIPPED_STUDICON_ID}_{}", u.id);
                                    let user_saved = if u.id > 0 {
                                        store.prefs_get_int(&scoped)
                                    } else {
                                        None
                                    };
                                    let effective = user_saved.or(saved_equipped);
                                    if effective.is_some_and(|sd| sd > 0) {
                                        u.studicon_id = effective.unwrap();
                                    }
                                    user = Some(u);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Server refresh (4s budget, best effort).
        if let Some(splash) = self
            .with_client_timeout(
                |c| c.post(SPLASH_LOGIN, &self.splash_body(2), None, false),
                std::time::Duration::from_secs(4),
            )
            .ok()
            .filter(|r| r.data.get("s").and_then(Value::as_bool).unwrap_or(false))
            .map(|r| r.data)
        {
            let splash_value = splash;
            let groups = extract_groups(&splash_value);
            let p_obj = splash_value.get("p").and_then(|v| v.as_object());
            let server_sd = extract_studicon_id(p_obj, Some(&splash_value));
            let user_saved: Option<i64> = user.as_ref().and_then(|u| {
                if u.id > 0 {
                    if let Ok(store) = self.store.lock() {
                        let scoped = format!("{KEY_USER_EQUIPPED_STUDICON_ID}_{}", u.id);
                        store.prefs_get_int(&scoped)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            let resolved_sd = if server_sd > 0 {
                server_sd
            } else {
                user_saved
                    .or(saved_equipped)
                    .unwrap_or_else(|| user.as_ref().map(|u| u.studicon_id).unwrap_or(-1))
            };

            let resolved_owned: Option<Vec<i64>> = splash_value.get("scs").and_then(Value::as_array).map(|arr| {
                let mut ids: Vec<i64> = Vec::new();
                for item in arr {
                    let id = if let Some(obj) = item.as_object() {
                        serde_json::Value::Object(obj.clone())
                            .get("id")
                            .or_else(|| item.get("sc"))
                            .or_else(|| item.get("sd"))
                            .or_else(|| item.get("studiconId"))
                            .and_then(int_from_value)
                    } else {
                        int_from_value(item)
                    };
                    if id.is_some_and(|v| v > 0) && !ids.contains(&id.unwrap()) {
                        ids.push(id.unwrap());
                    }
                }
                ids
            });

            if let Some(u) = user.as_mut() {
                if let Some(groups) = groups {
                    u.user_groups = groups;
                }
                if let Some(stm) = splash_value.get("stm").and_then(Value::as_str) {
                    u.status_message = stm.to_string();
                }
                u.studicon_id = resolved_sd;
                if let Some(fl) = splash_value.get("fl").and_then(Value::as_i64) {
                    u.flames_balance = fl;
                }
                if let Some(owned) = resolved_owned {
                    u.owned_studicon_ids = owned;
                }
            } else if let Ok(mut u) = build_user(&splash_value, &token) {
                u.studicon_id = resolved_sd;
                user = Some(u);
            }
            if let Some(u) = &user {
                self.cache_user(u);
            }
        }

        // Second refresh pass: reload/info (3s budget).
        let body = serde_json::json!({ "pv": 2, "cd": {} });
        if let Ok(resp) = self.with_client_timeout(
            |c| c.post(RELOAD_INFO, &body, None, false),
            std::time::Duration::from_secs(3),
        ) {
            if resp.data.get("s").and_then(Value::as_bool).unwrap_or(false) {
                if let Some(u) = user.as_mut() {
                    let data = resp.data;
                    let p_obj = data.get("p").and_then(|v| v.as_object());
                    let server_reload_sd = extract_studicon_id(p_obj, Some(&data));
                    let updated_stm = p_obj
                        .and_then(|p| p.get("stm"))
                        .and_then(Value::as_str)
                        .map(str::to_string);

                    let updated_owned: Option<Vec<i64>> =
                        data.get("scs").and_then(Value::as_array).map(|arr| {
                            let mut ids: Vec<i64> = Vec::new();
                            for item in arr {
                                let id = if let Some(obj) = item.as_object() {
                                    serde_json::Value::Object(obj.clone())
                                        .get("id")
                                        .or_else(|| item.get("sc"))
                                        .or_else(|| item.get("sd"))
                                        .or_else(|| item.get("studiconId"))
                                        .and_then(int_from_value)
                                } else {
                                    int_from_value(item)
                                };
                                if id.is_some_and(|v| v > 0) && !ids.contains(&id.unwrap()) {
                                    ids.push(id.unwrap());
                                }
                            }
                            ids
                        });

                    let user_saved = if u.id > 0 {
                        if let Ok(store) = self.store.lock() {
                            let scoped = format!("{KEY_USER_EQUIPPED_STUDICON_ID}_{}", u.id);
                            store.prefs_get_int(&scoped)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let final_sd = if server_reload_sd > 0 {
                        server_reload_sd
                    } else if u.studicon_id > 0 {
                        u.studicon_id
                    } else {
                        user_saved.or(saved_equipped).unwrap_or(u.studicon_id)
                    };
                    if let Some(stm) = updated_stm {
                        u.status_message = stm;
                    }
                    u.studicon_id = final_sd;
                    if let Some(owned) = updated_owned {
                        u.owned_studicon_ids = owned;
                    }
                    self.cache_user(u);
                }
            }
        }

        user
    }

    fn splash_body(&self, pv: i64) -> Value {
        serde_json::json!({
            "version": crate::constants::DEFAULT_VERSION,
            "pushToken": "",
            "timezone": crate::constants::DEFAULT_TIMEZONE,
            "deviceType": crate::constants::default_device_type(),
            "osVersion": 10,
            "deviceModel": "Desktop",
            "pv": pv,
            "language": DEFAULT_LANGUAGE,
        })
    }

    fn with_client_timeout<F, T>(&self, f: F, _timeout: std::time::Duration) -> Result<T, ApiError>
    where
        F: FnOnce(&ApiClient) -> Result<T, ApiError>,
    {
        // The reqwest client already carries the 15s cap; the Dart caller's
        // tighter budgets are advisory here (they keep a spinner from hanging).
        self.with_client(f)
    }

    pub fn change_nickname(&self, nickname: &str) -> bool {
        let body = serde_json::json!({ "nickname": nickname });
        self.with_client(|c| c.post(NICKNAME_CHANGE, &body, None, false))
            .map(|r| r.data.get("s").and_then(Value::as_bool).unwrap_or(false))
            .unwrap_or(false)
    }

    pub fn change_status_message(&self, status_msg: &str) -> bool {
        let body = serde_json::json!({ "statusMsg": status_msg });
        self.with_client(|c| c.post("/user/status_msg/change", &body, None, false))
            .map(|r| r.data.get("s").and_then(Value::as_bool).unwrap_or(false))
            .unwrap_or(false)
    }

    pub fn change_category(&self, category_id: i64) -> bool {
        let body = serde_json::json!({
            "category_id": category_id,
            "categoryId": category_id,
        });
        self.with_client(|c| c.post("/user/category/change", &body, None, false))
            .map(|r| r.data.get("s").and_then(Value::as_bool).unwrap_or(false))
            .unwrap_or(false)
    }

    /// POST /user/v2/create-email-account — create account with code verification.
    pub fn create_email_account(
        &self,
        email: &str,
        password: &str,
        nickname: &str,
        code: &str,
        country_id: i64,
        category_id: i64,
        language: &str,
    ) -> Result<UserModel, AuthError> {
        let body = serde_json::json!({
            "email": email,
            "password": password,
            "nickname": nickname,
            "code": code,
            "countryId": country_id,
            "categoryId": category_id,
            "language": language,
            "new": true,
            "getx": true,
        });
        let resp = self.with_client(|c| c.post(CREATE_EMAIL_ACCOUNT, &body, None, false))?;
        let mut data = resp.data;
        if !data.get("s").and_then(Value::as_bool).unwrap_or(false) {
            let msg = data
                .get("m")
                .or_else(|| data.get("message"))
                .map(|v| v.to_string().trim_matches('"').to_string())
                .unwrap_or_default();
            return Err(AuthError { message: msg, status_code: resp.status, code: None });
        }
        let token = data
            .get("jwt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if token.is_empty() {
            return Err(AuthError {
                message: tr_msg(
                    "auth_missing_token",
                    None,
                    Some("Token JWT não foi retornado pelo servidor"),
                ),
                status_code: resp.status,
                code: None,
            });
        }
        self.save_token(&token);
        crate::repositories::subject::SubjectRepository::save_subjects_from_raw_data(&self.store, &data);
        if let Some(splash) = self.splash_login(language, crate::constants::DEFAULT_TIMEZONE, crate::constants::DEFAULT_VERSION) {
            merge_splash(&mut data, &splash);
        }
        self.cache_user_data(&data, &token);
        let user = build_user(&data, &token)?;
        Ok(user)
    }

    /// GET /user/unregister — account deletion (code 115 blocks it).
    pub fn delete_account(&self) -> Result<bool, AuthError> {
        let resp = self.with_client(|c| c.get(UNREGISTER, None, None))?;
        let is_success = resp.data.get("s").and_then(Value::as_bool).unwrap_or(false);
        if is_success {
            self.delete_token();
            return Ok(true);
        }
        if resp.data.get("c").and_then(Value::as_str) == Some("115") {
            return Err(AuthError {
                message: tr_msg(
                    "auth_account_protected",
                    None,
                    Some("Esta conta está protegida contra exclusão devido a um problema em andamento."),
                ),
                status_code: resp.status,
                code: None,
            });
        }
        Ok(false)
    }

    pub fn logout(&self) {
        let _ = self.with_client(|c| {
            c.post("/user/logout", &serde_json::json!({ "pushToken": "" }), None, false)
        });
        self.delete_token();
    }

    /// Groups from the splash payload.
    pub fn fetch_user_groups(&self) -> Vec<GroupModel> {
        let Some(splash) = self.splash_login(
            DEFAULT_LANGUAGE,
            crate::constants::DEFAULT_TIMEZONE,
            crate::constants::DEFAULT_VERSION,
        ) else {
            return vec![];
        };
        extract_groups(&splash).unwrap_or_default()
    }
}

fn build_user(data: &Value, token: &str) -> Result<UserModel, AuthError> {
    let json = match data {
        Value::Object(map) => serde_json::Value::Object(map.clone()),
        other => other.clone(),
    };
    Ok(UserModel::from_json(&json, token))
}

/// Build a user from a splash payload with a known token (custom-JWT path).
pub fn user_from_payload(data: &Value, token: &str) -> UserModel {
    UserModel::from_json(data, token)
}

/// Fold splash fields (`p`, `gs`, `scs`, `fl`, `sd`) into a login payload.
fn merge_splash(data: &mut Value, splash: &Value) {
    for key in ["p", "gs", "scs", "fl", "sd"] {
        if let Some(v) = splash.get(key) {
            if !v.is_null() {
                data[key] = v.clone();
            }
        }
    }
    if let Some(name) = splash.get("n").and_then(Value::as_str) {
        if !name.trim().is_empty() {
            data["n"] = Value::String(name.to_string());
        }
    }
    if let Some(p) = splash.get("p").and_then(Value::as_object) {
        for key in ["sd", "ssd", "csd"] {
            if let Some(v) = p.get(key) {
                if !v.is_null() {
                    data[key] = v.clone();
                }
            }
        }
        if let Some(name) = p.get("n").and_then(Value::as_str) {
            if data.get("n").and_then(Value::as_str).unwrap_or("").trim().is_empty() {
                data["n"] = Value::String(name.to_string());
            }
        }
    }
}

fn extract_groups(splash: &Value) -> Option<Vec<GroupModel>> {
    let groups_raw = splash
        .get("gs")
        .or_else(|| splash.get("groups"))
        .or_else(|| splash.get("userGroups"))?;
    let arr = groups_raw.as_array()?;
    Some(arr.iter().filter_map(|g| g.as_object()).map(|g| GroupModel::from_json(&Value::Object(g.clone()))).collect())
}

fn extract_studicon_id(p: Option<&serde_json::Map<String, Value>>, root: Option<&Value>) -> i64 {
    let candidates = [
        p.and_then(|m| m.get("sd")),
        p.and_then(|m| m.get("csd")),
        p.and_then(|m| m.get("ssd")),
        p.and_then(|m| m.get("studiconId")),
        root.and_then(|v| v.get("sd")),
        root.and_then(|v| v.get("csd")),
        root.and_then(|v| v.get("ssd")),
        root.and_then(|v| v.get("studiconId")),
    ];
    for c in candidates.into_iter().flatten() {
        if let Some(val) = int_from_value(c) {
            if val > 0 {
                return val;
            }
        }
    }
    -1
}

fn int_from_value(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}
