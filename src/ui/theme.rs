//! Identidade visual: cinza escuro + neon (verde fluorescente por padrao; 8 cores a escolher)
//! e o tema "Vice City" (violeta da noite com degrade magenta -> amarelo quente).
//!
//! Todas as cores da interface vem daqui. O "neon" e feito com sombras coloridas sem
//! deslocamento (o egui desenha a sombra borrada) e texto repintado com halo transparente.

use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Rect, Shadow, Stroke,
    Vec2, vec2,
};

// ---- cores de base (fundo, paineis, texto) ----

/// Cores neutras da interface. O tema "Vice City" troca o grafite por violeta escuro.
pub struct Base {
    pub bg: Color32,
    pub panel: Color32,
    pub item: Color32,
    pub item_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub dim: Color32,
    /// Pocos escuros (forma de onda, logo, campos de texto).
    pub well: Color32,
    /// Trilho apagado de sliders e medidores.
    pub track: Color32,
    /// Fundo dos cabecalhos de colecao.
    pub header: Color32,
}

pub const GRAPHITE: Base = Base {
    bg: Color32::from_rgb(0x0A, 0x0C, 0x0E),
    panel: Color32::from_rgb(0x12, 0x15, 0x18),
    item: Color32::from_rgb(0x19, 0x1D, 0x21),
    item_hover: Color32::from_rgb(0x21, 0x27, 0x2C),
    border: Color32::from_rgb(0x26, 0x2D, 0x33),
    text: Color32::from_rgb(0xE3, 0xF0, 0xE6),
    muted: Color32::from_rgb(0x84, 0x93, 0x8A),
    dim: Color32::from_rgb(0x41, 0x4B, 0x46),
    well: Color32::from_rgb(0x0D, 0x10, 0x12),
    track: Color32::from_rgb(0x24, 0x2A, 0x2F),
    header: Color32::from_rgb(0x15, 0x19, 0x1C),
};

/// Violeta da noite (Deep Violet #3B146B escurecido), para o tema "Vice City".
pub const DUSK: Base = Base {
    bg: Color32::from_rgb(0x11, 0x08, 0x1E),
    panel: Color32::from_rgb(0x1A, 0x0E, 0x2B),
    item: Color32::from_rgb(0x24, 0x14, 0x3A),
    item_hover: Color32::from_rgb(0x2F, 0x1B, 0x4A),
    border: Color32::from_rgb(0x3D, 0x24, 0x5E),
    text: Color32::from_rgb(0xFB, 0xEC, 0xF1),
    muted: Color32::from_rgb(0xB8, 0x9C, 0xC4),
    dim: Color32::from_rgb(0x60, 0x48, 0x78),
    well: Color32::from_rgb(0x0E, 0x06, 0x19),
    track: Color32::from_rgb(0x33, 0x22, 0x4A),
    header: Color32::from_rgb(0x20, 0x11, 0x34),
};

pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x3B, 0x5C);
pub const DANGER_DEEP: Color32 = Color32::from_rgb(0x2B, 0x0E, 0x14);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xD2, 0x3F);

pub const RADIUS_CARD: u8 = 18;

pub fn base() -> &'static Base {
    current_accent().base
}
pub fn bg() -> Color32 {
    base().bg
}
pub fn panel() -> Color32 {
    base().panel
}
pub fn item() -> Color32 {
    base().item
}
pub fn item_hover() -> Color32 {
    base().item_hover
}
pub fn border() -> Color32 {
    base().border
}
pub fn txt() -> Color32 {
    base().text
}
pub fn muted() -> Color32 {
    base().muted
}
pub fn dim() -> Color32 {
    base().dim
}
pub fn well() -> Color32 {
    base().well
}
pub fn track() -> Color32 {
    base().track
}
pub fn header_fill() -> Color32 {
    base().header
}

// ---- cor do neon e temas (escolhidos pelo usuario) ----

/// Uma cor de neon (tubo forte + nucleo claro) ou um tema com degrade.
pub struct Accent {
    /// Nome na config (`"accent"`) e chave de traducao `accent_<key>`.
    pub key: &'static str,
    pub neon: Color32,
    pub core: Color32,
    pub base: &'static Base,
    /// Degrade do tema (esquerda -> direita); None = cor unica.
    pub spectrum: Option<&'static [Color32]>,
    /// Texto sobre o neon; None = escuro ou branco pela luminancia.
    pub on: Option<Color32>,
}

const fn neon_color(key: &'static str, neon: Color32, core: Color32) -> Accent {
    Accent { key, neon, core, base: &GRAPHITE, spectrum: None, on: None }
}

/// Paleta do tema Vice City (por do sol): Magenta, Coral Pink, Coral, Orange, Warm Yellow.
pub const VICE_CITY: [Color32; 5] = [
    Color32::from_rgb(0xC9, 0x34, 0x9B),
    Color32::from_rgb(0xF3, 0x6A, 0x78),
    Color32::from_rgb(0xF8, 0x75, 0x72),
    Color32::from_rgb(0xFF, 0x9B, 0x55),
    Color32::from_rgb(0xFF, 0xB4, 0x58),
];

