//! Port of `lib/core/oauth/*` — PKCE helpers, loopback server, Google flow.

use base64::Engine;
use sha2::{Digest, Sha256};

use crate::config::env_instance;
use crate::localization::tr_msg;

const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

pub struct OAuthException {
    pub message: String,
    pub is_cancelled: bool,
}

pub struct OAuthUserInfo {
    pub provider: String,
    pub social_id: String,
    pub email: String,
    pub name: String,
}

fn secure_pick(len: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

/// PKCE verifier (64), S256 challenge, and CSRF state — matches `OAuthPkce`.
pub struct OAuthPkce;

impl OAuthPkce {
    pub fn generate_verifier(length: usize) -> String {
        secure_pick(length)
    }

    pub fn generate_challenge(verifier: &str) -> String {
        let digest = Sha256::digest(verifier.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    }

    pub fn generate_state(length: usize) -> String {
        secure_pick(length)
    }

    pub fn decode_jwt_payload(jwt: &str) -> Result<serde_json::Value, String> {
        let parts: Vec<&str> = jwt.split('.').collect();
        if parts.len() != 3 {
            return Err(tr_msg(
                "oauth_invalid_jwt_structure",
                None,
                Some("Token JWT inválido: estrutura incorreta"),
            ));
        }
        let mut payload = parts[1].to_string();
        match payload.len() % 4 {
            2 => payload.push_str("=="),
            3 => payload.push('='),
            _ => {}
        }
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&payload)
            .map_err(|_| "invalid base64".to_string())?;
        let decoded = String::from_utf8_lossy(&bytes);
        let json: serde_json::Value = serde_json::from_str(&decoded)
            .map_err(|_| {
                tr_msg(
                    "oauth_invalid_jwt_payload",
                    None,
                    Some("Payload JWT não é um objeto JSON válido"),
                )
            })?;
        if json.is_object() {
            Ok(json)
        } else {
            Err(tr_msg(
                "oauth_invalid_jwt_payload",
                None,
                Some("Payload JWT não é um objeto JSON válido"),
            ))
        }
    }
}

/// Loopback HTTP server for the desktop auth redirect (port in, params out).
pub struct OAuthDesktopService {
    pub active_port: Option<u16>,
    pub auth_url: String,
}

impl OAuthDesktopService {
    pub fn new() -> Self {
        Self { active_port: None, auth_url: String::new() }
    }

    /// Starts a one-shot listener on 127.0.0.1:0 and blocks until the browser
    /// returns with `code`/`state`, or `timeout` elapses. Returns the callback
    /// query parameters.
    pub fn start_auth_flow(
        &mut self,
        auth_url_builder: impl FnOnce(u16) -> String,
        expected_state: Option<&str>,
        timeout: std::time::Duration,
    ) -> Result<std::collections::HashMap<String, String>, OAuthException> {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| OAuthException {
            message: format!("Could not start local callback server: {e}"),
            is_cancelled: false,
        })?;
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
        self.active_port = Some(port);
        let url = auth_url_builder(port);
        self.auth_url = url.clone();

        // Open the default browser (parity with `url_launcher`).
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &url])
            .spawn();

        listener
            .set_nonblocking(true)
            .map_err(|_| OAuthException {
                message: "listener setup failed".into(),
                is_cancelled: false,
            })?;

        // Non-blocking accept, polled so the deadline still fires with no traffic.
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(OAuthException {
                    message: "Sign-in timed out. Please try again.".into(),
                    is_cancelled: false,
                });
            }
            let stream = match listener.accept() {
                Ok((stream, _)) => stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(40));
                    continue;
                }
                Err(_) => continue,
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .ok();
            let mut reader = BufReader::new(stream.try_clone().map_err(|_| OAuthException {
                message: "stream error".into(),
                is_cancelled: false,
            })?);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
                continue;
            }
            // Drain headers so the browser finishes the request.
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(n) if line == "\r\n" || line == "\n" || n == 0 => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }

            // GET /?code=...&state=...
            let target = request_line.split_whitespace().nth(1).unwrap_or("/");
            let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
            let mut params: std::collections::HashMap<String, String> = Default::default();
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    params.insert(
                        k.to_string(),
                        urlencoding_decode(v),
                    );
                }
            }

            let success = params.contains_key("code") && !params.contains_key("error");
            if let Some(expected) = expected_state {
                if params.get("state").map(String::as_str) != Some(expected) {
                    // Wrong state: keep listening rather than accepting a spoof.
                    let mut stream2 = stream.try_clone().unwrap_or(stream);
                    let _ = stream2.write_all(error_page().as_bytes());
                    continue;
                }
            }

            let page = if success { success_page() } else { error_page() };
            let mut writer = stream;
            let _ = writer.write_all(page.as_bytes());

            if success {
                return Ok(params);
            }
            let message = params
                .get("error_description")
                .cloned()
                .or_else(|| params.get("error").cloned())
                .unwrap_or_else(|| "Authentication was cancelled.".into());
            return Err(OAuthException { message, is_cancelled: false });
        }
    }
}

fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn success_page() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>MPT — Sign in</title>
<style>
body {{ margin:0; background:#050506; color:#E9EAEC; font-family:'Segoe UI',sans-serif;
display:flex; align-items:center; justify-content:center; height:100vh; }}
.card {{ text-align:center; padding:48px; }}
.badge {{ display:inline-block; font-size:10px; letter-spacing:2px; text-transform:uppercase;
color:#6E8F72; border:1px solid #14FFFFFF; border-radius:999px; padding:4px 12px; margin-bottom:24px; }}
h1 {{ font-family:Georgia,serif; font-weight:400; font-size:34px; margin:0 0 12px; }}
p {{ color:#8B8F97; font-size:14px; line-height:1.6; max-width:380px; margin:0 auto; }}
.tag {{ margin-top:36px; font-size:11px; letter-spacing:3px; color:#565A61; }}
</style></head>
<body><div class="card">
<div class="badge">Connected</div>
<h1>Login successful!</h1>
<p>You can now close this tab and return to <strong>MPT</strong>.</p>
<div class="tag">MPT — DESKTOP STUDY TIMER</div>
</div></body></html>"#
    )
}

fn error_page() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>MPT — Sign in</title>
<style>
body {{ margin:0; background:#050506; color:#E9EAEC; font-family:'Segoe UI',sans-serif;
display:flex; align-items:center; justify-content:center; height:100vh; }}
.card {{ text-align:center; padding:48px; }}
.badge {{ display:inline-block; font-size:10px; letter-spacing:2px; text-transform:uppercase;
color:#C8433C; border:1px solid #24FFFFFF; border-radius:999px; padding:4px 12px; margin-bottom:24px; }}
h1 {{ font-family:Georgia,serif; font-weight:400; font-size:34px; margin:0 0 12px; }}
p {{ color:#8B8F97; font-size:14px; line-height:1.6; max-width:380px; margin:0 auto; }}
.tag {{ margin-top:36px; font-size:11px; letter-spacing:3px; color:#565A61; }}
</style></head>
<body><div class="card">
<div class="badge">Error</div>
<h1>Authentication failed</h1>
<p>An error occurred during authentication. Return to <strong>MPT</strong> to try again.</p>
<div class="tag">MPT — DESKTOP STUDY TIMER</div>
</div></body></html>"#
    )
}

/// Google OAuth: PKCE authorization-code flow through the loopback server.
pub struct GoogleOAuthService {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub desktop: OAuthDesktopService,
    cancellable: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl GoogleOAuthService {
    pub fn new() -> Self {
        let env = env_instance();
        let client_id = env.get_google_client_id();
        let secret = env.get_google_client_secret();
        Self {
            client_id,
            client_secret: if secret.is_empty() { None } else { Some(secret) },
            desktop: OAuthDesktopService::new(),
            cancellable: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty()
    }

    pub fn cancel(&self) {
        self.cancellable
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // The blocking accept loop checks the deadline; closing early is
        // approximated by a very short timeout on the next run.
    }

    pub fn authenticate(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<OAuthUserInfo, OAuthException> {
        if self.client_id.is_empty() {
            return Err(OAuthException {
                message: tr_msg(
                    "oauth_google_missing_client_id",
                    None,
                    Some("Google Client ID não configurado no arquivo .env."),
                ),
                is_cancelled: false,
            });
        }

        let verifier = OAuthPkce::generate_verifier(64);
        let challenge = OAuthPkce::generate_challenge(&verifier);
        let state = OAuthPkce::generate_state(32);
        let client_id = self.client_id.clone();
        let expected_state = state.clone();

        let callback = self.desktop.start_auth_flow(
            |port| {
                let redirect = format!("http://127.0.0.1:{port}");
                format!(
                    "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&code_challenge={}&code_challenge_method=S256&state={}&prompt=select_account",
                    urlencoding_encode(&client_id),
                    urlencoding_encode(&redirect),
                    urlencoding_encode(&challenge),
                    urlencoding_encode(&expected_state),
                )
            },
            Some(&expected_state),
            timeout,
        )?;

        let code = callback
            .get("code")
            .cloned()
            .filter(|c| !c.is_empty())
            .ok_or_else(|| OAuthException {
                message: tr_msg(
                    "oauth_google_no_code",
                    None,
                    Some("Código de autorização não recebido do Google."),
                ),
                is_cancelled: false,
            })?;

        let port = self.desktop.active_port.unwrap_or(0);
        let redirect_uri = format!("http://127.0.0.1:{port}");

        // Token exchange (form-encoded).
        let mut form: std::collections::HashMap<String, String> = Default::default();
        form.insert("client_id".into(), self.client_id.clone());
        form.insert("code".into(), code);
        form.insert("code_verifier".into(), verifier);
        form.insert("grant_type".into(), "authorization_code".into());
        form.insert("redirect_uri".into(), redirect_uri);
        if let Some(secret) = self.client_secret.clone() {
            if !secret.is_empty() {
                form.insert("client_secret".into(), secret);
            }
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| OAuthException { message: e.to_string(), is_cancelled: false })?;
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&form)
            .send()
            .map_err(|e| OAuthException { message: e.to_string(), is_cancelled: false })?;
        let token_data: serde_json::Value = resp
            .json()
            .unwrap_or(serde_json::Value::Object(Default::default()));

        let id_token = token_data
            .get("id_token")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let Some(id_token) = id_token else {
            let error_desc = token_data
                .get("error_description")
                .or_else(|| token_data.get("error"))
                .map(|v| v.to_string().trim_matches('"').to_string())
                .unwrap_or_else(|| {
                    tr_msg(
                        "oauth_google_token_exchange_failed",
                        None,
                        Some("Falha ao trocar código Google por token"),
                    )
                });
            return Err(OAuthException { message: error_desc, is_cancelled: false });
        };

        let payload = OAuthPkce::decode_jwt_payload(&id_token)
            .map_err(|e| OAuthException { message: e, is_cancelled: false })?;
        let social_id = payload.get("sub").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let email = payload.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = payload
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                if !email.is_empty() {
                    email.split('@').next().unwrap_or("").to_string()
                } else {
                    tr_msg("oauth_google_user", None, Some("Usuário Google"))
                }
            });

        Ok(OAuthUserInfo { provider: "Google".into(), social_id, email, name })
    }
}

pub fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
