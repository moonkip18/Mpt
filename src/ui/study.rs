//! The study screen and the mini player.
//!
//! Everything is positioned inside one body rect with the same paddings the
//! Flutter layout used (34/26/34/24 on the study column, a 316 px subject
//! panel), so window resizing behaves the same: the column flexes, the panel
//! keeps its width, and the clock stays optically centred.

use eframe::egui::{self, Align2, Color32, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::offline_sync::OfflineCmd;
use crate::state::{OfflineSyncState, TimerState};
use crate::theme::colors::*;
use crate::theme::fonts;
use crate::timer::TimerCmd;
use crate::utils::format::{format_clock, format_day_label, format_hm, format_hm_compact};
use crate::MptApp;

use super::{caps_job, caps_lift, font, hairline, mix, spinner, title_bar};

const PANEL_WIDTH: f32 = 316.0;
const CHIP_HEIGHT: f32 = 30.0;
const BUTTON_HEIGHT: f32 = 42.0;
const STAT_ROW_HEIGHT: f32 = 40.0;

#[allow(deprecated)]
fn sid(name: impl std::hash::Hash + std::fmt::Debug) -> Id {
    Id::new(name)
}

/// One frame's worth of state, copied out of the shared lock so the drawing
/// code never holds it.
struct Snap {
    timer: TimerState,
    offline: OfflineSyncState,
    user_name: String,
}

fn snapshot(app: &MptApp) -> Snap {
    let guard = app.shared.lock().unwrap();
    Snap {
        timer: guard.timer.clone(),
        offline: guard.offline.clone(),
        user_name: guard
            .auth
            .user
            .as_ref()
            .map(|u| u.name.clone())
            .unwrap_or_default(),
    }
}

/// Entry point from `main`.
pub fn home_screen(app: &mut MptApp, ctx: &egui::Context, ui: &mut Ui) {
    let booted = app.shared.lock().unwrap().timer_booted;
    let actions = title_bar(ctx, ui, app.logo.as_ref(), true, app.is_mini);
    hairline(ui);

    if !booted {
        // Bootstrap (subjects + today's stats) takes a couple of seconds on
        // the API. Hold the chrome with a quiet spinner instead of drawing an
        // empty home; the settle fade covers the switch once data lands.
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
        let body = ui.available_rect_before_wrap();
        spinner(ui, body.center(), 16.0, INK_FAINT);
        actions.apply(app, ctx);
        return;
    }

    if !app.synced_on_entry {
        app.synced_on_entry = true;
        let _ = app.offline_tx.send(OfflineCmd::LoadPending);
        let pending = app.shared.lock().unwrap().offline.has_pending();
        if pending {
            let _ = app.offline_tx.send(OfflineCmd::SyncNow);
        }
    }

    let snap = snapshot(app);
    if app.is_mini {
        mini_body(app, ui, &snap);
    } else {
        full_body(app, ctx, ui, &snap);
    }

    actions.apply(app, ctx);
}

// ------------------------------------------------------------------ full view

fn full_body(app: &mut MptApp, ctx: &egui::Context, ui: &mut Ui, snap: &Snap) {
    let body = ui.available_rect_before_wrap();
    if body.width() < 40.0 || body.height() < 40.0 {
        return;
    }

    // The panel is fixed; the study column takes whatever is left. Keep the
    // panel from ever stealing the column, even at odd window sizes.
    let panel_width = PANEL_WIDTH
        .min((body.width() * 0.42).max(240.0))
        .max(220.0)
        .min((body.width() - 320.0).max(220.0));
    let split = body.max.x - panel_width;

    ui.painter().rect_filled(
        Rect::from_min_max(Pos2::new(split, body.min.y), Pos2::new(split + 1.0, body.max.y)),
        0.0,
        LINE,
    );

    let study = Rect::from_min_max(body.min, Pos2::new(split, body.max.y));
    study_column(app, ctx, ui, study, snap);

    let panel = Rect::from_min_max(Pos2::new(split + 1.0, body.min.y), body.max);
    subject_panel(app, ctx, ui, panel, snap);

    all_subjects_menu(app, ctx, ui, snap);
}

fn study_column(app: &mut MptApp, ctx: &egui::Context, ui: &mut Ui, rect: Rect, snap: &Snap) {
    let inner = Rect::from_min_max(
        Pos2::new(rect.min.x + 34.0, rect.min.y + 26.0),
        Pos2::new(rect.max.x - 34.0, rect.max.y - 24.0),
    );
    let t = &snap.timer;
    let running = t.is_running;
    let resting = t.is_resting;
    let idle = !running && !resting;

    header(ui, inner, snap);

    // Bottom-up: the strip and actions sit above the floor, the clock centres
    // in whatever is left between them and the header.
    let notice_h = if t.notice.is_some() { 34.0 } else { 16.0 };
    let notice_y = inner.max.y - notice_h;
    let controls_y = notice_y - 14.0 - BUTTON_HEIGHT;
    let strip_y = controls_y - 22.0 - CHIP_HEIGHT;

    let clock_top = inner.min.y + 52.0;
    let clock_room = (strip_y - clock_top).max(120.0);
    clock_block(ui, Pos2::new(inner.min.x, clock_top), inner.width(), clock_room, snap);

    subject_strip(app, ui, Rect::from_min_size(
        Pos2::new(inner.min.x, strip_y),
        Vec2::new(inner.width(), CHIP_HEIGHT),
    ), snap, idle);

    controls(app, ui, Pos2::new(inner.min.x, controls_y), snap);

    if let Some(notice) = &t.notice {
        notice_line(app, ui, Rect::from_min_size(
            Pos2::new(inner.min.x, notice_y),
            Vec2::new(inner.width(), notice_h),
        ), notice);
    }

    let _ = ctx;
}

fn header(ui: &mut Ui, inner: Rect, snap: &Snap) {
    let t = &snap.timer;

    let caps = ui.painter().layout_job(caps_job("STUDY LOG", INK_FAINT));
    ui.painter().galley(
        Pos2::new(inner.min.x, inner.min.y + caps_lift(caps.size().y, 11.0)),
        caps,
        INK_FAINT,
    );
    ui.painter().text(
        Pos2::new(inner.min.x, inner.min.y + 19.0),
        Align2::LEFT_TOP,
        format_day_label(chrono::Local::now().date_naive()),
        font(20.0, fonts::DISPLAY),
        INK,
    );

    let right = inner.max.x;
    let caps = ui.painter().layout_job(caps_job("TODAY", INK_FAINT));
    ui.painter().galley(
        // Right-align to the column edge — a galley paints from its left
        // edge, so positioning at `right` would push the label off-window.
        Pos2::new(right - caps.size().x, inner.min.y + caps_lift(caps.size().y, 11.0)),
        caps,
        INK_FAINT,
    );
    ui.painter().text(
        Pos2::new(right, inner.min.y + 15.0),
        Align2::RIGHT_TOP,
        format_hm(t.display_total_ms()),
        font(26.0, fonts::MONO),
        INK,
    );

    // Rest time and the phone badge share the line under the total.
    let mut x = right;
    let badge_y = inner.min.y + 46.0;
    if t.remote_active {
        let label = "PHONE LIVE";
        let galley = ui
            .painter()
            .layout_no_wrap(label.to_string(), font(9.0, fonts::MONO), ACCENT);
        let w = galley.size().x;
        ui.painter()
            .galley(Pos2::new(x - w, badge_y), galley, ACCENT);
        x -= w + 10.0;
    }
    if t.stats.rest_ms > 0 {
        let label = format!("rest {}", format_hm(t.stats.rest_ms));
        let galley = ui
            .painter()
            .layout_no_wrap(label, font(11.0, fonts::MONO), INK_FAINT);
        let w = galley.size().x;
        ui.painter()
            .galley(Pos2::new(x - w, badge_y - 1.0), galley, INK_FAINT);
    }
}

fn clock_block(ui: &mut Ui, at: Pos2, width: f32, room: f32, snap: &Snap) {
    let t = &snap.timer;
    let state = if t.is_running {
        "STUDYING"
    } else if t.is_resting {
        "RESTING"
    } else if t.session_elapsed_ms > 0 {
        "PAUSED"
    } else {
        "READY"
    };

    // 11 + 12 + 30 + 8 + 86 + a little breathing room.
    let block_h = 152.0;
    let top = at.y + ((room - block_h) * 0.5).max(0.0);

    let mut x = at.x;
    if t.is_running {
        ui.painter()
            .circle_filled(Pos2::new(x + 2.5, top + 5.5), 2.5, ACCENT);
        x += 13.0;
    }
    let state_galley = ui.painter().layout_job(caps_job(state, INK_FAINT));
    ui.painter().galley(
        Pos2::new(x, top + caps_lift(state_galley.size().y, 11.0)),
        state_galley,
        INK_FAINT,
    );

    let subject = t
        .current_subject
        .as_ref()
        .map(|s| s.title.as_str())
        .unwrap_or("No subject");
    let galley = ui.painter().layout(
        subject.to_string(),
        font(27.0, fonts::DISPLAY_ITALIC),
        INK,
        width,
    );
    ui.painter()
        .galley(Pos2::new(at.x, top + 22.0), galley, INK);

    ui.painter().text(
        Pos2::new(at.x, top + 58.0),
        Align2::LEFT_TOP,
        format_clock(t.session_elapsed_ms),
        font(86.0, fonts::MONO),
        if t.is_running { INK } else { INK_DIM },
    );
}

fn subject_strip(app: &mut MptApp, ui: &mut Ui, rect: Rect, snap: &Snap, enabled: bool) {
    let subjects = &snap.timer.subjects;
    if subjects.is_empty() {
        ui.painter().text(
            rect.min,
            Align2::LEFT_TOP,
            "No subjects yet. Create one in the YPT app and it will appear here.",
            font(12.0, fonts::UI),
            INK_FAINT,
        );
        return;
    }

    let current_id = snap.timer.current_subject.as_ref().map(|s| s.id);
    let all_btn_w = if subjects.len() > 1 { 38.0 } else { 0.0 };
    let strip_right = rect.max.x - all_btn_w;

    let mut x = rect.min.x;
    for subject in subjects {
        let galley = ui.painter().layout_no_wrap(
            subject.title.clone(),
            font(12.0, fonts::UI),
            INK_DIM,
        );
        let chip_w = 11.0 + 6.0 + 7.0 + galley.size().x + 11.0;
        if x + chip_w > strip_right {
            break;
        }

        let chip = Rect::from_min_size(
            Pos2::new(x, rect.min.y),
            Vec2::new(chip_w, CHIP_HEIGHT),
        );
        let resp = ui.interact(chip, sid(("mpt.chip", subject.id)), Sense::click());
        let selected = Some(subject.id) == current_id;
        let hovered = resp.hovered() && enabled;
        let h = ui.ctx().animate_bool_with_time(
            sid(("mpt.chip.hover", subject.id)),
            hovered,
            0.12,
        );

        let fill = if selected {
            SURFACE_RAISED
        } else {
            mix(Color32::TRANSPARENT, SURFACE, h)
        };
        ui.painter().rect_filled(chip, 2.0, fill);
        ui.painter().rect_stroke(
            chip,
            2.0,
            Stroke::new(1.0, if selected { LINE_STRONG } else { LINE }),
            StrokeKind::Inside,
        );

        ui.painter().circle_filled(
            Pos2::new(x + 11.0 + 3.0, chip.center().y),
            3.0,
            from_argb(subject.color_int),
        );
        ui.painter().galley(
            Pos2::new(x + 11.0 + 6.0 + 7.0, chip.center().y - galley.size().y / 2.0),
            galley,
            if selected { INK } else { INK_DIM },
        );

        if enabled {
            resp.clone()
                .on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() && enabled {
            app.send_timer(TimerCmd::SelectSubject(subject.id));
        }

        x += chip_w + 8.0;
    }

    if subjects.len() > 1 {
        let btn = Rect::from_min_size(
            Pos2::new(rect.max.x - 30.0, rect.min.y),
            Vec2::new(30.0, CHIP_HEIGHT),
        );
        let resp = ui.interact(btn, sid("mpt.all_subjects"), Sense::click());
        let hovered = resp.hovered();
        ui.painter().rect_filled(
            btn,
            2.0,
            if hovered { SURFACE } else { Color32::TRANSPARENT },
        );
        ui.painter().rect_stroke(
            btn,
            2.0,
            Stroke::new(1.0, if hovered { LINE_STRONG } else { LINE }),
            StrokeKind::Inside,
        );
        ui.painter().text(
            btn.center(),
            Align2::CENTER_CENTER,
            "›",
            font(16.0, fonts::UI),
            if hovered { INK } else { INK_DIM },
        );
        resp.clone().on_hover_text("All subjects");
        resp.clone()
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            app.all_subjects_rect = Some(btn);
        }
    }
}

/// The full list behind the `›` button: every subject with today's time.
fn all_subjects_menu(app: &mut MptApp, ctx: &egui::Context, ui: &mut Ui, snap: &Snap) {
    let Some(anchor) = app.all_subjects_rect else {
        return;
    };
    if snap.timer.subjects.is_empty() {
        app.all_subjects_rect = None;
        return;
    }

    let rows = snap.timer.subjects.len() as f32;
    let height = (rows * 30.0 + 10.0).min(340.0);
    let below = anchor.bottom() + 4.0;
    let y = if below + height > ui.max_rect().max.y - 8.0 {
        (anchor.top() - height - 4.0).max(ui.max_rect().min.y + 4.0)
    } else {
        below
    };
    let x = anchor.min.x.min(ui.max_rect().max.x - 244.0).max(ui.max_rect().min.x + 4.0);

    let stats = snap.timer.display_stats();
    let current_id = snap.timer.current_subject.as_ref().map(|s| s.id);
    let enabled = !snap.timer.is_running && !snap.timer.is_resting;

    let area = egui::Area::new(sid("mpt.all_subjects.menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(x, y))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(SURFACE_RAISED)
                .stroke(Stroke::new(1.0, LINE_STRONG))
                .corner_radius(2.0f32)
                .inner_margin(egui::Margin::same(5))
                .show(ui, |ui| {
                    ui.set_width(232.0);
                    egui::ScrollArea::vertical()
                        .max_height(330.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for subject in &snap.timer.subjects {
                                let (row, resp) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), 30.0),
                                    Sense::click(),
                                );
                                let selected = Some(subject.id) == current_id;
                                if resp.hovered() {
                                    ui.painter().rect_filled(row, 2.0, SURFACE);
                                }
                                ui.painter().circle_filled(
                                    Pos2::new(row.min.x + 8.0, row.center().y),
                                    3.0,
                                    from_argb(subject.color_int),
                                );
                                ui.painter().text(
                                    Pos2::new(row.min.x + 19.0, row.center().y),
                                    Align2::LEFT_CENTER,
                                    &subject.title,
                                    if selected {
                                        font(12.5, fonts::UI_STRONG)
                                    } else {
                                        font(12.5, fonts::UI)
                                    },
                                    if selected { INK } else { INK_DIM },
                                );
                                let today = stats.ms_for(Some(subject.id), &subject.title);
                                if today > 0 {
                                    ui.painter().text(
                                        Pos2::new(row.max.x - 8.0, row.center().y),
                                        Align2::RIGHT_CENTER,
                                        format_hm(today),
                                        font(11.0, fonts::MONO),
                                        INK_FAINT,
                                    );
                                }
                                if resp.clicked() && enabled {
                                    app.send_timer(TimerCmd::SelectSubject(subject.id));
                                    app.all_subjects_rect = None;
                                }
                            }
                        });
                });
        });

    // A click anywhere outside dismisses it.
    let menu = area.response.rect;
    let clicked_outside = ui.input(|i| {
        i.pointer
            .any_click()
            .then(|| i.pointer.interact_pos())
            .flatten()
            .map(|p| !menu.contains(p) && !anchor.contains(p))
            .unwrap_or(false)
    });
    if clicked_outside {
        app.all_subjects_rect = None;
    }
}

