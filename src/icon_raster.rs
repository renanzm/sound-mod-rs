//! Desenho do icone do app em pixels (RGBA), sem dependencias: usado pela janela (logo.rs) e
//! pelo build.rs, que grava o icone dentro do .exe.
//!
//! Quadrado grafite com borda neon e cinco barras de onda, por campo de distancia
//! (anti-aliasing e brilho de graca).

pub const BARS: [f32; 5] = [0.13, 0.24, 0.32, 0.22, 0.12];

/// Verde do icone do .exe (o padrao do app).
#[allow(dead_code)] // usado pelo build.rs
pub const GREEN: ([u8; 3], [u8; 3]) = ([0x39, 0xFF, 0x14], [0xC8, 0xFF, 0xB8]);

/// Pixels RGBA (nao pre-multiplicados) de um icone `n` x `n`, com o neon (tubo, nucleo) dado.
pub fn rgba(n: usize, (tube, filament): ([u8; 3], [u8; 3])) -> Vec<u8> {
    let mut out = vec![0u8; n * n * 4];
    let s = n as f32;
    let px_unit = s / 64.0; // espessuras pensadas para 64 px
    let (half, radius) = (s * 0.46, s * 0.2);
    let neon = tube.map(f32::from);
    let core = filament.map(f32::from);
    for y in 0..n {
        for x in 0..n {
            let (px, py) = (x as f32 + 0.5 - s / 2.0, y as f32 + 0.5 - s / 2.0);
            let qx = px.abs() - (half - radius);
            let qy = py.abs() - (half - radius);
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let d_box = outside + qx.max(qy).min(0.0) - radius;
            let cover = (0.5 - d_box).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            let mut c = [0x0E as f32, 0x12 as f32, 0x14 as f32];
            let edge_w = (1.4 * px_unit).max(0.9);
            let edge = (1.0 - (d_box + 2.2 * px_unit).abs() / edge_w).clamp(0.0, 1.0);
            let edge_glow = (-(-d_box - 2.0 * px_unit).max(0.0) / (3.0 * px_unit)).exp() * 0.35;
            let mut bar = 0f32;
            let mut bar_glow = 0f32;
            for (i, h) in BARS.iter().enumerate() {
                let bx = (i as f32 - 2.0) * s * 0.135;
                let dy = (py.abs() - h * s).max(0.0);
                let d = ((px - bx).powi(2) + dy * dy).sqrt() - s * 0.045;
                bar = bar.max((0.5 - d).clamp(0.0, 1.0));
                bar_glow = bar_glow.max((-d.max(0.0) / (3.5 * px_unit)).exp() * 0.55);
            }
            let light = (edge_glow + bar_glow).min(1.0);
            for k in 0..3 {
                c[k] += (neon[k] - c[k]) * light;
                c[k] += (neon[k] - c[k]) * edge;
                c[k] += (core[k] - c[k]) * bar;
            }
            let i = (y * n + x) * 4;
            out[i..i + 3].copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8]);
            out[i + 3] = (cover * 255.0) as u8;
        }
    }
    out
}
