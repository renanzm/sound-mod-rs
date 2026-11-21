//! Widgets neon desenhados a mao: botoes iluminados, botao de tecla, interruptor, slider,
//! medidor de nivel, indicador "ao vivo" e cartoes.

use std::ops::RangeInclusive;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, InnerResponse, Layout, Margin, Rect,
    Response, Sense, Shadow, Stroke, StrokeKind, Ui, pos2, vec2,
};

use super::icons::{self, Icon};
use super::theme::{self, *};
use crate::hotkeys;
use crate::i18n::t;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Verde aceso (acao principal).
    Primary,
    /// Escuro, acende no hover.
    Ghost,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    /// Fundo escuro.
    Normal,
    /// Sem fundo ate passar o mouse.
    Flat,
    Danger,
}

fn hover_anim(ui: &Ui, id: Id, on: bool) -> f32 {
    ui.ctx().animate_bool_with_time(id.with("hover"), on, 0.12)
}

/// Botao-pilula com icone opcional.
pub fn button(ui: &mut Ui, text: &str, icon: Option<Icon>, style: Style, min_width: f32) -> Response {
    let font = bold(13.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let icon_w = if icon.is_some() { 24.0 } else { 0.0 };
    let width = (galley.size().x + icon_w + 34.0).max(min_width);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 34.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let h = hover_anim(ui, resp.id, resp.hovered());
        let pressed = resp.is_pointer_button_down_on();
        let p = ui.painter();
        let r = rect.height() / 2.0;
        let (fill, stroke, fg, glow_c, glow_a) = match style {
            Style::Primary => (mix(NEON, NEON_CORE, h * 0.3), Stroke::NONE, ON_NEON, NEON, 70.0 + 90.0 * h),
            Style::Ghost => (
                mix(ITEM, ITEM_HOVER, h),
                Stroke::new(1.0, mix(BORDER, NEON, h * 0.8)),
                mix(TEXT, NEON_CORE, h),
                NEON,
                45.0 * h,
            ),
        };
        if glow_a >= 1.0 {
            theme::glow(p, rect, r, alpha_of(glow_c, glow_a as u8), 18.0 + 10.0 * h);
        }
        let body = if pressed { rect.shrink(1.0) } else { rect };
        p.rect(body, CornerRadius::same(r as u8), fill, stroke, StrokeKind::Inside);
        if style == Style::Primary {
            // reflexo de vidro na metade de cima
            let top = Rect::from_min_max(body.min + vec2(r * 0.6, 2.0), pos2(body.max.x - r * 0.6, body.center().y - 2.0));
            p.rect_filled(top, CornerRadius::same((r * 0.5) as u8), Color32::from_white_alpha(28));
        }
        let total = icon_w + galley.size().x;
        let mut x = rect.center().x - total / 2.0;
        if let Some(icon) = icon {
            icons::paint(p, Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0)), icon, fg);
            x += icon_w;
        }
        p.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Botao quadrado arredondado so com icone.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, tint: Tint, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), sense);
    if ui.is_rect_visible(rect) {
        let h = if enabled { hover_anim(ui, resp.id, resp.hovered()) } else { 0.0 };
        let p = ui.painter();
        let r = (size * 0.32).min(10.0);
        let accent = if tint == Tint::Danger { DANGER } else { NEON };
        let base = match tint {
            Tint::Normal => ITEM,
            Tint::Flat | Tint::Danger => Color32::TRANSPARENT,
        };
        let hover_fill = if tint == Tint::Danger { DANGER_DEEP } else { ITEM_HOVER };
        if h > 0.01 {
            theme::glow(p, rect, r, alpha_of(accent, (40.0 * h) as u8), 12.0);
        }
        let body = if resp.is_pointer_button_down_on() { rect.shrink(1.0) } else { rect };
        let stroke = match tint {
            Tint::Normal => Stroke::new(1.0, mix(BORDER, accent, h * 0.8)),
            _ => Stroke::new(1.0, alpha_of(accent, (150.0 * h) as u8)),
        };
        p.rect(body, CornerRadius::same(r as u8), mix(base, hover_fill, h), stroke, StrokeKind::Inside);
        let idle = match (tint, enabled) {
            (_, false) => DIM,
            (Tint::Danger, _) => alpha_of(DANGER, 210),
            (Tint::Flat, _) => MUTED,
            (Tint::Normal, _) => TEXT,
        };
        let fg = if enabled { mix(idle, if tint == Tint::Danger { DANGER } else { NEON_CORE }, h) } else { idle };
        icons::paint(p, rect.shrink(size * 0.27), icon, fg);
    }
    if enabled { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp }
}

