//! Port of `lib/features/auth/auth_notifier.dart`.
//!
//! Runs on a worker thread so network work never blocks the egui frame; the UI
//! sends `AuthCmd` and repaints when a response arrives.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::constants::DEFAULT_LANGUAGE;
use crate::localization::tr_msg;
use crate::oauth::OAuthException;
use crate::repositories::auth::{AuthError, AuthFlowError, AuthRepository};
use crate::state::Shared;

#[derive(Debug)]
pub enum AuthCmd {
    CheckStatus,
    SignInEmail { email: String, password: String },
    SignInJwt(String),
    SignInGoogle,
    CancelGoogle,
    SignUpGoogle { nickname: String, category_id: i64, country_id: i64 },
    Logout,
    UpdateNickname(String),
    UpdateStatusMessage(String),
    Shutdown,
}

/// Result of a Google sign-in that needs the sign-up sheet.
#[derive(Debug, Clone)]
pub struct PendingGoogleUser {
    pub provider: String,
    pub social_id: String,
    pub email: String,
    pub name: String,
}

pub fn spawn_auth_thread(
    shared: Arc<Mutex<Shared>>,
    repo: Arc<AuthRepository>,
    on_signed_in: Box<dyn Fn() + Send + Sync>,
) -> Sender<AuthCmd> {
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_tx = tx.clone();
    std::thread::spawn(move || {
        let mut ctrl = AuthController::new(shared, repo, worker_tx, on_signed_in);
        ctrl.run(rx);
    });
    tx
}

struct AuthController {
    shared: Arc<Mutex<Shared>>,
    repo: Arc<AuthRepository>,
    tx: Sender<AuthCmd>,
    on_signed_in: Box<dyn Fn() + Send + Sync>,
    pending_google: Option<PendingGoogleUser>,
}

impl AuthController {
    fn new(
        shared: Arc<Mutex<Shared>>,
        repo: Arc<AuthRepository>,
        tx: Sender<AuthCmd>,
        on_signed_in: Box<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self { shared, repo, tx, on_signed_in, pending_google: None }
    }

    fn run(&mut self, rx: Receiver<AuthCmd>) {
        // Kick off the boot-time session restore immediately.
        let _ = self.tx.send(AuthCmd::CheckStatus);
        while let Ok(cmd) = rx.recv() {
            match cmd {
                AuthCmd::Shutdown => break,
                AuthCmd::CheckStatus => self.check_status(),
                AuthCmd::SignInEmail { email, password } => {
                    self.sign_in_email(&email, &password)
                }
                AuthCmd::SignInJwt(token) => self.sign_in_jwt(&token),
                AuthCmd::SignInGoogle => self.sign_in_google(),
                AuthCmd::CancelGoogle => {
                    let mut guard = self.shared.lock().unwrap();
                    guard.auth.is_loading = false;
                }
                AuthCmd::SignUpGoogle { nickname, category_id, country_id } => {
                    self.sign_up_google(&nickname, category_id, country_id)
                }
                AuthCmd::Logout => self.logout(),
                AuthCmd::UpdateNickname(n) => self.update_nickname(&n),
                AuthCmd::UpdateStatusMessage(s) => self.update_status_message(&s),
            }
        }
    }

    // ---------------------------------------------------------------- helpers

    fn set_loading(&self, loading: bool) {
        let mut guard = self.shared.lock().unwrap();
        guard.auth.is_loading = loading;
        if loading {
            guard.auth.error_message = None;
        }
    }

    fn set_error(&self, message: String) {
        let mut guard = self.shared.lock().unwrap();
        guard.auth.is_loading = false;
        guard.auth.error_message = Some(message);
    }

    fn set_user(&self, user: crate::models::UserModel) {
        {
            let mut guard = self.shared.lock().unwrap();
            guard.auth.user = Some(user);
            guard.auth.is_loading = false;
            guard.auth.error_message = None;
        }
        (self.on_signed_in)();
    }

