//! Efeitos de voz em tempo real para o microfone.
//!
//! Cada efeito processa um bloco mono f32 (qualquer tamanho) no lugar e guarda o proprio estado
//! entre blocos. Roda dentro do callback de audio: nada de I/O e quase nenhuma alocacao.

use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use crate::codec::SR;

pub const EFFECTS: [&str; 9] = ["normal", "deep", "high", "robot", "ogre", "autotune", "echo", "radio", "alien"];

pub trait Effect: Send {
    fn process(&mut self, x: &mut [f32]);
}

/// Cria o efeito pelo nome. "normal" (ou desconhecido) = None, sem processamento.
pub fn make(name: &str) -> Option<Box<dyn Effect>> {
    Some(match name {
        "deep" => Box::new(PitchShifter::new(0.75, 2048)),
        "high" => Box::new(PitchShifter::new(1.45, 2048)),
        "robot" => Box::new(Robot::new()),
        "ogre" => Box::new(Chain(vec![
            Box::new(PitchShifter::new(0.62, 2048)),
            Box::new(Saturate { pre: 0.6, drive: 2.5 }),
            Box::new(Fir::new(lowpass(2200.0, 101))),
        ])),
        "autotune" => Box::new(Autotune::new()),
        "echo" => Box::new(Echo::new(0.28, 0.45)),
        "radio" => Box::new(Chain(vec![
            Box::new(Fir::new(bandpass(400.0, 3000.0, 481))),
            Box::new(Saturate { pre: 0.8, drive: 2.0 }),
        ])),
        "alien" => Box::new(Vibrato::new()),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------

/// Muda o tom com uma linha de atraso lida por dois cursores em contrafase.
///
/// O atraso de cada cursor varia de 0 a `window` amostras a uma taxa (1 - razao), entao a
/// leitura anda na velocidade `razao`. As janelas sin^2 dos dois cursores somam 1, o que
/// esconde o salto quando um cursor volta ao inicio. Latencia media: window/2.
pub struct PitchShifter {
    ratio: f32,
    window: f64,
    buf: Vec<f32>,
    written: u64,
    phase: f64,
}

impl PitchShifter {
    const SIZE: usize = 1 << 14;

    pub fn new(ratio: f32, window: usize) -> Self {
        Self { ratio, window: window as f64, buf: vec![0.0; Self::SIZE], written: 0, phase: 0.0 }
    }

    /// Processa com a razao indo do valor atual ate `ratio` ao longo do bloco (sem estalos).
    pub fn process_to(&mut self, x: &mut [f32], ratio: f32) {
        let n = x.len();
        if n == 0 {
            return;
        }
        let mask = Self::SIZE as u64 - 1;
        let (r0, r1) = (self.ratio as f64, ratio as f64);
        self.ratio = ratio;
        for (k, s) in x.iter_mut().enumerate() {
            let now = self.written + k as u64;
            self.buf[(now & mask) as usize] = *s;
            let rate = r0 + (r1 - r0) * (k + 1) as f64 / n as f64;
            self.phase = (self.phase + (1.0 - rate) / self.window).rem_euclid(1.0);
            let mut out = 0.0f32;
            for offset in [0.0, 0.5] {
                let p = (self.phase + offset) % 1.0;
                let pos = now as f64 - (p * self.window + 1.0); // +1: nunca le alem da amostra atual
                let i = pos.floor();
                let frac = (pos - i) as f32;
                let i = i as i64 as u64;
                let a = self.buf[(i & mask) as usize];
                let b = self.buf[(i.wrapping_add(1) & mask) as usize];
                let w = (std::f64::consts::PI * p).sin().powi(2) as f32;
                out += (a + (b - a) * frac) * w;
            }
            *s = out;
        }
        self.written += n as u64;
    }
}

impl Effect for PitchShifter {
    fn process(&mut self, x: &mut [f32]) {
        let r = self.ratio;
        self.process_to(x, r);
    }
}

/// Filtro FIR com memoria entre blocos.
pub struct Fir {
    h: Vec<f32>,
    hist: Vec<f32>,
    z: Vec<f32>,
}

impl Fir {
    pub fn new(taps: Vec<f32>) -> Self {
        let n = taps.len();
        Self { h: taps, hist: vec![0.0; n - 1], z: Vec::new() }
    }
}

impl Effect for Fir {
    fn process(&mut self, x: &mut [f32]) {
        let m = self.hist.len();
        self.z.clear();
        self.z.extend_from_slice(&self.hist);
        self.z.extend_from_slice(x);
        for (k, out) in x.iter_mut().enumerate() {
            // h e simetrico (fase linear), entao correlacao = convolucao
            *out = self.z[k..k + m + 1].iter().zip(&self.h).map(|(a, b)| a * b).sum();
        }
        let len = self.z.len();
        self.hist.copy_from_slice(&self.z[len - m..]);
    }
}

fn hamming(n: usize) -> impl Iterator<Item = f32> {
    (0..n).map(move |i| 0.54 - 0.46 * (2.0 * PI * i as f32 / (n - 1) as f32).cos())
}

fn sinc(x: f32) -> f32 {
    if x.abs() < 1e-9 { 1.0 } else { (PI * x).sin() / (PI * x) }
}

pub fn lowpass(fc: f32, taps: usize) -> Vec<f32> {
    let mid = (taps - 1) as f32 / 2.0;
    let h: Vec<f32> =
        hamming(taps).enumerate().map(|(i, w)| sinc(2.0 * fc / SR as f32 * (i as f32 - mid)) * w).collect();
    let sum: f32 = h.iter().sum();
    h.into_iter().map(|v| v / sum).collect()
}

pub fn bandpass(lo: f32, hi: f32, taps: usize) -> Vec<f32> {
    lowpass(hi, taps).into_iter().zip(lowpass(lo, taps)).map(|(a, b)| a - b).collect()
}

/// Saturacao suave: tanh(drive * pre * x) / tanh(drive).
pub struct Saturate {
    pub pre: f32,
    pub drive: f32,
}

impl Effect for Saturate {
    fn process(&mut self, x: &mut [f32]) {
        let norm = self.drive.tanh();
        for s in x {
            *s = (self.drive * self.pre * *s).tanh() / norm;
        }
    }
}

/// Eco com realimentacao.
pub struct Echo {
    delay: u64,
    feedback: f32,
    buf: Vec<f32>,
    written: u64,
}

impl Echo {
    const SIZE: usize = 1 << 16;

    pub fn new(delay_s: f32, feedback: f32) -> Self {
        Self { delay: (delay_s * SR as f32) as u64, feedback, buf: vec![0.0; Self::SIZE], written: 0 }
    }
}

impl Effect for Echo {
    fn process(&mut self, x: &mut [f32]) {
        let mask = Self::SIZE as u64 - 1;
        for s in x {
            let k = self.written;
            let y = *s + self.feedback * self.buf[(k.wrapping_sub(self.delay) & mask) as usize];
            self.buf[(k & mask) as usize] = y;
            *s = y;
            self.written += 1;
        }
    }
}

/// Robotizacao: STFT com a fase zerada em cada quadro.
///
/// Todos os quadros viram pulsos alinhados, entao a voz sai monotona no tom SR/HOP (~188 Hz)
/// mantendo as palavras (envelope espectral). Latencia: um quadro (~21 ms).
pub struct Robot {
    fwd: Arc<dyn RealToComplex<f32>>,
    inv: Arc<dyn ComplexToReal<f32>>,
    win: Vec<f32>,
    frame: Vec<f32>,
    ola: Vec<f32>,
    pending: Vec<f32>,
    ready: VecDeque<f32>,
    input: Vec<f32>,
    spec: Vec<Complex<f32>>,
    output: Vec<f32>,
}

impl Robot {
    const N: usize = 1024;
    const HOP: usize = 256;

    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(Self::N);
        let inv = planner.plan_fft_inverse(Self::N);
        let spec = fwd.make_output_vec();
        // Hann periodica
        let win = (0..Self::N).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / Self::N as f32).cos()).collect();
        Self {
            fwd,
            inv,
            win,
            frame: vec![0.0; Self::N],
            ola: vec![0.0; Self::N],
            pending: Vec::with_capacity(4096),
            ready: VecDeque::from(vec![0.0; Self::N]), // pre-enchido = latencia fixa
            input: vec![0.0; Self::N],
            spec,
            output: vec![0.0; Self::N],
        }
    }
}