pub struct KeyResponse {
    pub action: Response,
    pub edit: Response,
    pub clear: Response,
}

fn key_label(key: &str, capturing: bool) -> String {
    if capturing {
        t("press_key").to_string()
    } else if key.is_empty() {
        "—".to_string()
    } else {
        hotkeys::display(key)
    }
}

fn key_pill_width(ui: &Ui, label: &str) -> f32 {
    let w = ui.painter().layout_no_wrap(label.to_string(), bold(13.0), Color32::PLACEHOLDER).size().x;
    (w + 46.0).max(88.0)
}

/// Largura total do botao de tecla (pilula + lapis + ⌫).
pub fn key_button_width(ui: &Ui, key: &str, capturing: bool) -> f32 {
    key_pill_width(ui, &key_label(key, capturing)) + 2.0 * (2.0 + 24.0)
}

/// Botao dividido: a pilula executa a acao, o lapis troca a tecla e o ⌫ remove a tecla.
#[allow(clippy::too_many_arguments)]
pub fn key_button(
    ui: &mut Ui,
    icon: Icon,
    key: &str,
    active: bool,
    capturing: bool,
    capture_locked: bool,
    danger: bool,
    time: f64,
) -> KeyResponse {
    let label = key_label(key, capturing);
    let w = key_pill_width(ui, &label);
    let total = w + 2.0 * (2.0 + 24.0);
    // layout proprio da esquerda para a direita: funciona tambem dentro de um layout invertido
    ui.allocate_ui_with_layout(vec2(total, 32.0), Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let galley = ui.painter().layout_no_wrap(label, bold(13.0), Color32::PLACEHOLDER);
        let sense = if capturing { Sense::hover() } else { Sense::click() };
        let (rect, action) = ui.allocate_exact_size(vec2(w, 32.0), sense);
        if ui.is_rect_visible(rect) {
            let h = hover_anim(ui, action.id, action.hovered());
            let pulse = theme::pulse(time, 6.0);
            let accent = if danger { DANGER } else { NEON };
            let p = ui.painter();
            let (fill, stroke, fg, glow_a) = if active {
                let fg = if danger { Color32::WHITE } else { ON_NEON };
                (mix(accent, NEON_CORE, if danger { 0.0 } else { 0.2 * h }), Stroke::NONE, fg, 90.0 + 70.0 * pulse)
            } else if capturing {
                (ITEM, Stroke::new(1.5, alpha_of(accent, (120.0 + 135.0 * pulse) as u8)), accent, 30.0 + 60.0 * pulse)
            } else {
                let base = if danger { DANGER_DEEP } else { ITEM };
                (
                    mix(base, ITEM_HOVER, h),
                    Stroke::new(1.0, mix(if danger { alpha_of(DANGER, 110) } else { BORDER }, accent, h * 0.8)),
                    mix(if danger { mix(DANGER, TEXT, 0.4) } else { TEXT }, if danger { DANGER } else { NEON_CORE }, h),
                    40.0 * h,
                )
            };
            if glow_a >= 1.0 {
                theme::glow(p, rect, 16.0, alpha_of(accent, glow_a as u8), 16.0);
            }
            let body = if action.is_pointer_button_down_on() { rect.shrink(1.0) } else { rect };
            p.rect(body, CornerRadius::same(16), fill, stroke, StrokeKind::Inside);
            let icon_rect = Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(13.0, 13.0));
            icons::paint(p, icon_rect, icon, fg);
            let x = rect.left() + 32.0 + (w - 40.0 - galley.size().x) / 2.0;
            p.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
        }
        let action = if capturing { action } else { action.on_hover_cursor(CursorIcon::PointingHand) };
        let can_edit = !capturing && !capture_locked;
        let edit = icon_button(ui, Icon::Edit, 24.0, Tint::Flat, can_edit).on_hover_text(t("tip_edit"));
        let clear = icon_button(ui, Icon::Backspace, 24.0, Tint::Flat, can_edit && !key.is_empty())
            .on_hover_text(t("tip_clear"));
        KeyResponse { action, edit, clear }
    })
    .inner
}

