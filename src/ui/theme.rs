//! Identidade visual: cinza escuro + neon (verde fluorescente por padrao; 8 cores a escolher).
//!
//! Todas as cores da interface vem daqui. O "neon" e feito com sombras coloridas sem
//! deslocamento (o egui desenha a sombra borrada) e texto repintado com halo transparente.

use std::sync::atomic::{AtomicUsize, Ordering};

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Rect, Shadow, Stroke,
    Vec2, vec2,
};

// ---- paleta ----
pub const BG: Color32 = Color32::from_rgb(0x0A, 0x0C, 0x0E);
pub const PANEL: Color32 = Color32::from_rgb(0x12, 0x15, 0x18);
pub const ITEM: Color32 = Color32::from_rgb(0x19, 0x1D, 0x21);
pub const ITEM_HOVER: Color32 = Color32::from_rgb(0x21, 0x27, 0x2C);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x2D, 0x33);
pub const TEXT: Color32 = Color32::from_rgb(0xE3, 0xF0, 0xE6);
pub const MUTED: Color32 = Color32::from_rgb(0x84, 0x93, 0x8A);
pub const DIM: Color32 = Color32::from_rgb(0x41, 0x4B, 0x46);
pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x3B, 0x5C);
pub const DANGER_DEEP: Color32 = Color32::from_rgb(0x2B, 0x0E, 0x14);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xD2, 0x3F);

pub const RADIUS_CARD: u8 = 18;

// ---- cor do neon (escolhida pelo usuario) ----

/// Uma cor de neon: o tubo (cor forte) e o nucleo claro (o "filamento").
pub struct Accent {
    /// Nome na config (`"accent"`) e chave de traducao `accent_<key>`.
    pub key: &'static str,
    pub neon: Color32,
    pub core: Color32,
}

pub const ACCENTS: [Accent; 8] = [
    Accent { key: "green", neon: Color32::from_rgb(0x39, 0xFF, 0x14), core: Color32::from_rgb(0xC8, 0xFF, 0xB8) },
    Accent { key: "cyan", neon: Color32::from_rgb(0x00, 0xE5, 0xFF), core: Color32::from_rgb(0xC4, 0xF9, 0xFF) },
    Accent { key: "blue", neon: Color32::from_rgb(0x2E, 0x7D, 0xFF), core: Color32::from_rgb(0xC6, 0xDC, 0xFF) },
    Accent { key: "purple", neon: Color32::from_rgb(0xB0, 0x3C, 0xFF), core: Color32::from_rgb(0xE8, 0xCC, 0xFF) },
    Accent { key: "pink", neon: Color32::from_rgb(0xFF, 0x2B, 0xD6), core: Color32::from_rgb(0xFF, 0xC8, 0xF3) },
    Accent { key: "red", neon: Color32::from_rgb(0xFF, 0x2E, 0x4D), core: Color32::from_rgb(0xFF, 0xC6, 0xCE) },
    Accent { key: "orange", neon: Color32::from_rgb(0xFF, 0x8A, 0x1F), core: Color32::from_rgb(0xFF, 0xDE, 0xC2) },
    Accent { key: "yellow", neon: Color32::from_rgb(0xFF, 0xE8, 0x1F), core: Color32::from_rgb(0xFF, 0xF8, 0xC6) },
];

static ACCENT: AtomicUsize = AtomicUsize::new(0);

/// Troca a cor do neon pelo nome (desconhecido = verde). Depois chame `apply_style`.
pub fn set_accent(key: &str) {
    ACCENT.store(ACCENTS.iter().position(|a| a.key == key).unwrap_or(0), Ordering::Relaxed);
}

pub fn current_accent() -> &'static Accent {
    &ACCENTS[ACCENT.load(Ordering::Relaxed).min(ACCENTS.len() - 1)]
}

/// Cor forte do neon.
pub fn accent() -> Color32 {
    current_accent().neon
}

/// Nucleo claro do neon.
pub fn accent_core() -> Color32 {
    current_accent().core
}

/// Fundo escuro tingido com o neon (itens ativos/tocando).
pub fn accent_deep() -> Color32 {
    mix(BG, accent(), 0.14)
}

/// Texto sobre fundo neon: escuro nas cores claras, branco nas escuras.
pub fn on_accent() -> Color32 {
    let c = accent();
    let lum = 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
    if lum > 128.0 { Color32::from_rgb(0x05, 0x0B, 0x08) } else { Color32::WHITE }
}

pub fn neon(alpha: u8) -> Color32 {
    alpha_of(accent(), alpha)
}

pub fn alpha_of(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

// ---- fontes ----
pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Fonte dos titulos (Bahnschrift: tecnica, condensada).
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
}