impl Effect for Robot {
    fn process(&mut self, x: &mut [f32]) {
        let (n, hop) = (Self::N, Self::HOP);
        // soma de Hann^2 com sobreposicao de 4x = 1.5; o irfft do numpy divide por N
        let norm = 1.0 / 1.5 / n as f32;
        self.pending.extend_from_slice(x);
        let mut used = 0;
        while self.pending.len() - used >= hop {
            self.frame.copy_within(hop.., 0);
            self.frame[n - hop..].copy_from_slice(&self.pending[used..used + hop]);
            used += hop;
            for ((i, f), w) in self.input.iter_mut().zip(&self.frame).zip(&self.win) {
                *i = f * w;
            }
            let _ = self.fwd.process(&mut self.input, &mut self.spec);
            for c in self.spec.iter_mut() {
                *c = Complex::new(c.norm(), 0.0);
            }
            let _ = self.inv.process(&mut self.spec, &mut self.output);
            for i in 0..n {
                let pulse = self.output[(i + n / 2) % n]; // fftshift
                self.ola[i] += pulse * self.win[i] * norm;
            }
            self.ready.extend(&self.ola[..hop]);
            self.ola.copy_within(hop.., 0);
            self.ola[n - hop..].fill(0.0);
        }
        self.pending.drain(..used);
        for s in x {
            *s = self.ready.pop_front().unwrap_or(0.0);
        }
    }
}

