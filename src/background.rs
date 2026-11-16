//! The backdrop.
//!
//! Drew this from the personal-site canvas in `Documents/Default Project/index.html`:
//! a drifting field of points with hairline links, a soft lift of light behind
//! the focal area, and a vignette. The site runs it loud because it *is* the
//! site; here the app is a timer, so it runs at a whisper — enough that the
//! window does not look like a flat black rectangle, never enough to compete
//! with the clock. Behind the study screen it is dimmed further still.

use eframe::egui::{self, Color32, Pos2, Stroke, Ui, Vec2};

const COUNT: usize = 44;
const LINK_DISTANCE: f32 = 132.0;

struct Mote {
    /// Normalised 0..1 so a resize does not teleport the field.
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    size: f32,
    alpha: u8,
}

pub struct Background {
    motes: Vec<Mote>,
    last_time: f64,
}

impl Background {
    pub fn new() -> Self {
        // Small LCG: repeatable layout, no dependency, no OS entropy needed.
        let mut seed: u32 = 0x9E37_79B9;
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / 16_777_216.0
        };

        let motes = (0..COUNT)
            .map(|_| {
                let (x, y) = (next(), next());
                let angle = next() * std::f32::consts::TAU;
                let speed = 0.0016 + next() * 0.0026;
                Mote {
                    x,
                    y,
                    vx: angle.cos() * speed,
                    vy: angle.sin() * speed,
                    size: 0.9 + next() * 1.1,
                    alpha: 10 + (next() * 16.0) as u8,
                }
            })
            .collect();

        Self { motes, last_time: 0.0 }
    }

    fn advance(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        for m in &mut self.motes {
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            // Wrap rather than bounce: a bounce reads as a hard edge.
            if m.x < -0.05 {
                m.x += 1.1;
            } else if m.x > 1.05 {
                m.x -= 1.1;
            }
            if m.y < -0.05 {
                m.y += 1.1;
            } else if m.y > 1.05 {
                m.y -= 1.1;
            }
        }
    }

    /// Painted before the screen content; `settled` is true once someone is
    /// signed in, where the field steps back.
    pub fn paint(&mut self, ui: &mut Ui, settled: bool) {
        let rect = ui.max_rect();
        if rect.width() < 2.0 || rect.height() < 2.0 {
            return;
        }
        let time = ui.input(|i| i.time);
        let dt = (time - self.last_time) as f32;
        self.last_time = time;
        self.advance(dt);

        let painter = ui.painter();
        let dim = if settled { 0.42 } else { 1.0 };

        // A soft lift behind the upper-middle, where the clock and headline sit.
        // Concentric translucent discs approximate a radial gradient cheaply.
        let glow_center = Pos2::new(rect.center().x, rect.min.y + rect.height() * 0.34);
        let base = (rect.width().min(rect.height()) * 0.55).max(120.0);
        for ring in (1..=7).rev() {
            let r = base * ring as f32 / 7.0;
            let alpha = (2.6 * (1.0 - ring as f32 / 8.0) * dim).max(0.0);
            painter.circle_filled(
                glow_center,
                r,
                Color32::from_rgba_unmultiplied(0x9F, 0xD6, 0xFF, alpha as u8),
            );
        }

        let pos: Vec<Pos2> = self
            .motes
            .iter()
            .map(|m| {
                let p = Pos2::new(
                    rect.min.x + m.x * rect.width(),
                    rect.min.y + m.y * rect.height(),
                );
                p
            })
            .collect();

        // Hairline links first, so dots sit on top of them.
        for i in 0..pos.len() {
            for j in (i + 1)..pos.len() {
                let d = pos[i].distance(pos[j]);
                if d > LINK_DISTANCE || d < 1.0 {
                    continue;
                }
                let falloff = 1.0 - d / LINK_DISTANCE;
                let alpha = (falloff * falloff * 13.0 * dim) as u8;
                if alpha == 0 {
                    continue;
                }
                painter.line_segment(
                    [pos[i], pos[j]],
                    Stroke::new(0.6, Color32::from_rgba_unmultiplied(0xDF, 0xE3, 0xE8, alpha)),
                );
            }
        }

        for (m, p) in self.motes.iter().zip(pos.iter()) {
            let alpha = (m.alpha as f32 * dim) as u8;
            if alpha == 0 {
                continue;
            }
            painter.circle_filled(
                *p,
                m.size,
                Color32::from_rgba_unmultiplied(0xDF, 0xE3, 0xE8, alpha),
            );
        }

        // Vignette: darken the outer band in thin strips (cheap, no shader).
        let strips = 14;
        let band = Vec2::new(rect.width() * 0.16, rect.height() * 0.16);
        for i in 0..strips {
            let t = i as f32 / strips as f32;
            let alpha = (t * 30.0) as u8;
            if alpha == 0 {
                continue;
            }
            let shade = Color32::from_rgba_unmultiplied(0x05, 0x05, 0x06, alpha);
            let inset = band * t;
            let frame = egui::Rect::from_min_size(rect.min, rect.size());
            // Top and bottom edges.
            painter.rect_filled(
                egui::Rect::from_min_size(frame.min, Vec2::new(frame.width(), inset.y.max(1.0))),
                0.0,
                shade,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(
                    Pos2::new(frame.min.x, frame.max.y - inset.y.max(1.0)),
                    Vec2::new(frame.width(), inset.y.max(1.0)),
                ),
                0.0,
                shade,
            );
        }
    }
}
