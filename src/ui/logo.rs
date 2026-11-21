//! Logo: quadrado escuro com borda na cor do neon e barras de onda coloridas (uma cor cada).
//! `icon_data` rasteriza o icone da janela/barra de tarefas; `paint` desenha o logo do cabecalho.

use eframe::egui::{self, Color32, CornerRadius, Painter, Rect, Stroke, StrokeKind, pos2};

use super::theme::{self, *};

use crate::icon_raster::{BAR_CORE_MIX, BARS};

/// Icone da janela e da barra de tarefas: o do .exe, com a borda na cor do neon atual.
pub fn icon_data() -> egui::IconData {
    let size = 64;
    let (a, c) = (accent(), accent_core());
    let colors = ([a.r(), a.g(), a.b()], [c.r(), c.g(), c.b()]);
    egui::IconData { rgba: crate::icon_raster::rgba(size, colors, bar_colors()), width: size as u32, height: size as u32 }
}

/// Logo vetorial para o cabecalho, com as barras "dancando" conforme o nivel de saida.
pub fn paint(p: &Painter, rect: Rect, level: f32, time: f64) {
    let r = (rect.width() * 0.22) as u8;
    theme::glow(p, rect, r as f32, neon(70), 22.0);
    p.rect(rect, CornerRadius::same(r), Color32::from_rgb(0x0E, 0x12, 0x14), Stroke::new(1.5, accent()), StrokeKind::Inside);
    let s = rect.width();
    let c = rect.center();
    let bars = bar_colors();
    for (i, h) in BARS.iter().enumerate() {
        let wobble = (time * 7.0 + i as f64 * 1.3).sin() as f32 * 0.5 + 0.5;
        let k = 0.75 + 0.6 * level.min(1.0) * wobble;
        let x = c.x + (i as f32 - 2.0) * s * 0.135;
        let hh = h * s * k;
        let a = pos2(x, c.y - hh);
        let b = pos2(x, c.y + hh);
        let (tube, core) = bars[i];
        let tube = Color32::from_rgb(tube[0], tube[1], tube[2]);
        let body = mix(tube, Color32::from_rgb(core[0], core[1], core[2]), BAR_CORE_MIX);
        p.line_segment([a, b], Stroke::new(s * 0.18, alpha_of(tube, 70)));
        p.line_segment([a, b], Stroke::new(s * 0.085, body));
    }
}
