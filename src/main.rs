//! Mpt — a minimal YPT study timer for the desktop.
//!
//! Entry point only: window setup, frame pump, close interception. Screens live
//! in `ui/`, feature state in `state::Shared`, and every network-facing feature
//! runs on its own worker thread so a slow request never stalls a frame.

// The client carries the full YPT request surface (sign-up, groups, subject
// management) even where the desktop UI does not call it today. Kept whole on
// purpose: it is the part of the app that talks to the service.
#![allow(dead_code)]

// No console window behind the app when double-clicked from Explorer.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod auth;
mod background;
mod config;
mod constants;
mod daily_stats;
mod localization;
mod model_utils;
mod models;
mod oauth;
mod offline_sync;
mod repositories;
mod settings;
mod state;
mod store;
mod theme;
mod timer;
mod timer_session_store;
mod ui;
mod utils;

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::auth::{spawn_auth_thread, AuthCmd};
use crate::offline_sync::{spawn_offline_thread, OfflineCmd};
use crate::repositories::auth::AuthRepository;
use crate::repositories::offline_sync::OfflineSyncRepository;
use crate::repositories::subject::SubjectRepository;
use crate::repositories::timer::TimerRepository;
use crate::settings::SettingsState;
use crate::state::Shared;
use crate::timer::{spawn_timer_thread, TimerCmd, TimerDeps};

const WINDOW_TITLE: &str = "Mpt";
const FULL_SIZE: [f32; 2] = [1180.0, 760.0];
const FULL_MIN: [f32; 2] = [900.0, 560.0];
const MINI_SIZE: [f32; 2] = [360.0, 148.0];
const MINI_MIN: [f32; 2] = [340.0, 140.0];

/// Closing must stop the running session first, but never hang the window.
const CLOSE_STOP_BUDGET: Duration = Duration::from_millis(1500);

pub struct MptApp {
    pub(crate) shared: Arc<Mutex<Shared>>,

    /// Spawned on first sign-in, torn down on logout: a restored session must
    /// never outlive the account it belongs to.
    timer_tx: Arc<Mutex<Option<Sender<TimerCmd>>>>,
    timer_args: Option<TimerArgs>,
    pub(crate) auth_tx: Sender<AuthCmd>,
    pub(crate) offline_tx: Sender<OfflineCmd>,

    // Login form.
    pub(crate) email: String,
    pub(crate) password: String,
    pub(crate) obscure_password: bool,
    pub(crate) local_error: Option<String>,

    pub(crate) google_enabled: bool,
    pub(crate) logo: Option<egui::TextureHandle>,

    // Mini player bookkeeping.
    pub(crate) is_mini: bool,
    restore_size: [f32; 2],
    restore_position: Option<egui::Pos2>,

    // Study-screen overlays.
    pub(crate) all_subjects_rect: Option<egui::Rect>,
    pub(crate) logout_requested: bool,
    /// One-shot: replay the offline queue the first time the study screen shows.
    pub(crate) synced_on_entry: bool,

    // Close interception.
    closing: bool,
    close_started: Option<Instant>,
    close_saw_busy: bool,
    focused: bool,

    background: background::Background,
    /// The window has been placed once (eframe does not centre on its own).
    positioned: bool,
}

/// Everything the timer thread needs, parked until authentication happens.
struct TimerArgs {
    deps: Arc<TimerDeps>,
    settings: SettingsState,
}