fn controls(app: &mut MptApp, ui: &mut Ui, at: Pos2, snap: &Snap) {
    let t = &snap.timer;
    let busy = t.busy;
    let mut x = at.x;
    let y = at.y;

    if !t.is_running && !t.is_resting {
        let label = if t.session_elapsed_ms > 0 {
            "Resume"
        } else {
            "Start"
        };
        let enabled = t.current_subject.is_some() && !busy;
        if action_button(ui, &mut x, y, label, ButtonKind::Primary, enabled, "start") {
            app.send_timer(TimerCmd::Start);
        }
        if t.session_elapsed_ms > 0 {
            if action_button(
                ui,
                &mut x,
                y,
                "Save & finish",
                ButtonKind::Ghost,
                !busy,
                "finish",
            ) {
                app.send_timer(TimerCmd::Stop);
            }
        }
    } else {
        let label = if t.is_resting { "Resume" } else { "Pause" };
        if action_button(ui, &mut x, y, label, ButtonKind::Primary, !busy, "pause") {
            app.send_timer(if t.is_resting {
                TimerCmd::Start
            } else {
                TimerCmd::Pause
            });
        }
        if action_button(ui, &mut x, y, "Stop", ButtonKind::Accent, !busy, "stop") {
            app.send_timer(TimerCmd::Stop);
        }
        if busy {
            spinner(ui, Pos2::new(x + 7.0, y + BUTTON_HEIGHT / 2.0), 12.0, INK_FAINT);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ButtonKind {
    Primary,
    Ghost,
    Accent,
}

/// A 42 px action button; returns true on click.
fn action_button(
    ui: &mut Ui,
    x: &mut f32,
    y: f32,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
    id: &str,
) -> bool {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font(13.0, fonts::UI_STRONG), INK);
    let width = (galley.size().x + 52.0).max(96.0);
    let rect = Rect::from_min_size(Pos2::new(*x, y), Vec2::new(width, BUTTON_HEIGHT));
    let resp = ui.interact(rect, sid(("mpt.action", id)), Sense::click());
    let h = ui
        .ctx()
        .animate_bool_with_time(sid(("mpt.action.hover", id)), resp.hovered() && enabled, 0.12);

    let (bg, fg, border) = if !enabled {
        (SURFACE, INK_FAINT, None)
    } else {
        match kind {
            // The original only brightens on hover; no extra darkening while
            // the pointer is down.
            ButtonKind::Primary => (mix(SYNAPSE, Color32::WHITE, h), BACKGROUND, None),
            ButtonKind::Accent => (
                mix(Color32::TRANSPARENT, ACCENT, h),
                mix(ACCENT, INK, h),
                Some(ACCENT_SOFT),
            ),
            ButtonKind::Ghost => (
                mix(Color32::TRANSPARENT, SURFACE_RAISED, h),
                INK,
                Some(LINE_STRONG),
            ),
        }
    };

    ui.painter().rect_filled(rect, 2.0, bg);
    if let Some(border) = border {
        ui.painter()
            .rect_stroke(rect, 2.0, Stroke::new(1.0, border), StrokeKind::Inside);
    }
    // galley() only colours PLACEHOLDER runs — the ink baked in above would
    // win and leave a light label on the light primary slab. Override.
    ui.painter().galley_with_override_text_color(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );

    if enabled {
        resp.clone()
            .on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    *x += width + 10.0;
    resp.clicked() && enabled
}

fn notice_line(app: &mut MptApp, ui: &mut Ui, rect: Rect, notice: &str) {
    ui.painter().text(
        Pos2::new(rect.min.x, rect.min.y + 2.0),
        Align2::LEFT_TOP,
        "ⓘ",
        font(12.0, fonts::UI),
        INK_FAINT,
    );
    let galley = ui.painter().layout(
        notice.to_string(),
        font(12.0, fonts::UI),
        INK_DIM,
        rect.width() - 34.0,
    );
    ui.painter()
        .galley(Pos2::new(rect.min.x + 20.0, rect.min.y), galley, INK_DIM);

    let close = Rect::from_min_size(
        Pos2::new(rect.max.x - 14.0, rect.min.y),
        Vec2::new(14.0, 14.0),
    );
    let resp = ui.interact(close, sid("mpt.notice.close"), Sense::click());
    ui.painter().text(
        close.center(),
        Align2::CENTER_CENTER,
        "✕",
        font(11.0, fonts::UI),
        if resp.hovered() { INK } else { INK_FAINT },
    );
    if resp.clicked() {
        app.send_timer(TimerCmd::ClearNotice);
    }
}

// --------------------------------------------------------------- subject panel

fn subject_panel(app: &mut MptApp, ctx: &egui::Context, ui: &mut Ui, rect: Rect, snap: &Snap) {
    let inner = Rect::from_min_max(
        Pos2::new(rect.min.x + 20.0, rect.min.y + 26.0),
        Pos2::new(rect.max.x - 20.0, rect.max.y - 18.0),
    );
    let stats = snap.timer.display_stats();
    let idle = !snap.timer.is_running && !snap.timer.is_resting;
    let current_id = snap.timer.current_subject.as_ref().map(|s| s.id);

    let caps = ui
        .painter()
        .layout_job(caps_job("TODAY BY SUBJECT", INK_FAINT));
    ui.painter().galley(
        Pos2::new(inner.min.x, inner.min.y + caps_lift(caps.size().y, 11.0)),
        caps,
        INK_FAINT,
    );
    ui.painter().text(
        Pos2::new(inner.max.x, inner.min.y + 1.0),
        Align2::RIGHT_TOP,
        format_hm_compact(stats.total_ms),
        font(11.0, fonts::MONO),
        INK_FAINT,
    );

    // Reserve the footer and the offline strip before the list gets its height.
    let footer_h = 52.0;
    let offline_h = if snap.offline.has_pending() { 44.0 } else { 0.0 };
    let list = Rect::from_min_max(
        Pos2::new(inner.min.x, inner.min.y + 18.0),
        Pos2::new(inner.max.x, inner.max.y - footer_h - offline_h),
    );

    if stats.subjects.is_empty() {
        ui.painter().text(
            list.min,
            Align2::LEFT_TOP,
            "Nothing logged yet today.\nPress start and the hours appear here.",
            font(12.0, fonts::UI),
            INK_FAINT,
        );
    } else {
        let max_ms = stats.subjects.first().map(|s| s.ms).unwrap_or(0).max(1);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for row in &stats.subjects {
                        let (row_rect, resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), STAT_ROW_HEIGHT),
                            Sense::click(),
                        );
                        let h = ui.ctx().animate_bool_with_time(
                            sid(("mpt.row.hover", row.id, row.title.as_str())),
                            resp.hovered() && idle,
                            0.12,
                        );
                        if h > 0.002 {
                            ui.painter()
                                .rect_filled(row_rect, 2.0, mix(Color32::TRANSPARENT, SURFACE, h));
                        }

                        let colour = from_argb(row.color_int);
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                Pos2::new(row_rect.min.x + 8.0, row_rect.min.y + 6.0),
                                Vec2::new(2.0, 12.0),
                            ),
                            0.0,
                            colour,
                        );
                        ui.painter().text(
                            Pos2::new(row_rect.min.x + 19.0, row_rect.min.y + 6.0),
                            Align2::LEFT_TOP,
                            &row.title,
                            font(12.5, fonts::UI),
                            if idle && row.id == current_id {
                                INK
                            } else {
                                INK_DIM
                            },
                        );
                        ui.painter().text(
                            Pos2::new(row_rect.max.x - 8.0, row_rect.min.y + 6.0),
                            Align2::RIGHT_TOP,
                            format_hm(row.ms),
                            font(11.5, fonts::MONO),
                            INK,
                        );

                        // Hairline track with a coloured fill sized against the
                        // biggest subject of the day.
                        let track_y = row_rect.min.y + 27.0;
                        let x0 = row_rect.min.x + 8.0;
                        let x1 = row_rect.max.x - 8.0;
                        ui.painter().rect_filled(
                            Rect::from_min_max(
                                Pos2::new(x0, track_y),
                                Pos2::new(x1, track_y + 2.0),
                            ),
                            0.0,
                            LINE,
                        );
                        let target = (row.ms as f32 / max_ms as f32).clamp(0.0, 1.0);
                        let fraction = ui.ctx().animate_value_with_time(
                            sid(("mpt.bar", row.id, row.title.as_str())),
                            target,
                            0.45,
                        );
                        ui.painter().rect_filled(
                            Rect::from_min_max(
                                Pos2::new(x0, track_y),
                                Pos2::new(x0 + (x1 - x0) * fraction, track_y + 2.0),
                            ),
                            0.0,
                            colour,
                        );

                        if resp.clicked() && idle {
                            if let Some(id) = row.id {
                                app.send_timer(TimerCmd::SelectSubject(id));
                            }
                        }
                        if idle {
                            resp.clone()
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                        }
                    }
                });
        });
    }

    let mut y = list.max.y;
    if snap.offline.has_pending() {
        ui.painter().rect_filled(
            Rect::from_min_max(Pos2::new(inner.min.x, y), Pos2::new(inner.max.x, y + 1.0)),
            0.0,
            LINE,
        );
        y += 13.0;
        let count = snap.offline.pending_count();
        ui.painter().text(
            Pos2::new(inner.min.x, y),
            Align2::LEFT_TOP,
            format!("{count} session{} offline", if count == 1 { "" } else { "s" }),
            font(11.0, fonts::MONO),
            ACCENT,
        );
        let sync_rect = Rect::from_min_size(
            Pos2::new(inner.max.x - 58.0, y - 2.0),
            Vec2::new(58.0, 16.0),
        );
        let resp = ui.interact(sync_rect, sid("mpt.sync_now"), Sense::click());
        let label = if snap.offline.is_syncing {
            "syncing…"
        } else {
            "sync now"
        };
        ui.painter().text(
            Pos2::new(inner.max.x, y),
            Align2::RIGHT_TOP,
            label,
            font(11.0, fonts::MONO),
            if resp.hovered() && !snap.offline.is_syncing {
                Color32::WHITE
            } else {
                INK
            },
        );
        if resp.clicked() && !snap.offline.is_syncing {
            let _ = app.offline_tx.send(OfflineCmd::SyncNow);
        }
    }

    // Footer: account and sign-out.
    let footer_top = inner.max.y - 38.0;
    ui.painter().rect_filled(
        Rect::from_min_max(
            Pos2::new(inner.min.x, footer_top),
            Pos2::new(inner.max.x, footer_top + 1.0),
        ),
        0.0,
        LINE,
    );
    ui.painter().text(
        Pos2::new(inner.min.x, footer_top + 12.0),
        Align2::LEFT_TOP,
        &snap.user_name,
        font(13.0, fonts::UI),
        INK,
    );
    ui.painter().text(
        Pos2::new(inner.min.x, footer_top + 30.0),
        Align2::LEFT_TOP,
        "connected to YPT",
        font(11.0, fonts::MONO),
        INK_FAINT,
    );

    let logout = Rect::from_min_size(
        Pos2::new(inner.max.x - 22.0, footer_top + 12.0),
        Vec2::new(22.0, 22.0),
    );
    let resp = ui.interact(logout, sid("mpt.logout"), Sense::click());
    ui.painter().text(
        logout.center(),
        Align2::CENTER_CENTER,
        "⎋",
        font(14.0, fonts::UI),
        if resp.hovered() { INK } else { INK_FAINT },
    );
    resp.clone().on_hover_text("Sign out");
    if resp.clicked() {
        app.logout_requested = true;
    }

    update_button(app, ui, Rect::from_min_max(
        Pos2::new(inner.min.x + 180.0, footer_top + 12.0),
        Pos2::new(inner.max.x - 34.0, footer_top + 34.0),
    ));

    let _ = ctx;
}