/// Frequencia fundamental por autocorrelacao, ou None se for silencio/ruido.
pub struct PitchDetector {
    fwd: Arc<dyn RealToComplex<f32>>,
    inv: Arc<dyn ComplexToReal<f32>>,
    input: Vec<f32>,
    spec: Vec<Complex<f32>>,
    ac: Vec<f32>,
}

impl PitchDetector {
    pub fn new(n: usize) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(2 * n);
        let inv = planner.plan_fft_inverse(2 * n);
        let spec = fwd.make_output_vec();
        Self { fwd, inv, input: vec![0.0; 2 * n], spec, ac: vec![0.0; 2 * n] }
    }

    pub fn detect(&mut self, x: &[f32], fmin: f32, fmax: f32) -> Option<f32> {
        let n = x.len();
        let mean = x.iter().sum::<f32>() / n as f32;
        let power = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / n as f32;
        if power < 1e-5 {
            return None;
        }
        self.input.fill(0.0);
        for (d, v) in self.input.iter_mut().zip(x) {
            *d = v - mean;
        }
        self.fwd.process(&mut self.input, &mut self.spec).ok()?;
        for c in self.spec.iter_mut() {
            *c = Complex::new(c.norm_sqr(), 0.0);
        }
        self.inv.process(&mut self.spec, &mut self.ac).ok()?;
        let ac = &self.ac[..n];
        let lo = (SR as f32 / fmax) as usize;
        let hi = ((SR as f32 / fmin) as usize).min(n - 2);
        // comeca depois da primeira queda abaixo de zero, para nao pegar a cauda do lag 0
        let start = lo + ac[lo..hi].iter().position(|&v| v < 0.0).unwrap_or(0);
        if start >= hi {
            return None;
        }
        let lag = start + ac[start..hi].iter().enumerate().fold(0, |best, (i, &v)| if v > ac[start + best] { i } else { best });
        if ac[lag] < 0.5 * ac[0] {
            return None; // pouco periodico: consoante, ruido
        }
        let (a, b, c) = (ac[lag - 1], ac[lag], ac[lag + 1]);
        let den = a - 2.0 * b + c;
        let exact = lag as f32 + if den != 0.0 { 0.5 * (a - c) / den } else { 0.0 };
        Some(SR as f32 / exact)
    }
}

/// Puxa a voz para a nota (semitom) mais proxima.
///
/// Com histerese: a nota atual so muda quando a voz se afasta mais de HOLD semitons dela.
/// Sem isso, uma voz perto da fronteira entre duas notas fica alternando (soa tremida).
pub struct Autotune {
    shift: PitchShifter,
    detector: PitchDetector,
    history: Vec<f32>,
    speed: f32,
    ratio: f32,
    note: Option<f32>,
}

impl Autotune {
    const HOLD: f32 = 0.6;
    const HISTORY: usize = 2048;

    pub fn new() -> Self {
        Self {
            shift: PitchShifter::new(1.0, 1024),
            detector: PitchDetector::new(Self::HISTORY),
            history: vec![0.0; Self::HISTORY],
            speed: 0.6,
            ratio: 1.0,
            note: None,
        }
    }
}