/// Interruptor neon com texto ao lado.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), font(13.5), Color32::PLACEHOLDER);
    let size = vec2(42.0 + 10.0 + galley.size().x, 26.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let k = ui.ctx().animate_bool_with_time(resp.id, *on, 0.16);
        let h = hover_anim(ui, resp.id, resp.hovered());
        let p = ui.painter();
        let track = Rect::from_min_size(pos2(rect.left(), rect.center().y - 11.0), vec2(42.0, 22.0));
        if k > 0.01 {
            theme::glow(p, track, 11.0, neon((50.0 * k + 40.0 * h * k) as u8), 14.0);
        }
        let stroke = Stroke::new(1.0, mix(mix(BORDER, MUTED, h), NEON, k));
        p.rect(track, CornerRadius::same(11), mix(ITEM, NEON_DEEP, k), stroke, StrokeKind::Inside);
        let knob = pos2(egui::lerp(track.left() + 11.0..=track.right() - 11.0, k), track.center().y);
        if k > 0.01 {
            theme::glow(p, Rect::from_center_size(knob, vec2(12.0, 12.0)), 6.0, neon((150.0 * k) as u8), 10.0);
        }
        p.circle_filled(knob, 7.5, mix(MUTED, NEON_CORE, k));
        p.galley(
            pos2(track.right() + 10.0, rect.center().y - galley.size().y / 2.0),
            galley,
            mix(mix(MUTED, TEXT, h), TEXT, k),
        );
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Slider fino com trilho aceso ate o cursor. `muted` = trilho cinza (volume).
pub fn slider(ui: &mut Ui, value: &mut f32, range: RangeInclusive<f32>, width: f32, muted: bool, enabled: bool) -> Response {
    let sense = if enabled { Sense::click_and_drag() } else { Sense::hover() };
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, 22.0), sense);
    let (lo, hi) = (*range.start(), *range.end());
    let span = (hi - lo).max(1e-6);
    let inner = rect.shrink2(vec2(7.0, 0.0));
    if enabled && resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let v = lo + ((pos.x - inner.left()) / inner.width()).clamp(0.0, 1.0) * span;
            if v != *value {
                *value = v;
                resp.mark_changed();
            }
        }
    }
    if ui.is_rect_visible(rect) {
        let active = resp.hovered() || resp.dragged();
        let h = hover_anim(ui, resp.id, active);
        let p = ui.painter();
        let frac = ((*value - lo) / span).clamp(0.0, 1.0);
        let y = rect.center().y;
        let track = Rect::from_min_max(pos2(inner.left(), y - 2.0), pos2(inner.right(), y + 2.0));
        p.rect_filled(track, CornerRadius::same(2), Color32::from_rgb(0x24, 0x2A, 0x2F));
        let x = inner.left() + frac * inner.width();
        let lit = Rect::from_min_max(track.min, pos2(x, track.max.y));
        let color = if !enabled { DIM } else if muted { mix(MUTED, NEON, h) } else { NEON };
        if enabled && !muted && lit.width() > 1.0 {
            theme::glow(p, lit, 2.0, neon((60.0 + 50.0 * h) as u8), 8.0);
        }
        p.rect_filled(lit, CornerRadius::same(2), color);
        if enabled {
            let knob = pos2(x, y);
            theme::glow(p, Rect::from_center_size(knob, vec2(10.0, 10.0)), 5.0, alpha_of(color, (50.0 + 110.0 * h) as u8), 10.0);
            p.circle_filled(knob, 5.5 + 1.5 * h, mix(color, NEON_CORE, 0.6));
        }
    }
    if enabled { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp }
}

/// Texto de uma linha cortado com "…" na largura dada.
pub fn label_truncated(ui: &mut Ui, text: &str, width: f32, font: FontId, color: Color32) -> Response {
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap.max_width = width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, galley.size().y.max(20.0)), Sense::hover());
    ui.painter().galley(pos2(rect.left(), rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

pub fn caption(ui: &mut Ui, text: &str) -> Response {
    ui.label(egui::RichText::new(text).font(font(12.5)).color(MUTED))
}

/// Indicador "AO VIVO" (bolinha pulsando) ou "PARADO".
pub fn status_pill(ui: &mut Ui, live: bool, time: f64) -> Response {
    let text = t(if live { "live" } else { "offline" }).to_uppercase();
    let galley = ui.painter().layout_no_wrap(text, bold(12.0), Color32::PLACEHOLDER);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 40.0, 30.0), Sense::hover());
    let p = ui.painter();
    let stroke = Stroke::new(1.0, if live { neon(130) } else { BORDER });
    if live {
        theme::glow(p, rect, 15.0, neon(30), 14.0);
    }
    p.rect(rect, CornerRadius::same(15), ITEM, stroke, StrokeKind::Inside);
    let dot = pos2(rect.left() + 16.0, rect.center().y);
    if live {
        let k = theme::pulse(time, 4.0);
        theme::glow(p, Rect::from_center_size(dot, vec2(8.0, 8.0)), 4.0, neon((90.0 + 140.0 * k) as u8), 8.0 + 6.0 * k);
        p.circle_filled(dot, 4.0, NEON_CORE);
    } else {
        p.circle_filled(dot, 4.0, DIM);
    }
    let fg = if live { NEON_CORE } else { MUTED };
    p.galley(pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0), galley, fg);
    resp
}

