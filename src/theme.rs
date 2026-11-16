//! Palette, type scale and the egui style Mpt runs on.
//!
//! Near-black ground, hairline rules, cool ink, a single muted red accent.
//! System faces only (Segoe UI / Georgia / Consolas), so the first frame is
//! instant and nothing is downloaded.

use egui::text::{ByteIndex, LayoutJob, LayoutSection, TextFormat};
use egui::{Color32, FontFamily, FontId, Stroke};

pub mod colors {
    use egui::Color32;

    pub const BACKGROUND: Color32 = Color32::from_rgb(0x05, 0x05, 0x06);
    pub const SURFACE: Color32 = Color32::from_rgb(0x0A, 0x0B, 0x0D);
    pub const SURFACE_RAISED: Color32 = Color32::from_rgb(0x0D, 0x0F, 0x12);
    pub const INK: Color32 = Color32::from_rgb(0xE9, 0xEA, 0xEC);
    pub const INK_DIM: Color32 = Color32::from_rgb(0x8B, 0x8F, 0x97);
    pub const INK_FAINT: Color32 = Color32::from_rgb(0x56, 0x5A, 0x61);
    pub const LINE: Color32 = Color32::from_rgba_unmultiplied_const(0xFF, 0xFF, 0xFF, 0x14);
    pub const LINE_STRONG: Color32 = Color32::from_rgba_unmultiplied_const(0xFF, 0xFF, 0xFF, 0x24);
    pub const SYNAPSE: Color32 = Color32::from_rgb(0xDF, 0xE3, 0xE8);
    pub const ACCENT: Color32 = Color32::from_rgb(0xC8, 0x43, 0x3C);
    pub const ACCENT_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(0xC8, 0x43, 0x3C, 0x66);
    pub const OK: Color32 = Color32::from_rgb(0x6E, 0x8F, 0x72);
    pub const SCROLL_THUMB: Color32 = Color32::from_rgb(0x24, 0x29, 0x33);

    /// YPT sends subject colours as ARGB ints; a missing colour falls back to
    /// the same violet Desky used.
    pub fn from_argb_int(argb: i64) -> Color32 {
        if argb == 0 {
            return Color32::from_rgb(0xA7, 0x8B, 0xFA);
        }
        let a = ((argb >> 24) & 0xFF) as u8;
        let r = ((argb >> 16) & 0xFF) as u8;
        let g = ((argb >> 8) & 0xFF) as u8;
        let b = (argb & 0xFF) as u8;
        // Zero alpha means "no colour was chosen", not "invisible".
        let a = if a == 0 { 255 } else { a };
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }
}

pub mod fonts {
    pub const UI: &str = "Segoe UI";
    pub const DISPLAY: &str = "Georgia";
    pub const MONO: &str = "Consolas";
}

pub fn named(family: &str) -> FontFamily {
    FontFamily::Name(std::sync::Arc::from(family))
}

/// Text roles, one per place the layout needs a deliberate size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MptText {
    Hero,
    HeroSmall,
    Section,
    Body,
    BodyDim,
    Label,
    Numeric,
    NumericLarge,
}

impl MptText {
    pub fn font_id(self) -> FontId {
        use fonts::*;
        match self {
            MptText::Hero => FontId::new(96.0, named(DISPLAY)),
            MptText::HeroSmall => FontId::new(42.0, named(DISPLAY)),
            MptText::Section => FontId::new(11.0, named(UI)),
            MptText::Body => FontId::new(13.0, named(UI)),
            MptText::BodyDim => FontId::new(12.0, named(UI)),
            MptText::Label => FontId::new(11.0, named(MONO)),
            MptText::Numeric => FontId::new(13.0, named(MONO)),
            MptText::NumericLarge => FontId::new(22.0, named(MONO)),
        }
    }

    pub fn color(self) -> Color32 {
        use colors::*;
        match self {
            MptText::Section | MptText::BodyDim | MptText::Label => INK_FAINT,
            _ => INK,
        }
    }
}

/// Apply the palette and the widget strokes to the context.
pub fn apply(ctx: &egui::Context) {
    use colors::*;

    ctx.set_theme(egui::ThemePreference::Dark);

    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = SURFACE_RAISED;
    style.visuals.extreme_bg_color = SURFACE;
    style.visuals.faint_bg_color = SURFACE;
    style.visuals.override_text_color = Some(INK);
    style.visuals.selection.bg_fill = SYNAPSE;
    style.visuals.selection.stroke = Stroke::new(1.0, BACKGROUND);
    style.visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    style.visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE_STRONG);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, INK);
    style.visuals.widgets.hovered.bg_fill = SURFACE_RAISED;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, LINE_STRONG);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, INK);
    style.visuals.widgets.active.bg_fill = SURFACE_RAISED;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, SYNAPSE);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, INK);
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = false;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);

    let _ = ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);

    ctx.set_fonts(system_fonts());
}

/// Register the three system families, falling back to egui's bundled faces
/// when a Windows font file cannot be read.
fn system_fonts() -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();

    let candidates: [(&str, &[&str]); 3] = [
        (fonts::UI, &["segoeui.ttf", "seguisb.ttf"]),
        (fonts::DISPLAY, &["georgia.ttf", "georgiai.ttf"]),
        (fonts::MONO, &["consola.ttf"]),
    ];

    for (family, files) in candidates {
        for file in files {
            let Ok(bytes) = std::fs::read(format!("C:\\Windows\\Fonts\\{file}")) else {
                continue;
            };
            defs.font_data
                .insert(file.to_string(), egui::FontData::from_owned(bytes).into());
            defs.families
                .entry(named(family))
                .or_default()
                .push(file.to_string());
            break;
        }
    }

    // A registered family must own at least one font, or egui panics the first
    // time it lays out text. Borrow the bundled proportional face if no system
    // file was found.
    let fallback = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, _) in candidates {
        defs.families
            .entry(named(family))
            .or_insert_with(|| fallback.clone());
    }

    defs
}

/// A single-run text job.
pub fn text_job(text: impl Into<String>, font: FontId, color: Color32) -> LayoutJob {
    LayoutJob::simple(text.into(), font, color, f32::INFINITY)
}

/// A text job with an explicit run, for places that need a section rather than
/// a whole-string format.
pub fn styled_job(text: String, font: FontId, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.text = text;
    job.sections.push(LayoutSection {
        leading_space: 0.0,
        byte_range: ByteIndex(0)..ByteIndex(job.text.len()),
        format: TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    });
    job
}