/// Small "check for updates" control at the footer's right edge. Each state
/// paints itself and answers clicks; the heavy work lives in `updater`'s
/// worker threads, so this only reads the shared state.
fn update_button(app: &mut MptApp, ui: &mut Ui, rect: Rect) {
    let state = app.update.lock().unwrap().clone();
    // Repaint while the state can move on its own (spinner, progress).
    if matches!(state, crate::updater::UpdateState::Checking)
        || matches!(state, crate::updater::UpdateState::Downloading { .. })
    {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
    }

    let (label, text_color, clickable): (String, Color32, bool) = match &state {
        crate::updater::UpdateState::Idle => ("check for updates".into(), INK_FAINT, true),
        crate::updater::UpdateState::Checking => ("checking…".into(), INK_FAINT, false),
        crate::updater::UpdateState::UpToDate => ("up to date".into(), INK_FAINT, true),
        crate::updater::UpdateState::Available { version, .. } => {
            (format!("update to {version}"), ACCENT, true)
        }
        crate::updater::UpdateState::Downloading { progress } => (
            format!("updating… {}", (progress * 100.0) as u32),
            INK_DIM,
            false,
        ),
        crate::updater::UpdateState::Ready => ("restart to finish".into(), ACCENT, true),
        crate::updater::UpdateState::Failed(_) => ("retry update".into(), ACCENT, true),
    };

    let resp = ui.interact(rect, sid("mpt.update"), Sense::click());
    let hover = resp.hovered() && clickable;
    ui.painter().text(
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        label,
        font(11.0, fonts::MONO),
        if hover { Color32::WHITE } else { text_color },
    );
    if let crate::updater::UpdateState::Downloading { progress } = &state {
        // 2 px progress line under the label, like the day bars.
        let w = rect.width() * progress.clamp(0.0, 1.0);
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(rect.right() - w, rect.max.y + 2.0), Vec2::new(w, 2.0)),
            0.0,
            ACCENT,
        );
    }
    if hover {
        if let crate::updater::UpdateState::Failed(e) = &state {
            resp.clone().on_hover_text(e.clone());
        } else if let crate::updater::UpdateState::Available { size, .. } = &state {
            resp.clone()
                .on_hover_text(format!("{:.1} MB", *size as f32 / 1_048_576.0));
        }
    }

    if !resp.clicked() || !clickable {
        return;
    }
    match state {
        crate::updater::UpdateState::Idle
        | crate::updater::UpdateState::UpToDate
        | crate::updater::UpdateState::Failed(_) => {
            crate::updater::check(app.update.clone());
        }
        crate::updater::UpdateState::Available { url, .. } => {
            crate::updater::download(app.update.clone(), url);
        }
        crate::updater::UpdateState::Ready => {
            crate::updater::apply(app.update.clone());
        }
        crate::updater::UpdateState::Checking | crate::updater::UpdateState::Downloading { .. } => {}
    }
}

