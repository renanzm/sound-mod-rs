//! Desenho do icone do app em pixels (RGBA), sem dependencias: usado pela janela (logo.rs) e
//! pelo build.rs, que grava o icone dentro do .exe.
//!
//! Quadrado grafite com borda neon e cinco barras de onda, cada uma acesa numa cor, por campo
//! de distancia (anti-aliasing e brilho de graca).

pub const BARS: [f32; 5] = [0.13, 0.24, 0.32, 0.22, 0.12];

/// Cor de cada barra (tubo, nucleo): verde, ciano, azul, roxo e rosa, da esquerda para a direita.
pub const BAR_COLORS: [([u8; 3], [u8; 3]); 5] = [
    ([0x39, 0xFF, 0x14], [0xC8, 0xFF, 0xB8]),
    ([0x00, 0xE5, 0xFF], [0xC4, 0xF9, 0xFF]),
    ([0x2E, 0x7D, 0xFF], [0xC6, 0xDC, 0xFF]),
    ([0xB0, 0x3C, 0xFF], [0xE8, 0xCC, 0xFF]),
    ([0xFF, 0x2B, 0xD6], [0xFF, 0xC8, 0xF3]),
];

/// Quanto o corpo da barra puxa para o nucleo claro (0 = cor do tubo, 1 = quase branco).
pub const BAR_CORE_MIX: f32 = 0.35;

/// Verde do icone do .exe (o padrao do app).
#[allow(dead_code)] // usado pelo build.rs
pub const GREEN: ([u8; 3], [u8; 3]) = ([0x39, 0xFF, 0x14], [0xC8, 0xFF, 0xB8]);

/// Pixels RGBA (nao pre-multiplicados) de um icone `n` x `n`. `tube`/`filament` = cor da borda;
/// `bars` = (tubo, nucleo) de cada barra.
pub fn rgba(n: usize, (tube, filament): ([u8; 3], [u8; 3]), bars: [([u8; 3], [u8; 3]); 5]) -> Vec<u8> {
    let mut out = vec![0u8; n * n * 4];
    let s = n as f32;
    let px_unit = s / 64.0; // espessuras pensadas para 64 px
    let (half, radius) = (s * 0.46, s * 0.2);
    let neon = tube.map(f32::from);
    let _ = filament; // o nucleo da borda nao aparece no desenho do icone
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
            // barra mais proxima: a cobertura e o brilho usam a cor dela
            let (mut best_d, mut best) = (f32::MAX, 0);
            for (i, h) in BARS.iter().enumerate() {
                let bx = (i as f32 - 2.0) * s * 0.135;
                let dy = (py.abs() - h * s).max(0.0);
                let d = ((px - bx).powi(2) + dy * dy).sqrt() - s * 0.045;
                if d < best_d {
                    (best_d, best) = (d, i);
                }
            }
            let bar = (0.5 - best_d).clamp(0.0, 1.0);
            let bar_glow = (-best_d.max(0.0) / (3.5 * px_unit)).exp() * 0.55;
            let (bt, bc) = bars[best];
            let bar_tube = bt.map(f32::from);
            let bar_body: Vec<f32> = (0..3).map(|k| bar_tube[k] + (bc[k] as f32 - bar_tube[k]) * BAR_CORE_MIX).collect();
            for k in 0..3 {
                c[k] += (neon[k] - c[k]) * edge_glow;
                c[k] += (bar_tube[k] - c[k]) * bar_glow;
                c[k] += (neon[k] - c[k]) * edge;
                c[k] += (bar_body[k] - c[k]) * bar;
            }
            let i = (y * n + x) * 4;
            out[i..i + 3].copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8]);
            out[i + 3] = (cover * 255.0) as u8;
        }
    }
    out
}
