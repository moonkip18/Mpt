//! Frameless chrome, boot, and login. Same hierarchy and metrics as the
//! Flutter screens this replaced.

pub mod study;

use eframe::egui::{
    self, Align2, Color32, Id, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use egui::text::LayoutJob;

use crate::auth::AuthCmd;
use crate::theme::colors::*;
use crate::theme::{fonts, named};
use crate::MptApp;

const ICON_MINIMIZE: &str = "–";
const ICON_MAXIMIZE: &str = "□";
const ICON_RESTORE: &str = "❐";
const ICON_CLOSE: &str = "✕";
const ICON_PIP: &str = "⧉";
const ICON_EXPAND: &str = "⛶";

/// Stable widget id from a name. `Id::new` is deprecated in egui in favour of
/// salts; a global name is exactly what a fixed chrome button needs.
#[allow(deprecated)]
fn sid(name: impl std::hash::Hash + std::fmt::Debug) -> Id {
    Id::new(name)
}

pub fn font(size: f32, family: &str) -> egui::FontId {
    egui::FontId::new(size, named(family))
}

pub fn hairline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, LINE);
}

pub fn dot(ui: &mut Ui, diameter: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
    ui.painter().circle_filled(rect.center(), diameter / 2.0, color);
}

/// A small rotating arc, standing in for Flutter's `CircularProgressIndicator`.
pub fn spinner(ui: &mut Ui, center: Pos2, size: f32, color: Color32) {
    let radius = (size / 2.0 - 1.0).max(2.0);
    let time = ui.input(|i| i.time) as f32;
    let start = time * std::f32::consts::TAU;
    let sweep = 2.2_f32;
    let steps = 12;
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start + sweep * i as f32 / steps as f32;
            center + Vec2::new(a.cos(), a.sin()) * radius
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(1.6, color)));
}