/// Medidor de nivel em segmentos (0..1, escala perceptiva).
pub fn level_meter(ui: &mut Ui, label: &str, level: f32) -> Response {
    const SEGS: usize = 16;
    let (rect, resp) = ui.allocate_exact_size(vec2(150.0, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(15), ITEM, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    p.text(pos2(rect.left() + 12.0, rect.center().y), Align2::LEFT_CENTER, label, bold(10.5), MUTED);
    let x0 = rect.left() + 52.0;
    let seg_w = (rect.right() - 12.0 - x0) / SEGS as f32;
    let lit = (level.clamp(0.0, 1.0).sqrt() * SEGS as f32).round() as usize;
    if lit > 0 {
        let glow_rect = Rect::from_min_max(pos2(x0, rect.center().y - 5.0), pos2(x0 + seg_w * lit as f32, rect.center().y + 5.0));
        theme::glow(p, glow_rect, 3.0, neon(60), 10.0);
    }
    for i in 0..SEGS {
        let x = x0 + i as f32 * seg_w;
        let r = Rect::from_min_max(pos2(x, rect.center().y - 5.0), pos2(x + seg_w - 2.0, rect.center().y + 5.0));
        let hot = i >= SEGS - 3;
        let color = if i < lit {
            if hot { WARN } else { mix(NEON, NEON_CORE, i as f32 / SEGS as f32 * 0.5) }
        } else {
            Color32::from_rgb(0x22, 0x28, 0x2D)
        };
        p.rect_filled(r, CornerRadius::same(1), color);
    }
    resp
}

/// Pilula de escolha (um selecionado por grupo).
pub fn choice(ui: &mut Ui, text: &str, selected: bool, width: f32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), bold(13.0), Color32::PLACEHOLDER);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 34.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let h = hover_anim(ui, resp.id, resp.hovered());
        let s = ui.ctx().animate_bool_with_time(resp.id.with("sel"), selected, 0.18);
        let p = ui.painter();
        if s > 0.01 || h > 0.01 {
            theme::glow(p, rect, 17.0, neon((90.0 * s + 35.0 * h) as u8), 16.0);
        }
        let fill = mix(mix(ITEM, ITEM_HOVER, h), NEON, s);
        let stroke = Stroke::new(1.0, mix(mix(BORDER, NEON, h * 0.8), NEON, s));
        p.rect(rect, CornerRadius::same(17), fill, stroke, StrokeKind::Inside);
        let fg = mix(mix(MUTED, NEON_CORE, h), ON_NEON, s);
        p.galley(rect.center() - galley.size() / 2.0, galley, fg);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Painel com borda fina e um filete de luz verde no topo.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let resp = egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(RADIUS_CARD)
        .inner_margin(Margin::same(16))
        .shadow(Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(160) })
        .show(ui, add);
    let r = resp.response.rect;
    let p = ui.painter();
    let (a, b) = (pos2(r.left() + 24.0, r.top()), pos2(r.left() + 96.0, r.top()));
    theme::glow(p, Rect::from_two_pos(a, b).expand2(vec2(0.0, 1.0)), 1.0, neon(140), 12.0);
    p.line_segment([a, b], Stroke::new(2.0, NEON_CORE));
    resp
}

/// Titulo de secao: icone neon + texto em caixa alta espacado.
pub fn title(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat { font_id: display(17.0), color: TEXT, extra_letter_spacing: 1.4, ..Default::default() },
    );
    let galley = ui.painter().layout_job(job);
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0 + galley.size().x, 26.0), Sense::hover());
    let p = ui.painter();
    let icon_rect = Rect::from_center_size(pos2(rect.left() + 9.0, rect.center().y), vec2(18.0, 18.0));
    theme::glow(p, icon_rect.shrink(3.0), 4.0, neon(70), 10.0);
    icons::paint(p, icon_rect, icon, NEON);
    p.galley(pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0), galley, TEXT);
    resp
}