impl Effect for Autotune {
    fn process(&mut self, x: &mut [f32]) {
        let h = Self::HISTORY;
        if x.len() >= h {
            self.history.copy_from_slice(&x[x.len() - h..]);
        } else {
            self.history.copy_within(x.len().., 0);
            self.history[h - x.len()..].copy_from_slice(x);
        }
        let mut target = 1.0;
        match self.detector.detect(&self.history, 70.0, 1000.0) {
            Some(f0) => {
                let midi = 69.0 + 12.0 * (f0 / 440.0).log2();
                let note = match self.note {
                    Some(n) if (midi - n).abs() <= Self::HOLD => n,
                    _ => midi.round(),
                };
                self.note = Some(note);
                target = 2f32.powf((note - midi) / 12.0);
            }
            None => self.note = None,
        }
        self.ratio += self.speed * (target - self.ratio);
        let r = self.ratio;
        self.shift.process_to(x, r);
    }
}

pub struct Chain(pub Vec<Box<dyn Effect>>);

impl Effect for Chain {
    fn process(&mut self, x: &mut [f32]) {
        for step in &mut self.0 {
            step.process(x);
        }
    }
}

/// Pitch que oscila (voz de alien).
pub struct Vibrato {
    shift: PitchShifter,
    ratio: f32,
    depth: f32,
    rate: f32,
    t: f64,
}

impl Vibrato {
    pub fn new() -> Self {
        Self { shift: PitchShifter::new(1.3, 1536), ratio: 1.3, depth: 0.12, rate: 5.0, t: 0.0 }
    }
}

impl Effect for Vibrato {
    fn process(&mut self, x: &mut [f32]) {
        self.t += x.len() as f64 / SR as f64;
        let r = self.ratio + self.depth * (2.0 * std::f64::consts::PI * self.rate as f64 * self.t).sin() as f32;
        self.shift.process_to(x, r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| (2.0 * PI * freq * i as f32 / SR as f32).sin() * 0.5).collect()
    }

    /// Frequencia dominante por cruzamentos por zero (sinal ja estabilizado).
    fn freq(x: &[f32]) -> f32 {
        let crossings = x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        crossings as f32 * SR as f32 / x.len() as f32
    }

    fn run(fx: &mut dyn Effect, x: &[f32]) -> Vec<f32> {
        // blocos irregulares, como os do WASAPI
        let mut out = x.to_vec();
        let mut i = 0;
        for size in [480usize, 441, 512, 160, 1024].iter().cycle() {
            if i >= out.len() {
                break;
            }
            let end = (i + size).min(out.len());
            fx.process(&mut out[i..end]);
            i = end;
        }
        out
    }

    #[test]
    fn pitch_shifter_changes_frequency() {
        let x = tone(220.0, SR as usize);
        for (ratio, want) in [(0.75, 165.0), (1.45, 319.0)] {
            let y = run(&mut PitchShifter::new(ratio, 2048), &x);
            let f = freq(&y[SR as usize / 4..]);
            assert!((f - want).abs() < want * 0.06, "ratio {ratio}: {f} Hz");
        }
    }

    #[test]
    fn every_effect_is_finite_and_keeps_size() {
        let x = tone(200.0, SR as usize / 2);
        for name in EFFECTS {
            let Some(mut fx) = make(name) else { continue };
            let y = run(fx.as_mut(), &x);
            assert_eq!(y.len(), x.len());
            assert!(y.iter().all(|v| v.is_finite()), "{name}");
            assert!(y.iter().any(|v| v.abs() > 0.01), "{name} silenced the signal");
        }
    }

    #[test]
    fn silence_stays_silent() {
        let x = vec![0.0; 9600];
        for name in EFFECTS {
            let Some(mut fx) = make(name) else { continue };
            let y = run(fx.as_mut(), &x);
            assert!(y.iter().all(|v| v.abs() < 1e-6), "{name}");
        }
    }

    #[test]
    fn detects_pitch() {
        let mut d = PitchDetector::new(2048);
        let f = d.detect(&tone(196.0, 2048), 70.0, 1000.0).unwrap();
        assert!((f - 196.0).abs() < 2.0, "{f}");
        assert!(d.detect(&vec![0.0; 2048], 70.0, 1000.0).is_none());
    }

    #[test]
    fn autotune_snaps_to_semitone() {
        // 210 Hz fica entre G#3 (207.65) e A3 (220): deve ir para 207.65
        let x = tone(210.0, SR as usize);
        let y = run(&mut Autotune::new(), &x);
        let f = freq(&y[SR as usize / 2..]);
        assert!((f - 207.65).abs() < 3.0, "{f}");
    }
}