/// A 40×38 chrome button painted into `rect`; close gets the red hover.
fn chrome_button(ui: &mut Ui, rect: Rect, id: Id, icon: &str, tooltip: &str, danger: bool) -> bool {
    let resp = ui.interact(rect, id, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(id.with("hover"), resp.hovered(), 0.12);
    if hover > 0.002 {
        let target = if danger { ACCENT_SOFT } else { LINE };
        ui.painter()
            .rect_filled(rect, 0.0, mix(Color32::TRANSPARENT, target, hover));
        resp.clone().on_hover_text(tooltip);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        font(15.0, fonts::UI),
        mix(INK_DIM, INK, hover),
    );
    resp.clicked()
}

/// Collected from the title bar and applied by the caller, because the bar
/// cannot borrow app state while it paints.
#[derive(Default)]
pub struct TitleActions {
    pub minimize: bool,
    pub maximize: bool,
    pub unmaximize: bool,
    pub close: bool,
    pub toggle_mini: bool,
}

impl TitleActions {
    pub fn apply(&self, app: &mut MptApp, ctx: &egui::Context) {
        if self.minimize {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        if self.maximize {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        if self.unmaximize {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        }
        if self.close {
            app.begin_close(ctx);
        }
        if self.toggle_mini {
            app.toggle_mini(ctx);
        }
    }
}

/// 38 px frameless bar: drag area and wordmark on the left, window buttons on
/// the right.
pub fn title_bar(
    ctx: &egui::Context,
    ui: &mut Ui,
    logo: Option<&egui::TextureHandle>,
    show_mini_toggle: bool,
    is_mini: bool,
) -> TitleActions {
    let mut actions = TitleActions::default();
    let height = 38.0;
    let button_count = if show_mini_toggle { 5 } else { 4 };
    let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    let buttons_left = bar.max.x - 4.0 - button_count as f32 * 40.0;

    // Drag area spans everything left of the buttons.
    let drag_rect = Rect::from_min_max(bar.min, Pos2::new(buttons_left - 8.0, bar.max.y));
    let drag = ui.interact(drag_rect, sid("mpt.title.drag"), Sense::click_and_drag());
    if drag.dragged() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }

    let mut x = bar.min.x + 16.0;
    if let Some(logo) = logo {
        let rect = Rect::from_min_size(
            Pos2::new(x, bar.min.y + (height - 17.0) / 2.0),
            Vec2::splat(17.0),
        );
        ui.painter().image(
            logo.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        x += 17.0 + 10.0;
    }
    ui.painter().text(
        Pos2::new(x, bar.center().y),
        Align2::LEFT_CENTER,
        "MPT",
        font(11.0, fonts::UI),
        INK_DIM,
    );

    // Right-aligned: close, maximize/restore, minimize, then the mini player.
    let mut right = bar.max.x - 4.0;
    let button = |icon: &str, tip: &str, danger: bool, ui: &mut Ui, right: &mut f32| {        let rect = Rect::from_min_size(Pos2::new(*right - 40.0, bar.min.y), Vec2::new(40.0, height));
        *right -= 40.0;
        chrome_button(ui, rect, sid(("mpt.title.btn", tip)), icon, tip, danger)
    };

    if button(ICON_CLOSE, "Close", true, ui, &mut right) {
        actions.close = true;
    }
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let max_icon = if maximized { ICON_RESTORE } else { ICON_MAXIMIZE };
    if button(max_icon, "Maximize", false, ui, &mut right) {
        if maximized {
            actions.unmaximize = true;
        } else {
            actions.maximize = true;
        }
    }
    if button(ICON_MINIMIZE, "Minimize", false, ui, &mut right) {
        actions.minimize = true;
    }
    if show_mini_toggle {
        let (icon, tip) = if is_mini {
            (ICON_EXPAND, "Back to full size")
        } else {
            (ICON_PIP, "Mini player")
        };
        if button(icon, tip, false, ui, &mut right) {
            actions.toggle_mini = true;
        }
    }

    actions
}

// ---------------------------------------------------------------- boot/login

pub fn boot_screen(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size_before_wrap(), Sense::hover());
    spinner(ui, rect.center(), 16.0, INK_FAINT);
}

/// Login: title bar, ground gradient, a centered 360 px column.
pub fn login_screen(app: &mut MptApp, ctx: &egui::Context, ui: &mut Ui) {
    let actions = title_bar(ctx, ui, app.logo.as_ref(), false, false);
    hairline(ui);

    let body_size = ui.available_size_before_wrap();
    let (body_rect, _) = ui.allocate_exact_size(body_size, Sense::hover());

    let auth_loading = app.shared.lock().unwrap().auth.is_loading;
    let error = app
        .local_error
        .clone()
        .or_else(|| app.shared.lock().unwrap().auth.error_message.clone());

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(body_rect)
            .layout(Layout::top_down(egui::Align::Center)),
        |ui| login_form(app, ui, auth_loading, error.as_deref()),
    );

    actions.apply(app, ctx);
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    mix(a, b, t)
}

/// Interpolate two colours, alpha included, so hover states can fade instead of
/// snapping on.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// The centered 360 px column, with the same vertical rhythm as the original.
fn login_form(app: &mut MptApp, ui: &mut Ui, busy: bool, error: Option<&str>) {
    let width = 360.0f32.min(ui.available_width());
    let total_h: f32 = 40.0
        + 20.0
        + 28.0
        + 42.0
        + 10.0
        + 42.0
        + 32.0
        + 46.0
        + 12.0
        + 46.0
        + if error.is_some() { 14.0 + 20.0 } else { 0.0 }
        + 38.0
        + 46.0
        + if app.google_enabled { 10.0 + 46.0 } else { 0.0 }
        + 26.0
        + 40.0;

    let avail = ui.available_height();
    let top = if total_h < avail {
        (avail - total_h) * 0.35
    } else {
        12.0
    };
    let mut y = ui.cursor().min.y + top;
    let x0 = ui.cursor().min.x + (ui.available_width() - width) / 2.0;
    let mut rect = |dy: f32, h: f32| {
        y += dy;
        let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(width, h));
        y += h;
        r
    };

    // Mark + wordmark.
    let logo_rect = rect(40.0, 20.0);
    ui.scope_builder(egui::UiBuilder::new().max_rect(logo_rect), |ui| {
        ui.horizontal(|ui| {
            if let Some(logo) = &app.logo {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(
                    logo.id(),
                    Vec2::splat(20.0),
                )));
            }
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("MPT")
                    .font(font(11.0, fonts::UI))
                    .color(INK_DIM),
            );
        });
    });

    // Headline and sub-copy.
    let head_rect = rect(28.0, 42.0);
    ui.painter().text(
        Pos2::new(head_rect.min.x, head_rect.min.y),
        Align2::LEFT_TOP,
        "Sign in to YPT",
        font(38.0, fonts::DISPLAY),
        INK,
    );
    let sub_rect = rect(10.0, 42.0);
    let sub_job = LayoutJob::simple(
        "Your timer and subjects stay in sync with the phone app. \
         Time studied here is written to the same account."
            .to_string(),
        font(13.0, fonts::UI),
        INK_DIM,
        sub_rect.width(),
    );
    let galley = ui.painter().layout_job(sub_job);
    ui.painter().galley(sub_rect.min, galley, INK_DIM);

    // E-mail.
    let email_rect = rect(32.0, 46.0);
    let mut submit = false;
    let email_resp = put_field(
        ui,
        email_rect,
        "E-mail",
        &mut app.email,
        false,
        busy,
        sid("mpt.login.email"),
    );
    if email_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        submit = true;
    }

    // Password, with the reveal eye on the right edge.
    let pw_rect = rect(12.0, 46.0);
    let pw_resp = put_field(
        ui,
        pw_rect,
        "Password",
        &mut app.password,
        app.obscure_password,
        busy,
        sid("mpt.login.password"),
    );
    if pw_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        submit = true;
    }
    let eye_rect = Rect::from_min_max(
        Pos2::new(pw_rect.max.x - 36.0, pw_rect.min.y),
        pw_rect.max,
    );
    let eye_resp = ui.interact(eye_rect, sid("mpt.login.reveal"), Sense::click());
    if eye_resp.hovered() {
        eye_resp.clone().on_hover_text("Show password");
        eye_resp.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let eye = if app.obscure_password { "👁" } else { "🙈" };
    ui.painter().text(
        eye_rect.center(),
        Align2::CENTER_CENTER,
        eye,
        font(15.0, fonts::UI),
        INK_FAINT,
    );
    if eye_resp.clicked() {
        app.obscure_password = !app.obscure_password;
    }

    // Error line: accent dot + copy.
    if let Some(err) = error {
        let err_rect = rect(14.0, 20.0);
        let dot_rect = Rect::from_min_size(
            Pos2::new(err_rect.min.x + 5.0, err_rect.min.y + 6.0),
            Vec2::splat(4.0),
        );
        ui.painter().circle_filled(dot_rect.center(), 2.0, ACCENT);
        let galley = ui.painter().layout(
            err.to_string(),
            font(12.0, fonts::UI),
            ACCENT,
            err_rect.width() - 17.0,
        );
        ui.painter().galley(
            Pos2::new(err_rect.min.x + 17.0, err_rect.min.y + 3.0),
            galley,
            ACCENT,
        );
    }

    // Primary action.
    let btn_rect = rect(38.0, 46.0);
    let btn_resp = ui.interact(btn_rect, sid("mpt.login.submit"), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(
        sid("mpt.login.submit.hover"),
        btn_resp.hovered() && !busy,
        0.12,
    );
    let down = btn_resp.is_pointer_button_down_on() && !busy;
    ui.painter().rect_filled(
        btn_rect,
        3.0,
        mix(SYNAPSE, Color32::WHITE, if down { 0.85 } else { hover * 0.45 }),
    );
    if !busy {
        btn_resp.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    if busy {
        spinner(ui, btn_rect.center(), 14.0, BACKGROUND);
    } else {
        ui.painter().text(
            btn_rect.center(),
            Align2::CENTER_CENTER,
            "Sign in",
            font(13.0, fonts::UI),
            BACKGROUND,
        );
    }
    if btn_resp.clicked() && !busy {
        submit = true;
    }

    // Ghost Google button, only when a client id is configured.
    if app.google_enabled {
        let g_rect = rect(10.0, 46.0);
        let g_resp = ui.interact(g_rect, sid("mpt.login.google"), Sense::click());
        let g_hover = ui.ctx().animate_bool_with_time(
            sid("mpt.login.google.hover"),
            g_resp.hovered() && !busy,
            0.12,
        );
        ui.painter().rect_filled(
            g_rect,
            3.0,
            mix(Color32::TRANSPARENT, SURFACE_RAISED, g_hover),
        );
        g_resp.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.painter()
            .rect_stroke(g_rect, 3.0, Stroke::new(1.0, LINE_STRONG), StrokeKind::Inside);
        ui.painter().text(
            g_rect.center(),
            Align2::CENTER_CENTER,
            "Continue with Google",
            font(13.0, fonts::UI),
            INK,
        );
        if g_resp.clicked() && !busy {
            let _ = app.auth_tx.send(AuthCmd::SignInGoogle);
        }
    }

    // Footer note.
    let foot_rect = rect(26.0, 40.0);
    let foot_job = LayoutJob::simple(
        "This is an unofficial desktop client. Use the same account \
         you already have on Yeolpumta."
            .to_string(),
        font(11.0, fonts::UI),
        INK_FAINT,
        foot_rect.width(),
    );
    let galley = ui.painter().layout_job(foot_job);
    ui.painter().galley(foot_rect.min, galley, INK_FAINT);

    if submit {
        submit_login(app);
    }
}

/// A 46 px single-line field: filled surface, hairline border, 3 px radius.
fn put_field(
    ui: &mut Ui,
    rect: Rect,
    hint: &str,
    value: &mut String,
    password: bool,
    enabled: bool,
    id: Id,
) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(3.0f32)
        .inner_margin(egui::Margin::symmetric(14, 14));
    let mut edit = egui::TextEdit::singleline(value)
        .id(id)
        .hint_text(hint)
        .password(password)
        .frame(frame)
        .desired_width(f32::INFINITY);
    if !enabled {
        edit = edit.lock_focus(true);
    }
    let resp = ui.put(rect, edit);
    // Hairline only while the field is focused — the frame's own stroke is the
    // resting look, same as the Flutter fields this mirrors.
    if resp.has_focus() {
        ui.painter()
            .rect_stroke(rect, 3.0, Stroke::new(1.0, INK_FAINT), StrokeKind::Inside);
    }
    resp
}

pub fn submit_login(app: &mut MptApp) {
    let email = app.email.trim().to_string();
    let password = app.password.clone();
    if email.is_empty() || !email.contains('@') {
        app.local_error = Some("Enter the e-mail used in Yeolpumta.".into());
        return;
    }
    if password.is_empty() {
        app.local_error = Some("Enter your password.".into());
        return;
    }
    app.local_error = None;
    let _ = app.auth_tx.send(AuthCmd::SignInEmail { email, password });
}