impl MptApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);

        let store = Arc::new(Mutex::new(store::Store::load()));
        let settings = SettingsState::from_store(&store.lock().unwrap());
        let shared = Arc::new(Mutex::new(Shared::default()));

        let timer_repo = Arc::new(TimerRepository::new(&store));
        let subject_repo = Arc::new(SubjectRepository::new(&store));
        let offline_repo = Arc::new(OfflineSyncRepository::new(
            timer_repo.clone() as _,
            store.clone(),
        ));
        let auth_repo = Arc::new(AuthRepository::new(&store));

        let offline_tx = spawn_offline_thread(shared.clone(), offline_repo.clone());
        let offline_tx_for_auth = offline_tx.clone();

        let auth_tx = spawn_auth_thread(
            shared.clone(),
            auth_repo,
            Box::new(move || {
                // Entering the app: replay any stops queued while signed out.
                let _ = offline_tx_for_auth.send(OfflineCmd::SyncNow);
            }),
        );

        let timer_args = Some(TimerArgs {
            deps: Arc::new(TimerDeps {
                timer: timer_repo as _,
                subjects: subject_repo as _,
                offline: offline_repo,
                session_store: store.clone(),
            }),
            settings,
        });

        Self {
            shared,
            timer_tx: Arc::new(Mutex::new(None)),
            timer_args,
            auth_tx,
            offline_tx,
            email: String::new(),
            password: String::new(),
            obscure_password: true,
            local_error: None,
            google_enabled: !config::env_instance().get_google_client_id().is_empty(),
            logo: load_logo(&cc.egui_ctx),
            is_mini: false,
            restore_size: FULL_SIZE,
            restore_position: None,
            all_subjects_rect: None,
            logout_requested: false,
            synced_on_entry: false,
            closing: false,
            close_started: None,
            close_saw_busy: false,
            focused: true,
            background: background::Background::new(),
            positioned: false,
        }
    }

    /// Send a command to the timer thread, spawning it on first use.
    pub(crate) fn send_timer(&self, cmd: TimerCmd) {
        if let Some(tx) = self.ensure_timer_thread() {
            let _ = tx.send(cmd);
        }
    }

    fn ensure_timer_thread(&self) -> Option<Sender<TimerCmd>> {
        let mut slot = self.timer_tx.lock().ok()?;
        if let Some(tx) = slot.as_ref() {
            return Some(tx.clone());
        }
        let args = self.timer_args.as_ref()?;
        let tx = spawn_timer_thread(
            self.shared.clone(),
            args.deps.clone(),
            args.settings.clone(),
            None,
        );
        let _ = slot.replace(tx.clone());
        Some(tx)
    }

    /// Tear the timer thread down (logout); the next sign-in bootstraps fresh.
    fn shutdown_timer(&self) {
        if let Ok(mut slot) = self.timer_tx.lock() {
            if let Some(tx) = slot.take() {
                let _ = tx.send(TimerCmd::Shutdown);
            }
        }
    }

    pub fn toggle_mini(&mut self, ctx: &egui::Context) {
        if !self.is_mini {
            // Remember where the window was and how big, so leaving the mini
            // player puts it back exactly.
            let (pos, size) = ctx.input(|i| {
                let v = i.viewport();
                (v.outer_rect.map(|r| r.min), v.inner_rect.map(|r| r.size()))
            });
            self.restore_position = pos;
            if let Some(size) = size {
                self.restore_size = [size.x, size.y];
            }
            self.is_mini = true;
            // Shrink the minimum before the size, or the window refuses to move.
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                MINI_MIN[0], MINI_MIN[1],
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(egui::vec2(
                MINI_SIZE[0], MINI_SIZE[1],
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                MINI_SIZE[0], MINI_SIZE[1],
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
        } else {
            self.is_mini = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
            ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(egui::vec2(
                f32::MAX,
                f32::MAX,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                FULL_MIN[0], FULL_MIN[1],
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                self.restore_size[0],
                self.restore_size[1],
            )));
            if let Some(pos) = self.restore_position {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
            }
        }
    }

    /// Stop the session (bounded), then let the window go — the snapshot on
    /// disk keeps whatever was still in flight.
    pub(crate) fn begin_close(&mut self, ctx: &egui::Context) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.close_started = Some(Instant::now());
        let (has_session, busy) = {
            let guard = self.shared.lock().unwrap();
            (guard.timer.has_session(), guard.timer.busy)
        };
        self.close_saw_busy = busy;
        if has_session {
            self.send_timer(TimerCmd::Stop);
        } else {
            self.close_started = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn handle_close_budget(&mut self, ctx: &egui::Context) {
        let Some(started) = self.close_started else {
            return;
        };
        let busy = self.shared.lock().unwrap().timer.busy;
        self.close_saw_busy |= busy;
        // Leave once the stop landed, or once the budget elapses with the stop
        // still in flight — the snapshot keeps the rest.
        let settled = self.close_saw_busy && !busy;
        if settled || started.elapsed() >= CLOSE_STOP_BUDGET {
            self.close_started = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Window focus: refresh the day's totals as soon as the window comes back.
    fn handle_focus(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.focused {
            let authenticated = self.shared.lock().unwrap().auth.is_authenticated();
            if authenticated {
                self.send_timer(TimerCmd::RefreshStats);
            }
        }
        self.focused = focused;
    }
}

fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    // Embedded so the exe is a single file, wherever it is run from.
    let bytes: &[u8] = include_bytes!("../assets/images/logo.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (width, height) = (info.width as usize, info.height as usize);
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);
    Some(ctx.load_texture("logo", color_image, egui::TextureOptions::LINEAR))
}

impl eframe::App for MptApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // The clock and the spinners animate; 200 ms is plenty for a 1 s clock.
        ctx.request_repaint_after(Duration::from_millis(200));

        self.handle_focus(&ctx);
        self.handle_close_budget(&ctx);

        // First frame: place the window in the middle of the monitor.
        if !self.positioned {
            if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
                self.positioned = true;
                if !self.is_mini {
                    let x = (monitor.x - FULL_SIZE[0]) * 0.5;
                    let y = (monitor.y - FULL_SIZE[1]) * 0.5;
                    if x > 0.0 && y > 0.0 {
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                            x, y,
                        )));
                    }
                }
            }
        }

        // Sign-out requested from the subject panel.
        if self.logout_requested {
            self.logout_requested = false;
            self.is_mini = false;
            self.all_subjects_rect = None;
            self.synced_on_entry = false;
            // Signing out must not throw away a live session: stop records it (or
            // queues it offline), which needs a moment before the worker dies.
            let live = {
                let guard = self.shared.lock().unwrap();
                guard.timer.is_running || guard.timer.is_resting
            };
            if live {
                self.send_timer(TimerCmd::Stop);
                std::thread::sleep(Duration::from_millis(250));
            }
            self.shutdown_timer();
            let _ = self.auth_tx.send(AuthCmd::Logout);
        }

        let (authenticated, loading) = {
            let guard = self.shared.lock().unwrap();
            (guard.auth.is_authenticated(), guard.auth.is_loading)
        };
        if authenticated {
            // First frame after sign-in bootstraps the timer thread.
            let _ = self.ensure_timer_thread();
        } else {
            self.is_mini = false;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::colors::BACKGROUND))
            .show(ui, |ui| {
                self.background.paint(ui, authenticated);

                if !authenticated {
                    if loading {
                        ui::boot_screen(ui);
                    } else {
                        ui::login_screen(self, &ctx, ui);
                    }
                } else {
                    ui::study::home_screen(self, &ctx, ui);
                }
            });

        self.resize_edges(&ctx, ui);
    }
}