    fn format_error(&self, e: &AuthError) -> String {
        let msg = &e.message;
        if msg.contains("timed out") || msg.contains("timeout") {
            return tr_msg(
                "net_timeout",
                None,
                Some("Tempo limite de conexão esgotado. Verifique sua internet."),
            );
        }
        if msg.contains("connection error") {
            return tr_msg(
                "net_server_unreachable",
                None,
                Some("Falha de conexão com o servidor da TGC Lab / YPT."),
            );
        }
        // Server body carried a machine code: map it to a friendly sentence.
        if let Some(body) = e.message_code() {
            let mapped = match body.as_str() {
                "103" | "116" => Some(tr_msg(
                    "auth_nickname_invalid",
                    None,
                    Some("Não é possível usar este apelido. Insira outro apelido."),
                )),
                "104" => Some(tr_msg(
                    "auth_nickname_in_use",
                    None,
                    Some("Este apelido já está em uso. Insira outro apelido."),
                )),
                "106" => Some(tr_msg(
                    "auth_email_taken",
                    None,
                    Some("Este e-mail já está registrado. Volte e inicie a sessão ou use outro endereço de e-mail."),
                )),
                "107" => Some(tr_msg(
                    "auth_email_invalid_register",
                    None,
                    Some("Não é possível cadastrar este endereço de e-mail."),
                )),
                "108" | "invalid_auth_code_msg" => Some(tr_msg(
                    "auth_code_mismatch",
                    None,
                    Some("O código de autenticação não corresponde."),
                )),
                "112" => Some("Senha incorreta. Verifique sua senha e tente novamente.".into()),
                "113" => Some(tr_msg(
                    "auth_email_not_registered",
                    None,
                    Some("E-mail não cadastrado no Yeolpumta."),
                )),
                "114" => Some("Conta suspensa ou inativa.".into()),
                "115" => Some(tr_msg(
                    "auth_account_protected",
                    None,
                    Some("Esta conta está protegida contra exclusão devido a um problema em andamento."),
                )),
                "125" => Some(tr_msg(
                    "auth_email_invalid",
                    None,
                    Some("E-mail inválido. Tente com outro endereço de e-mail."),
                )),
                _ => None,
            };
            if let Some(mapped) = mapped {
                return mapped;
            }
        }
        // Fallback: strip Dart exception prefixes, mirroring _formatError.
        msg.replace("Exception: ", "")
            .replace("ApiException: ", "")
    }

    // ------------------------------------------------------------------ flows

    fn check_status(&self) {
        self.set_loading(true);
        let user = self.repo.try_restore_session();
        let mut guard = self.shared.lock().unwrap();
        guard.auth.is_loading = false;
        guard.auth.user = user;
        if guard.auth.user.is_some() {
            drop(guard);
            (self.on_signed_in)();
        }
    }

    fn sign_in_email(&self, email: &str, password: &str) {
        self.set_loading(true);
        match self
            .repo
            .sign_in_with_email(email, password, DEFAULT_LANGUAGE)
        {
            Ok(user) => self.set_user(user),
            Err(e) => self.set_error(self.format_error(&e)),
        }
    }

    fn sign_in_jwt(&self, token: &str) {
        self.set_loading(true);
        let clean = token.trim().replace("JWT ", "").replace("Bearer ", "");
        if clean.is_empty() {
            self.set_error(tr_msg("auth_no_token", None, Some("Token JWT não fornecido.")));
            return;
        }
        // Mirror AuthRepository.signInWithJwt: persist then splash.
        self.repo.save_token_for_restore(&clean);
        match self
            .repo
            .splash_login(DEFAULT_LANGUAGE, crate::constants::DEFAULT_TIMEZONE, crate::constants::DEFAULT_VERSION)
        {
            Some(splash) => {
                crate::repositories::subject::SubjectRepository::save_subjects_from_raw_data(
                    self.repo.store_handle(),
                    &splash,
                );
                let user = crate::repositories::auth::user_from_payload(&splash, &clean);
                self.repo.cache_user(&user);
                self.set_user(user);
            }
            None => {
                self.repo.clear_token_for_restore();
                self.set_error(tr_msg(
                    "auth_session_expired",
                    None,
                    Some("Sessão inválida ou expirada no servidor."),
                ));
            }
        }
    }