pub const ACCENTS: [Accent; 9] = [
    neon_color("green", Color32::from_rgb(0x39, 0xFF, 0x14), Color32::from_rgb(0xC8, 0xFF, 0xB8)),
    neon_color("cyan", Color32::from_rgb(0x00, 0xE5, 0xFF), Color32::from_rgb(0xC4, 0xF9, 0xFF)),
    neon_color("blue", Color32::from_rgb(0x2E, 0x7D, 0xFF), Color32::from_rgb(0xC6, 0xDC, 0xFF)),
    neon_color("purple", Color32::from_rgb(0xB0, 0x3C, 0xFF), Color32::from_rgb(0xE8, 0xCC, 0xFF)),
    neon_color("pink", Color32::from_rgb(0xFF, 0x2B, 0xD6), Color32::from_rgb(0xFF, 0xC8, 0xF3)),
    neon_color("red", Color32::from_rgb(0xFF, 0x2E, 0x4D), Color32::from_rgb(0xFF, 0xC6, 0xCE)),
    neon_color("orange", Color32::from_rgb(0xFF, 0x8A, 0x1F), Color32::from_rgb(0xFF, 0xDE, 0xC2)),
    neon_color("yellow", Color32::from_rgb(0xFF, 0xE8, 0x1F), Color32::from_rgb(0xFF, 0xF8, 0xC6)),
    // tema: violeta da noite com degrade magenta -> amarelo quente
    Accent {
        key: "vice_city",
        neon: Color32::from_rgb(0xF3, 0x6A, 0x78),
        core: Color32::from_rgb(0xFF, 0xD9, 0xC4),
        base: &DUSK,
        spectrum: Some(&VICE_CITY),
        on: Some(Color32::from_rgb(0x2A, 0x0B, 0x4A)),
    },
];

static ACCENT: AtomicUsize = AtomicUsize::new(0);

/// Troca a cor do neon pelo nome (desconhecido = verde). Depois chame `apply_style`.
pub fn set_accent(key: &str) {
    // "sunset" era o nome antigo do tema Vice City (configs ja salvas)
    let key = if key == "sunset" { "vice_city" } else { key };
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
    mix(bg(), accent(), 0.14)
}

/// Texto sobre fundo neon: escuro nas cores claras, branco nas escuras.
pub fn on_accent() -> Color32 {
    if let Some(c) = current_accent().on {
        return c;
    }
    let c = accent();
    let lum = 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
    if lum > 128.0 { Color32::from_rgb(0x05, 0x0B, 0x08) } else { Color32::WHITE }
}

pub fn neon(alpha: u8) -> Color32 {
    alpha_of(accent(), alpha)
}

/// Cor do degrade na posicao `t` (0 = esquerda, 1 = direita); sem degrade, a cor do neon.
pub fn spectrum_at(t: f32) -> Color32 {
    spectrum_of(current_accent(), t)
}

pub fn spectrum_of(a: &Accent, t: f32) -> Color32 {
    let Some(s) = a.spectrum else { return a.neon };
    let x = t.clamp(0.0, 1.0) * (s.len() - 1) as f32;
    let i = (x.floor() as usize).min(s.len() - 2);
    mix(s[i], s[i + 1], x - i as f32)
}

/// O tema atual tem degrade?
pub fn has_spectrum() -> bool {
    current_accent().spectrum.is_some()
}

/// Cores das 5 barras do icone (tubo, nucleo): arco-iris nas cores de neon, a paleta no tema.
pub fn bar_colors() -> [([u8; 3], [u8; 3]); 5] {
    if !has_spectrum() {
        return crate::icon_raster::BAR_COLORS;
    }
    std::array::from_fn(|i| {
        let c = spectrum_at(i as f32 / 4.0);
        let core = mix(c, Color32::WHITE, 0.55);
        ([c.r(), c.g(), c.b()], [core.r(), core.g(), core.b()])
    })
}

pub fn alpha_of(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Textura 256x1 com o degrade do tema atual (None sem degrade). Recriada em `apply_style`.
static GRADIENT: Mutex<Option<egui::TextureHandle>> = Mutex::new(None);

fn load_gradient(ctx: &egui::Context) {
    let tex = has_spectrum().then(|| {
        let pixels: Vec<Color32> = (0..256).map(|i| spectrum_at(i as f32 / 255.0)).collect();
        let image = egui::ColorImage::new([256, 1], pixels);
        ctx.load_texture("theme-gradient", image, egui::TextureOptions::LINEAR)
    });
    *GRADIENT.lock() = tex;
}

/// Preenche um retangulo arredondado: com o degrade do tema, ou com `fallback` se nao houver.
pub fn fill(p: &Painter, rect: Rect, radius: impl Into<CornerRadius>, fallback: Color32) {
    match GRADIENT.lock().as_ref() {
        Some(tex) => {
            let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            p.add(egui::epaint::RectShape::filled(rect, radius, Color32::WHITE).with_texture(tex.id(), uv));
        }
        None => {
            p.rect_filled(rect, radius, fallback);
        }
    }
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
        v.panel_fill = bg();
        v.window_fill = panel();
        v.extreme_bg_color = well();
        v.faint_bg_color = item();
        v.code_bg_color = item();
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
        w.noninteractive.bg_fill = panel();
        w.noninteractive.weak_bg_fill = panel();
        w.noninteractive.bg_stroke = Stroke::new(1.0, border());
        w.noninteractive.fg_stroke = Stroke::new(1.0, txt());
        w.noninteractive.corner_radius = radius;
        w.inactive.bg_fill = item();
        w.inactive.weak_bg_fill = item();
        w.inactive.bg_stroke = Stroke::new(1.0, border());
        w.inactive.fg_stroke = Stroke::new(1.0, txt());
        w.inactive.corner_radius = radius;
        w.hovered.bg_fill = item_hover();
        w.hovered.weak_bg_fill = item_hover();
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
        w.open.bg_fill = item_hover();
    });
    load_gradient(ctx);
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
