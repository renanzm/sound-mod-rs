//! Forma de onda do clipe em barras neon, com alcas para escolher o trecho e cursor de reproducao.
//!
//! Arrastar as alcas muda o trecho (evento `Range` ao soltar); clicar/arrastar no meio move a
//! posicao (`Seek`).

use eframe::egui::{Align2, Color32, CornerRadius, CursorIcon, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::theme::{self, *};
use crate::i18n::{t, tf};

const PAD: f32 = 14.0;
const GRAB: f32 = 10.0;
const STEP: f32 = 5.0; // pixels por barra (barra + espaco)
const MIN_SEL: f64 = 0.1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    Start,
    End,
    Seek,
}

pub enum WaveEvent {
    Range(f64, f64),
    Seek(f64),
}

#[derive(Default)]
pub struct Waveform {
    pub version: u64,
    peaks: Vec<f32>,
    pub dur: f64,
    pub start: f64,
    pub end: f64,
    drag: Option<Drag>,
}

impl Waveform {
    /// `data` estereo intercalado; `start`/`end` em segundos.
    pub fn set_audio(&mut self, version: u64, data: &[f32], rate: u32, start: f64, end: f64) {
        self.version = version;
        let frames = data.len() / 2;
        self.dur = frames as f64 / rate as f64;
        let k = (frames / 2000).max(1);
        self.peaks = data
            .chunks(k * 2)
            .map(|c| c.iter().fold(0f32, |m, v| m.max(v.abs())))
            .collect();
        self.start = start.clamp(0.0, self.dur);
        self.end = if end <= self.start || end > self.dur { self.dur } else { end };
        self.drag = None;
    }

    pub fn ui(&mut self, ui: &mut Ui, height: f32, pos: Option<f64>, playing: bool, time: f64) -> Option<WaveEvent> {
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
        let p = ui.painter().with_clip_rect(rect.expand(8.0));
        p.rect(rect, CornerRadius::same(14), Color32::from_rgb(0x0D, 0x10, 0x12), Stroke::new(1.0, BORDER), StrokeKind::Inside);
        // grade sutil de "osciloscopio"
        let mid = rect.center().y;
        p.hline(rect.x_range().shrink(PAD), mid, Stroke::new(1.0, Color32::from_white_alpha(8)));
        if self.peaks.is_empty() || self.dur <= 0.0 {
            p.text(rect.center(), Align2::CENTER_CENTER, t("no_clip"), font(13.0), MUTED);
            return None;
        }

        let inner_w = (rect.width() - 2.0 * PAD).max(1.0);
        let x_of = |s: f64| rect.left() + PAD + (s / self.dur) as f32 * inner_w;
        let t_of = |x: f32| (((x - rect.left() - PAD) / inner_w).clamp(0.0, 1.0) as f64) * self.dur;

        // ---- mouse ----
        let mut event = None;
        let hover = resp.hover_pos().map(|h| h.x);
        let which = |x: f32| {
            if (x - x_of(self.start)).abs() <= GRAB {
                Drag::Start
            } else if (x - x_of(self.end)).abs() <= GRAB {
                Drag::End
            } else {
                Drag::Seek
            }
        };
        if let Some(x) = hover {
            let icon = if which(x) == Drag::Seek { CursorIcon::PointingHand } else { CursorIcon::ResizeHorizontal };
            ui.ctx().set_cursor_icon(icon);
        }
        if resp.is_pointer_button_down_on() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let drag = *self.drag.get_or_insert_with(|| which(pos.x));
                let at = t_of(pos.x);
                match drag {
                    Drag::Start => self.start = at.min(self.end - MIN_SEL).max(0.0),
                    Drag::End => self.end = at.max(self.start + MIN_SEL).min(self.dur),
                    Drag::Seek => event = Some(WaveEvent::Seek(at.clamp(self.start, self.end))),
                }
            }
        } else if let Some(drag) = self.drag.take() {
            if drag != Drag::Seek {
                event = Some(WaveEvent::Range(self.start, self.end));
            }
        }

        // ---- desenho ----
        let (xs, xe) = (x_of(self.start), x_of(self.end));
        let sel = Rect::from_min_max(pos2(xs, rect.top() + 6.0), pos2(xe, rect.bottom() - 6.0));
        p.rect_filled(sel, CornerRadius::same(6), neon(12));
        let bars = ((inner_w / STEP) as usize).max(1);
        let n = self.peaks.len();
        let amps: Vec<f32> = (0..bars)
            .map(|i| {
                let a = i * n / bars;
                let b = ((i + 1) * n / bars).max(a + 1).min(n);
                self.peaks[a.min(n - 1)..b].iter().fold(0f32, |m, v| m.max(*v))
            })
            .collect();
        let top = amps.iter().fold(1e-3f32, |m, v| m.max(*v));
        let half = height / 2.0 - 20.0;
        let k = if playing { theme::pulse(time, 8.0) } else { 0.0 };
        for (i, a) in amps.iter().enumerate() {
            let x = rect.left() + PAD + (i as f32 + 0.5) * STEP;
            let hh = (a / top * half).max(1.5);
            let inside = x >= xs && x <= xe;
            if inside {
                p.line_segment([pos2(x, mid - hh), pos2(x, mid + hh)], Stroke::new(5.0, neon((28.0 + 22.0 * k) as u8)));
                p.line_segment([pos2(x, mid - hh), pos2(x, mid + hh)], Stroke::new(2.6, NEON));
            } else {
                p.line_segment([pos2(x, mid - hh), pos2(x, mid + hh)], Stroke::new(2.6, DIM));
            }
        }

        // alcas
        for (x, when, left) in [(xs, self.start, true), (xe, self.end, false)] {
            let line = [pos2(x, rect.top() + 6.0), pos2(x, rect.bottom() - 6.0)];
            p.line_segment(line, Stroke::new(4.0, neon(60)));
            p.line_segment(line, Stroke::new(1.5, NEON_CORE));
            let knob = pos2(x, mid);
            theme::glow(&p, Rect::from_center_size(knob, vec2(12.0, 12.0)), 6.0, neon(150), 10.0);
            p.circle(knob, 6.5, NEON_CORE, Stroke::new(2.0, NEON));
            let (anchor, dx) = if left { (Align2::LEFT_TOP, 6.0) } else { (Align2::RIGHT_TOP, -6.0) };
            p.text(pos2(x + dx, rect.top() + 6.0), anchor, theme::mmss_tenths(when), bold(10.5), NEON_CORE);
        }
        let label = tf("selection", &[("sel", &format!("{:.1}", self.end - self.start)), ("total", &format!("{:.1}", self.dur))]);
        p.text(pos2(rect.right() - PAD, rect.bottom() - 5.0), Align2::RIGHT_BOTTOM, label, font(11.0), MUTED);

        // cursor de reproducao
        if let Some(pos) = pos {
            let x = x_of(pos);
            let line = [pos2(x, rect.top() + 4.0), pos2(x, rect.bottom() - 4.0)];
            p.line_segment(line, Stroke::new(6.0, Color32::from_white_alpha(30)));
            p.line_segment(line, Stroke::new(2.0, Color32::WHITE));
        }
        event
    }
}