// ----------------------------------------------------------------- mini player

fn mini_body(app: &mut MptApp, ui: &mut Ui, snap: &Snap) {
    let body = ui.available_rect_before_wrap();
    if body.width() < 40.0 || body.height() < 30.0 {
        return;
    }
    // Flutter: Padding LTRB(16, 10, 12, 12).
    let inner = Rect::from_min_max(
        Pos2::new(body.min.x + 16.0, body.min.y + 10.0),
        Pos2::new(body.max.x - 12.0, body.max.y - 12.0),
    );
    let t = &snap.timer;
    let running = t.is_running;
    let resting = t.is_resting;
    let busy = t.busy;

    // Two 36 px actions on the right, vertically centred.
    let btn_y = inner.center().y - 18.0;
    let play_rect = Rect::from_min_size(Pos2::new(inner.max.x - 72.0, btn_y), Vec2::splat(36.0));
    let stop_rect = Rect::from_min_size(Pos2::new(inner.max.x - 36.0, btn_y), Vec2::splat(36.0));

    // Subject line, ellipsized to the room left of the actions like Flutter's
    // Flexible text inside the Expanded column.
    let text_x = inner.min.x + if running { 4.0 + 6.0 } else { 0.0 };
    let max_text_w = (inner.max.x - 72.0 - 12.0 - text_x).max(40.0);
    let title = t
        .current_subject
        .as_ref()
        .map(|s| s.title.clone())
        .unwrap_or_else(|| "No subject".to_string());
    let chars: Vec<char> = title.chars().collect();
    let mut subject_galley = ui
        .painter()
        .layout_no_wrap(title.clone(), font(11.0, fonts::UI), INK_DIM);
    if subject_galley.size().x > max_text_w && chars.len() > 1 {
        let mut n = chars.len();
        while n > 1 {
            n -= 1;
            let shorter: String = chars[..n].iter().collect::<String>() + "…";
            let g = ui
                .painter()
                .layout_no_wrap(shorter, font(11.0, fonts::UI), INK_DIM);
            if g.size().x <= max_text_w {
                subject_galley = g;
                break;
            }
        }
    }

    // The clock, tightened by the original's -1.5 letter spacing.
    let mut clock_job = egui::text::LayoutJob::simple(
        format_clock(t.session_elapsed_ms),
        font(34.0, fonts::MONO),
        if running { INK } else { INK_DIM },
        f32::INFINITY,
    );
    clock_job.sections[0].format.extra_letter_spacing = -1.5;
    let clock_galley = ui.painter().layout_job(clock_job);

    // Text column centred vertically (Flutter's MainAxisAlignment.center):
    // subject row, 2 px gap, clock.
    let block_h = subject_galley.size().y + 2.0 + clock_galley.size().y;
    let block_top = inner.center().y - block_h * 0.5;

    if running {
        // 4 px accent dot, centred on the subject line, 6 px before the text.
        ui.painter().circle_filled(
            Pos2::new(inner.min.x + 2.0, block_top + subject_galley.size().y * 0.5),
            2.0,
            ACCENT,
        );
    }
    ui.painter()
        .galley(Pos2::new(text_x, block_top), subject_galley.clone(), INK_DIM);
    ui.painter().galley(
        Pos2::new(inner.min.x, block_top + subject_galley.size().y + 2.0),
        clock_galley,
        if running { INK } else { INK_DIM },
    );

    // Starting needs a subject; pausing and stopping always work.
    let play_enabled = !busy && (running || resting || t.current_subject.is_some());
    let play_resp = ui.interact(play_rect, sid("mpt.mini.play"), Sense::click());
    if play_resp.hovered() && play_enabled {
        ui.painter().rect_filled(play_rect, 2.0, LINE);
    }
    // Drawn rather than typed: no font can turn these into boxes.
    let icon = if play_enabled { INK } else { INK_FAINT };
    let c = play_rect.center();
    if resting || !running {
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(c.x - 4.0, c.y - 6.0),
                Pos2::new(c.x - 4.0, c.y + 6.0),
                Pos2::new(c.x + 6.0, c.y),
            ],
            icon,
            Stroke::NONE,
        ));
    } else {
        for dx in [-4.0, 1.5] {
            ui.painter().rect_filled(
                Rect::from_min_size(Pos2::new(c.x + dx, c.y - 6.0), Vec2::new(2.5, 12.0)),
                0.0,
                icon,
            );
        }
    }
    if play_resp.clicked() && play_enabled {
        app.send_timer(if resting || !running {
            TimerCmd::Start
        } else {
            TimerCmd::Pause
        });
    }

    let stop_enabled = !busy && (running || resting);
    let stop_resp = ui.interact(stop_rect, sid("mpt.mini.stop"), Sense::click());
    if stop_resp.hovered() && stop_enabled {
        ui.painter().rect_filled(stop_rect, 2.0, ACCENT_SOFT);
    }
    ui.painter().rect_filled(
        Rect::from_center_size(stop_rect.center(), Vec2::splat(10.0)),
        0.0,
        if stop_enabled { ACCENT } else { INK_FAINT },
    );
    if stop_resp.clicked() && stop_enabled {
        app.send_timer(TimerCmd::Stop);
    }
}

fn from_argb(argb: i64) -> Color32 {
    crate::theme::colors::from_argb_int(argb)
}