impl MptApp {
    /// A window without decorations gets no OS resize border, so the 5 px ring
    /// is ours to provide. Hit-testing only — the frame draws no border of its
    /// own; hairlines belong to the screens, not the window edge.
    fn resize_edges(&self, ctx: &egui::Context, ui: &mut egui::Ui) {
        use egui::CursorIcon;
        use egui::viewport::ResizeDirection as Dir;

        let Some(win) = ctx.input(|i| i.viewport().inner_rect) else {
            return;
        };
        let b = 5.0;
        let (l, r, t, bo) = (win.min.x, win.max.x, win.min.y, win.max.y);
        let z = |x0: f32, y0: f32, x1: f32, y1: f32| {
            egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
        };
        let zones = [
            (z(l, t, l + b, t + b), Dir::NorthWest, CursorIcon::ResizeNwSe),
            (z(r - b, t, r, t + b), Dir::NorthEast, CursorIcon::ResizeNeSw),
            (z(l, bo - b, l + b, bo), Dir::SouthWest, CursorIcon::ResizeNeSw),
            (z(r - b, bo - b, r, bo), Dir::SouthEast, CursorIcon::ResizeNwSe),
            (z(l, t + b, l + b, bo - b), Dir::West, CursorIcon::ResizeHorizontal),
            (z(r - b, t + b, r, bo - b), Dir::East, CursorIcon::ResizeHorizontal),
            (z(l + b, t, r - b, t + b), Dir::North, CursorIcon::ResizeVertical),
            (z(l + b, bo - b, r - b, bo), Dir::South, CursorIcon::ResizeVertical),
        ];
        for (i, (zone, dir, cursor)) in zones.into_iter().enumerate() {
            let resp = ui.interact(zone, ui.scope_id().with(("resize", i)), egui::Sense::drag());
            if resp.hovered() {
                ctx.set_cursor_icon(cursor);
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                }
            }
        }
    }
}

fn main() -> eframe::Result {
    install_panic_log();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(WINDOW_TITLE)
        .with_decorations(false)
        .with_inner_size(FULL_SIZE)
        .with_min_inner_size(FULL_MIN);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|cc| Ok(Box::new(MptApp::new(cc)))),
    )
}

/// There is no console to print to, so a crash leaves one small file behind
/// instead of the app simply vanishing.
fn install_panic_log() {
    std::panic::set_hook(Box::new(|info| {
        let path = crate::store::app_data_dir().join("crash.log");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, format!("{info}\n"));
    }));
}

/// Taskbar / alt-tab icon, from the same PNG the title bar draws.
fn window_icon() -> Option<egui::IconData> {
    let bytes: &[u8] = include_bytes!("../assets/images/logo.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    Some(egui::IconData {
        rgba: buf,
        width: info.width,
        height: info.height,
    })
}