/// Segoe UI e Bahnschrift do Windows quando existirem; senao as fontes embutidas do egui.
pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let defaults = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
    let mut load = |name: &str, file: &str| -> Option<String> {
        let bytes = std::fs::read(dir.join(file)).ok()?;
        defs.font_data.insert(name.into(), FontData::from_owned(bytes).into());
        Some(name.to_string())
    };
    let ui = load("ui", "segoeui.ttf");
    let semi = load("ui-semibold", "seguisb.ttf").or_else(|| load("ui-bold", "segoeuib.ttf"));
    let disp = load("display", "bahnschrift.ttf");
    let sym = load("symbols", "seguisym.ttf");

    let chain = |first: &[&Option<String>]| -> Vec<String> {
        let mut v: Vec<String> = first.iter().filter_map(|n| (*n).clone()).collect();
        v.extend(defaults.iter().cloned());
        v.extend(sym.clone());
        v
    };
    defs.families.insert(FontFamily::Proportional, chain(&[&ui]));
    defs.families.insert(FontFamily::Name("bold".into()), chain(&[&semi, &ui]));
    defs.families.insert(FontFamily::Name("display".into()), chain(&[&disp, &semi, &ui]));
    ctx.set_fonts(defs);
}

pub fn apply_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        use egui::TextStyle as T;
        style.text_styles.insert(T::Body, font(14.0));
        style.text_styles.insert(T::Button, bold(13.5));
        style.text_styles.insert(T::Small, font(12.0));
        style.text_styles.insert(T::Heading, display(20.0));
        style.text_styles.insert(T::Monospace, FontId::monospace(13.0));

        let s = &mut style.spacing;
        s.item_spacing = vec2(8.0, 8.0);
        s.button_padding = vec2(12.0, 6.0);
        s.interact_size.y = 30.0;
        s.combo_height = 320.0;
        s.menu_margin = egui::Margin::same(8);
        s.scroll = egui::style::ScrollStyle::thin();

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = None;
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.extreme_bg_color = Color32::from_rgb(0x0E, 0x11, 0x13);
        v.faint_bg_color = ITEM;
        v.code_bg_color = ITEM;
        v.warn_fg_color = WARN;
        v.error_fg_color = DANGER;
        v.hyperlink_color = accent();
        v.window_stroke = Stroke::new(1.0, neon(90));
        v.window_corner_radius = CornerRadius::same(16);
        v.menu_corner_radius = CornerRadius::same(12);
        v.window_shadow = Shadow { offset: [0, 0], blur: 40, spread: 0, color: neon(40) };
        v.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(170) };
        v.selection.bg_fill = neon(60);
        v.selection.stroke = Stroke::new(1.0, accent_core());
        v.text_cursor.stroke = Stroke::new(2.0, accent());

        let w = &mut v.widgets;
        let radius = CornerRadius::same(10);
        w.noninteractive.bg_fill = PANEL;
        w.noninteractive.weak_bg_fill = PANEL;
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        w.noninteractive.corner_radius = radius;
        w.inactive.bg_fill = ITEM;
        w.inactive.weak_bg_fill = ITEM;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = radius;
        w.hovered.bg_fill = ITEM_HOVER;
        w.hovered.weak_bg_fill = ITEM_HOVER;
        w.hovered.bg_stroke = Stroke::new(1.0, neon(170));
        w.hovered.fg_stroke = Stroke::new(1.5, accent_core());
        w.hovered.corner_radius = radius;
        w.hovered.expansion = 0.0;
        w.active.bg_fill = accent_deep();
        w.active.weak_bg_fill = accent_deep();
        w.active.bg_stroke = Stroke::new(1.0, accent());
        w.active.fg_stroke = Stroke::new(2.0, accent());
        w.active.corner_radius = radius;
        w.active.expansion = 0.0;
        w.open = w.hovered;
        w.open.bg_fill = ITEM_HOVER;
    });
}

// ---- neon ----

/// Halo de luz atras de `rect` (pintar antes do elemento).
pub fn glow(p: &Painter, rect: Rect, radius: f32, color: Color32, blur: f32) {
    let shadow = Shadow { offset: [0, 0], blur: blur.clamp(0.0, 255.0) as u8, spread: 0, color };
    p.add(shadow.as_shape(rect, CornerRadius::same(radius.clamp(0.0, 255.0) as u8)));
}

/// Luz ambiente: uma mancha grande e bem difusa.
pub fn ambient(p: &Painter, center: Pos2, size: Vec2, color: Color32) {
    glow(p, Rect::from_center_size(center, size), size.min_elem() / 2.0, color, 255.0);
}

/// Pulso suave entre 0 e 1 (para luzes "respirando").
pub fn pulse(time: f64, speed: f64) -> f32 {
    (0.5 + 0.5 * (time * speed).sin()) as f32
}

pub fn mmss(t: f64) -> String {
    let t = t.max(0.0);
    format!("{}:{:02}", (t / 60.0) as u64, (t % 60.0) as u64)
}

pub fn mmss_tenths(t: f64) -> String {
    let t = t.max(0.0);
    format!("{}:{:04.1}", (t / 60.0) as u64, t % 60.0)
}
