//! Icones vetoriais desenhados com o painter: nitidos em qualquer escala e sem depender de fonte.

use std::f32::consts::PI;

use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Record,
    Edit,
    Backspace,
    Trash,
    Folder,
    Plus,
    Volume,
    Mic,
    Save,
    Power,
    Waves,
    Sliders,
}

/// Desenha o icone centralizado em `rect`, ocupando o lado menor.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height());
    let o = rect.center() - vec2(s, s) / 2.0;
    let at = |x: f32, y: f32| -> Pos2 { o + vec2(x * s, y * s) };
    let w = (s * 0.1).max(1.4);
    let stroke = Stroke::new(w, color);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), stroke));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(Shape::closed_line(pts.iter().map(|&(x, y)| at(x, y)).collect(), stroke));
    };
    let fill = |pts: &[(f32, f32)]| {
        p.add(Shape::convex_polygon(pts.iter().map(|&(x, y)| at(x, y)).collect(), color, Stroke::NONE));
    };
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| {
        let n = 16;
        let pts = (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                at(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        p.add(Shape::line(pts, stroke));
    };
    let rrect = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        p.rect_filled(Rect::from_min_max(at(x0, y0), at(x1, y1)), CornerRadius::same((r * s) as u8), color);
    };

    match icon {
        Icon::Play => fill(&[(0.3, 0.2), (0.82, 0.5), (0.3, 0.8)]),
        Icon::Pause => {
            rrect(0.26, 0.2, 0.42, 0.8, 0.05);
            rrect(0.58, 0.2, 0.74, 0.8, 0.05);
        }
        Icon::Stop => rrect(0.24, 0.24, 0.76, 0.76, 0.08),
        Icon::Record => {
            p.circle_filled(at(0.5, 0.5), s * 0.24, color);
        }
        Icon::Edit => {
            p.line_segment([at(0.3, 0.7), at(0.72, 0.28)], Stroke::new(w * 1.6, color));
            fill(&[(0.2, 0.8), (0.25, 0.6), (0.4, 0.75)]);
        }
        Icon::Backspace => {
            closed(&[(0.12, 0.5), (0.34, 0.24), (0.88, 0.24), (0.88, 0.76), (0.34, 0.76)]);
            line(&[(0.48, 0.38), (0.7, 0.62)]);
            line(&[(0.7, 0.38), (0.48, 0.62)]);
        }
        Icon::Trash => {
            line(&[(0.2, 0.28), (0.8, 0.28)]);
            line(&[(0.4, 0.28), (0.42, 0.18), (0.58, 0.18), (0.6, 0.28)]);
            closed(&[(0.27, 0.34), (0.32, 0.84), (0.68, 0.84), (0.73, 0.34)]);
            line(&[(0.44, 0.44), (0.45, 0.72)]);
            line(&[(0.56, 0.44), (0.55, 0.72)]);
        }
        Icon::Folder => {
            closed(&[(0.12, 0.26), (0.4, 0.26), (0.48, 0.36), (0.88, 0.36), (0.88, 0.8), (0.12, 0.8)]);
            line(&[(0.12, 0.44), (0.88, 0.44)]);
        }
        Icon::Plus => {
            line(&[(0.5, 0.2), (0.5, 0.8)]);
            line(&[(0.2, 0.5), (0.8, 0.5)]);
        }
        Icon::Volume => {
            fill(&[(0.12, 0.38), (0.3, 0.38), (0.5, 0.2), (0.5, 0.8), (0.3, 0.62), (0.12, 0.62)]);
            arc(0.5, 0.5, 0.17, -PI / 4.0, PI / 4.0);
            arc(0.5, 0.5, 0.33, -PI / 3.5, PI / 3.5);
        }
        Icon::Mic => {
            p.rect_stroke(
                Rect::from_min_max(at(0.37, 0.12), at(0.63, 0.6)),
                CornerRadius::same((0.13 * s) as u8),
                stroke,
                eframe::egui::StrokeKind::Middle,
            );
            arc(0.5, 0.46, 0.25, 0.0, PI);
            line(&[(0.5, 0.71), (0.5, 0.88)]);
            line(&[(0.34, 0.88), (0.66, 0.88)]);
        }
        Icon::Save => {
            line(&[(0.5, 0.14), (0.5, 0.62)]);
            line(&[(0.3, 0.44), (0.5, 0.64), (0.7, 0.44)]);
            line(&[(0.16, 0.62), (0.16, 0.84), (0.84, 0.84), (0.84, 0.62)]);
        }
        Icon::Power => {
            arc(0.5, 0.54, 0.3, -PI / 2.0 + 0.7, PI * 1.5 - 0.7);
            line(&[(0.5, 0.14), (0.5, 0.5)]);
        }
        Icon::Waves => {
            for (i, h) in [0.18, 0.34, 0.46, 0.3, 0.16].iter().enumerate() {
                let x = 0.2 + i as f32 * 0.15;
                p.line_segment([at(x, 0.5 - h), at(x, 0.5 + h)], Stroke::new(w * 1.3, color));
            }
        }
        Icon::Sliders => {
            for (i, k) in [0.35, 0.65, 0.45].iter().enumerate() {
                let y = 0.25 + i as f32 * 0.25;
                line(&[(0.14, y), (0.86, y)]);
                p.circle_filled(at(*k, y), s * 0.09, color);
            }
        }
    }
}
