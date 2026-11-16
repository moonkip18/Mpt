//! Port of `lib/core/api/*` (Dio client + JWT interceptor).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::constants::{BASE_URL, JWT_PREFIX, USER_AGENT};
use crate::store::Store;

pub const KEY_JWT_TOKEN: &str = "jwt_token";

#[derive(Debug)]
pub enum ApiError {
    Timeout,
    Network(String),
    Http(u16, String),
    Decode(String),
}

impl ApiError {
    pub fn is_timeout(&self) -> bool {
        matches!(self, ApiError::Timeout)
    }

    pub fn is_connection(&self) -> bool {
        matches!(self, ApiError::Network(_))
    }

    pub fn status_code(&self) -> Option<u16> {
        match self {
            ApiError::Http(code, _) => Some(*code),
            _ => None,
        }
    }

    pub fn response_data(&self) -> Option<&str> {
        match self {
            ApiError::Http(_, body) => Some(body),
            _ => None,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Timeout => write!(f, "connection timed out"),
            ApiError::Network(msg) => write!(f, "connection error: {msg}"),
            ApiError::Http(code, _) => write!(f, "HTTP {code}"),
            ApiError::Decode(msg) => write!(f, "decode error: {msg}"),
        }
    }
}

impl std::error::Error for ApiError {}

pub struct ApiResponse {
    pub status: Option<u16>,
    pub data: Value,
}

pub struct ApiClient {
    client: reqwest::blocking::Client,
    base_url: String,
    store: Arc<Mutex<Store>>,
}

impl ApiClient {
    pub fn new(store: Arc<Mutex<Store>>) -> Self {
        Self::with_base_url(store, BASE_URL)
    }

    pub fn with_base_url(store: Arc<Mutex<Store>>, base_url: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            store,
        }
    }

    fn resolve_url(&self, path: &str, absolute_base: Option<&str>) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            return path.to_string();
        }
        let base = absolute_base
            .map(str::to_string)
            .unwrap_or_else(|| self.base_url.clone());
        if path.starts_with('/') {
            format!("{base}{path}")
        } else {
            format!("{base}/{path}")
        }
    }

    fn token(&self) -> Option<String> {
        let store = self.store.lock().ok()?;
        store
            .secure_get(KEY_JWT_TOKEN)
            .filter(|t| !t.is_empty())
            .or_else(|| store.prefs_get_string(KEY_JWT_TOKEN).filter(|t| !t.is_empty()))
    }

    fn auth_header(&self) -> String {
        match self.token() {
            Some(token) => format!("{JWT_PREFIX}{token}"),
            None => "JWT".to_string(),
        }
    }

    fn build_request(&self, method: reqwest::Method, url: &str) -> reqwest::blocking::RequestBuilder {
        self.client
            .request(method, url)
            .header("User-Agent", USER_AGENT)
            .header("Content-Type", "application/json")
            .header("Authorization", self.auth_header())
    }

    pub fn get(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        absolute_base: Option<&str>,
    ) -> Result<ApiResponse, ApiError> {
        let url = self.resolve_url(path, absolute_base);
        let mut builder = self.build_request(reqwest::Method::GET, &url);
        if let Some(q) = query {
            builder = builder.query(q);
        }
        self.send(builder)
    }

    pub fn post(
        &self,
        path: &str,
        body: &Value,
        absolute_base: Option<&str>,
        is_form: bool,
    ) -> Result<ApiResponse, ApiError> {
        let url = self.resolve_url(path, absolute_base);
        let builder = if is_form {
            self.client
                .post(&url)
                .header("User-Agent", USER_AGENT)
                .header("Authorization", self.auth_header())
                .form(&serde_json::from_value::<std::collections::HashMap<String, String>>(body.clone()).unwrap_or_default())
        } else {
            self.build_request(reqwest::Method::POST, &url).json(body)
        };
        self.send(builder)
    }

    pub fn put(
        &self,
        path: &str,
        body: &Value,
        absolute_base: Option<&str>,
    ) -> Result<ApiResponse, ApiError> {
        let url = self.resolve_url(path, absolute_base);
        self.send(self.build_request(reqwest::Method::PUT, &url).json(body))
    }

    pub fn delete(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        body: Option<&Value>,
        absolute_base: Option<&str>,
    ) -> Result<ApiResponse, ApiError> {
        let url = self.resolve_url(path, absolute_base);
        let mut builder = self.build_request(reqwest::Method::DELETE, &url);
        if let Some(q) = query {
            builder = builder.query(q);
        }
        if let Some(b) = body {
            builder = builder.json(b);
        }
        self.send(builder)
    }

    fn send(&self, builder: reqwest::blocking::RequestBuilder) -> Result<ApiResponse, ApiError> {
        let response = builder.send().map_err(classify_send_error)?;
        let status = response.status().as_u16();
        let body_text = response.text().unwrap_or_default();
        let data = if body_text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&body_text).unwrap_or(Value::String(body_text.clone()))
        };
        // Dio validateStatus: < 500 is treated as a normal response.
        if status >= 500 {
            return Err(ApiError::Http(status, body_text));
        }
        Ok(ApiResponse {
            status: Some(status),
            data,
        })
    }
}

fn classify_send_error(err: reqwest::Error) -> ApiError {
    if err.is_timeout() {
        ApiError::Timeout
    } else if err.is_connect() {
        ApiError::Network(err.to_string())
    } else {
        ApiError::Network(err.to_string())
    }
}