    fn sign_in_google(&mut self) {
        self.set_loading(true);
        let mut google = crate::oauth::GoogleOAuthService::new();
        if !google.is_configured() {
            self.set_error(tr_msg(
                "oauth_google_missing_client_id",
                None,
                Some("Google Client ID não configurado no arquivo .env."),
            ));
            return;
        }
        let info = match google.authenticate(std::time::Duration::from_secs(180)) {
            Ok(info) => info,
            Err(OAuthException { message, is_cancelled }) => {
                if is_cancelled {
                    self.set_loading(false);
                } else {
                    self.set_error(message);
                }
                return;
            }
        };

        let formatted = if info.social_id.starts_with('g') {
            info.social_id.clone()
        } else {
            format!("g{}", info.social_id)
        };
        let account_exists = self.repo.check_username_exists(&formatted);
        if !account_exists {
            self.pending_google = Some(PendingGoogleUser {
                provider: info.provider.clone(),
                social_id: info.social_id.clone(),
                email: info.email.clone(),
                name: info.name.clone(),
            });
            let mut guard = self.shared.lock().unwrap();
            guard.auth.is_loading = false;
            return;
        }

        match self.repo.sign_in_with_social(
            &info.provider,
            &info.social_id,
            &info.email,
            &info.name,
            DEFAULT_LANGUAGE,
        ) {
            Ok(user) => self.set_user(user),
            Err(AuthFlowError::SignUpRequired(req)) => {
                self.pending_google = Some(PendingGoogleUser {
                    provider: req.provider,
                    social_id: req.social_id,
                    email: req.email,
                    name: req.name,
                });
                let mut guard = self.shared.lock().unwrap();
                guard.auth.is_loading = false;
            }
            Err(AuthFlowError::Shown(e)) => self.set_error(self.format_error(&e)),
        }
    }

    fn sign_up_google(&self, nickname: &str, category_id: i64, country_id: i64) {
        let Some(pending) = self.pending_google.clone() else {
            self.set_loading(false);
            return;
        };
        self.set_loading(true);
        match self.repo.sign_up_with_social(
            &pending.provider,
            &pending.social_id,
            &pending.email,
            &pending.name,
            nickname,
            country_id,
            category_id,
            DEFAULT_LANGUAGE,
        ) {
            Ok(user) => self.set_user(user),
            Err(AuthFlowError::Shown(e)) => self.set_error(self.format_error(&e)),
            Err(AuthFlowError::SignUpRequired(_)) => {
                self.set_error(tr_msg(
                    "auth_failed_via_provider",
                    Some(vec!["Google"]),
                    Some("Falha ao autenticar via Google"),
                ));
            }
        }
    }

    fn logout(&self) {
        self.repo.logout();
        let mut guard = self.shared.lock().unwrap();
        guard.auth.user = None;
        guard.auth.is_loading = false;
        guard.auth.error_message = None;
        // Drop the session state so a different account never inherits the
        // previous subject or totals (mirrors `ref.invalidate(timerProvider)`).
        guard.timer = crate::state::TimerState::default();
        guard.timer.last_study_date = String::new();
        drop(guard);
        let _ = self.tx.send(AuthCmd::CheckStatus);
    }

    fn update_nickname(&self, new_nickname: &str) {
        if self.repo.change_nickname(new_nickname) {
            let mut guard = self.shared.lock().unwrap();
            if let Some(user) = guard.auth.user.as_mut() {
                user.name = new_nickname.to_string();
            }
        }
    }

    fn update_status_message(&self, new_status: &str) {
        if self.repo.change_status_message(new_status) {
            let mut guard = self.shared.lock().unwrap();
            if let Some(user) = guard.auth.user.as_mut() {
                user.status_message = new_status.to_string();
            }
        }
    }
}
