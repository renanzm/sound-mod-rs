//! Mixer: microfone real + sons -> dispositivo de saida (cabo virtual) e monitor (fones).
//!
//! Tudo roda a 48 kHz estereo. As saidas abrem direto nesse formato (o WASAPI converte com
//! AUTOCONVERTPCM); o microfone abre no formato nativo e e reamostrado aqui.
//!
//! Regras: os callbacks de audio nao fazem I/O, quase nao alocam e nunca tocam na interface.
//! `Engine` e compartilhado (`Arc`) entre a interface, as hotkeys e os callbacks; os objetos
//! `cpal::Stream` ficam com quem abriu (a interface) e fechar = soltar esses objetos.

use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, StreamConfig};
use parking_lot::Mutex;

use crate::codec::{self, SR};
use crate::i18n::{t, tf};
use crate::voice::{self, Effect};

pub type ChannelId = u64;
/// Canal do clipe do audio do PC. Os slots usam ids a partir de 1.
pub const CLIP: ChannelId = 0;

/// Streams abertos por `Engine::open`. O microfone e o monitor sao opcionais: se falharem, o
/// resto abre do mesmo jeito (os sons nao dependem deles) e o erro vem em `warnings`.
pub struct OpenedStreams {
    pub streams: Vec<cpal::Stream>,
    pub has_monitor: bool,
    pub warnings: Vec<String>,
}

/// Descarta audio acumulado alem de 250 ms (relogios de dispositivos divergem).
const MAX_LAG: usize = SR as usize / 4;

/// Um som carregado com posicao propria. Toca apenas o trecho [start, end) (em quadros).
struct Channel {
    data: Arc<Vec<f32>>,
    pos: usize,
    /// Parte fracionaria da posicao (quando `rate` != 1).
    frac: f64,
    /// Velocidade de leitura: 2 = uma oitava acima e duas vezes mais rapido.
    rate: f64,
    start: usize,
    end: usize,
    gain: f32,
    playing: bool,
}

impl Channel {
    fn frames(&self) -> usize {
        self.data.len() / 2
    }

    fn rewind(&mut self) {
        self.playing = false;
        self.pos = self.start;
        self.frac = 0.0;
    }

    fn new(data: Arc<Vec<f32>>, gain: f32) -> Self {
        let end = data.len() / 2;
        Self { data, pos: 0, frac: 0.0, rate: 1.0, start: 0, end, gain, playing: false }
    }

    /// Soma ate `frames` quadros estereo em `mix`, avancando a posicao. Retorna se chegou ao fim.
    fn mix_into(&mut self, mix: &mut [f32]) -> bool {
        let frames = mix.len() / 2;
        if self.rate == 1.0 && self.frac == 0.0 {
            let n = frames.min(self.end.saturating_sub(self.pos));
            let src = &self.data[self.pos * 2..(self.pos + n) * 2];
            for (m, v) in mix.iter_mut().zip(src) {
                *m += v * self.gain;
            }
            self.pos += n;
            return self.pos >= self.end;
        }
        // velocidade variavel: interpolacao linear entre o quadro atual e o proximo
        let last = self.end.saturating_sub(1);
        for out in mix.chunks_exact_mut(2) {
            if self.pos >= last {
                self.pos = self.end;
                return true;
            }
            let (i, t) = (self.pos * 2, self.frac as f32);
            let d = &self.data;
            out[0] += (d[i] + (d[i + 2] - d[i]) * t) * self.gain;
            out[1] += (d[i + 1] + (d[i + 3] - d[i + 1]) * t) * self.gain;
            self.frac += self.rate;
            let whole = self.frac.floor();
            self.pos += whole as usize;
            self.frac -= whole;
        }
        self.pos >= last
    }
}

#[derive(Default)]
struct AtomicF32(AtomicU32);

impl AtomicF32 {
    fn new(v: f32) -> Self {
        Self(AtomicU32::new(v.to_bits()))
    }
    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed)
    }
}

pub struct Engine {
    channels: Mutex<HashMap<ChannelId, Channel>>,
    cache: Mutex<HashMap<PathBuf, Arc<Vec<f32>>>>,
    voice: Mutex<Option<Box<dyn Effect>>>,
    mic_buf: Mutex<VecDeque<f32>>,
    mon_buf: Mutex<VecDeque<f32>>,
    pub mix_mic: AtomicBool,
    pub monitor: AtomicBool,
    /// Manda a propria voz (com efeito) para os fones.
    pub hear_self: AtomicBool,
    master: AtomicF32,
    has_monitor: AtomicBool,
    running: AtomicBool,
    device_lost: AtomicBool,
    mic_level: AtomicF32,
    out_level: AtomicF32,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            channels: Mutex::default(),
            cache: Mutex::default(),
            voice: Mutex::new(None),
            mic_buf: Mutex::default(),
            mon_buf: Mutex::default(),
            mix_mic: AtomicBool::new(true),
            monitor: AtomicBool::new(true),
            hear_self: AtomicBool::new(false),
            master: AtomicF32::new(1.0),
            has_monitor: AtomicBool::new(false),
            running: AtomicBool::new(false),
            device_lost: AtomicBool::new(false),
            mic_level: AtomicF32::default(),
            out_level: AtomicF32::default(),
        }
    }
}

impl Engine {
    // ---- ajustes ao vivo ----
    pub fn set_master(&self, v: f32) {
        self.master.set(v);
    }

    pub fn set_voice(&self, name: &str) {
        *self.voice.lock() = voice::make(name);
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Um dispositivo sumiu desde a ultima abertura (lido uma vez).
    pub fn take_device_lost(&self) -> bool {
        self.device_lost.swap(false, Ordering::Relaxed)
    }

    /// Picos (0..1) do ultimo bloco: (microfone com efeito, saida).
    pub fn levels(&self) -> (f32, f32) {
        (self.mic_level.get(), self.out_level.get())
    }

    // ---- sons ----
    pub fn load(&self, path: &Path) -> Result<Arc<Vec<f32>>, String> {
        if let Some(data) = self.cache.lock().get(path) {
            return Ok(data.clone());
        }
        let data = Arc::new(codec::decode(path)?); // fora do lock: pode demorar
        self.cache.lock().insert(path.to_path_buf(), data.clone());
        Ok(data)
    }

    pub fn invalidate(&self, path: &Path) {
        self.cache.lock().remove(path);
    }

    // ---- canais ----
    /// Carrega (decodifica se preciso) e associa ao canal, parado. Retorna a duracao em s.
    pub fn set_sound(&self, cid: ChannelId, path: &Path, gain: f32) -> Result<f64, String> {
        let data = self.load(path)?;
        let frames = data.len() / 2;
        let mut chans = self.channels.lock();
        // o som recarregado mantem a velocidade escolhida (ex.: o tom do clipe)
        let rate = chans.get(&cid).map_or(1.0, |c| c.rate);
        let mut ch = Channel::new(data, gain);
        ch.rate = rate;
        chans.insert(cid, ch);
        Ok(frames as f64 / SR as f64)
    }

    pub fn remove(&self, cid: ChannelId) {
        self.channels.lock().remove(&cid);
    }

    pub fn has(&self, cid: ChannelId) -> bool {
        self.channels.lock().contains_key(&cid)
    }

    /// Audio do canal (estereo intercalado) e trecho [inicio, fim) em segundos.
    pub fn channel_audio(&self, cid: ChannelId) -> Option<(Arc<Vec<f32>>, f64, f64)> {
        let chans = self.channels.lock();
        let ch = chans.get(&cid)?;
        Some((ch.data.clone(), ch.start as f64 / SR as f64, ch.end as f64 / SR as f64))
    }

    /// So um som por vez: para todos os outros canais.
    fn solo(chans: &mut HashMap<ChannelId, Channel>, cid: ChannelId) {
        for (other, ch) in chans.iter_mut() {
            if *other != cid {
                ch.rewind();
            }
        }
    }

    /// Se ja estiver tocando, recomeca; senao continua de onde parou.
    pub fn play(&self, cid: ChannelId) -> bool {
        let mut chans = self.channels.lock();
        let Some(ch) = chans.get_mut(&cid) else { return false };
        if ch.playing || !(ch.start..ch.end).contains(&ch.pos) {
            ch.pos = ch.start;
        }
        ch.playing = true;
        Self::solo(&mut chans, cid);
        true
    }

    /// Tocando -> para; senao toca (continua se estava pausado). None se o canal nao existe.
    pub fn toggle(&self, cid: ChannelId) -> Option<bool> {
        let mut chans = self.channels.lock();
        let ch = chans.get_mut(&cid)?;
        if ch.playing {
            ch.rewind();
            return Some(false);
        }
        if !(ch.start..ch.end).contains(&ch.pos) {
            ch.pos = ch.start;
        }
        ch.playing = true;
        Self::solo(&mut chans, cid);
        Some(true)
    }

    /// Alterna pausa/continuar (nao faz nada se estiver parado no inicio).
    pub fn pause(&self, cid: ChannelId) {
        let mut chans = self.channels.lock();
        let Some(ch) = chans.get_mut(&cid) else { return };
        if !ch.playing && ch.pos == ch.start {
            return;
        }
        ch.playing = !ch.playing;
        if ch.playing {
            Self::solo(&mut chans, cid);
        }
    }

    pub fn stop(&self, cid: ChannelId) {
        if let Some(ch) = self.channels.lock().get_mut(&cid) {
            ch.rewind();
        }
    }

    pub fn stop_all(&self) {
        self.channels.lock().values_mut().for_each(Channel::rewind);
    }

    pub fn seek(&self, cid: ChannelId, seconds: f64) {
        if let Some(ch) = self.channels.lock().get_mut(&cid) {
            let p = (seconds.max(0.0) * SR as f64) as usize;
            ch.pos = p.clamp(ch.start, ch.end.saturating_sub(1).max(ch.start));
        }
    }

    /// Tom ao tocar, em semitons (-12..12): muda a velocidade de leitura (e a duracao junto).
    pub fn set_pitch(&self, cid: ChannelId, semitones: f32) {
        if let Some(ch) = self.channels.lock().get_mut(&cid) {
            ch.rate = 2f64.powf(semitones.clamp(-24.0, 24.0) as f64 / 12.0);
        }
    }

    pub fn set_gain(&self, cid: ChannelId, gain: f32) {
        if let Some(ch) = self.channels.lock().get_mut(&cid) {
            ch.gain = gain;
        }
    }

    pub fn set_range(&self, cid: ChannelId, start_s: f64, end_s: f64) {
        if let Some(ch) = self.channels.lock().get_mut(&cid) {
            let n = ch.frames();
            if n == 0 {
                return;
            }
            ch.start = ((start_s.max(0.0) * SR as f64) as usize).min(n - 1);
            ch.end = ((end_s.max(0.0) * SR as f64) as usize).clamp(ch.start + 1, n);
            if !(ch.start..ch.end).contains(&ch.pos) {
                ch.pos = ch.start;
            }
        }
    }

    /// (posicao_s, duracao_total_s, tocando) ou None.
    pub fn state(&self, cid: ChannelId) -> Option<(f64, f64, bool)> {
        let chans = self.channels.lock();
        let ch = chans.get(&cid)?;
        Some((ch.pos as f64 / SR as f64, ch.frames() as f64 / SR as f64, ch.playing))
    }

    // ---- callbacks ----
    fn push_mic(&self, mono: &[f32]) {
        let mut buf = self.mic_buf.lock();
        buf.extend(mono);
        let extra = buf.len().saturating_sub(MAX_LAG);
        buf.drain(..extra);
    }

    /// Gera `out` (intercalado com `ch` canais) para o cabo virtual.
    fn render(&self, out: &mut [f32], ch: usize, s: &mut Scratch) {
        let frames = out.len() / ch;
        s.mix.clear();
        s.mix.resize(frames * 2, 0.0);
        {
            let mut chans = self.channels.lock();
            for c in chans.values_mut().filter(|c| c.playing) {
                if c.mix_into(&mut s.mix[..frames * 2]) {
                    c.rewind();
                }
            }
        }
        let master = self.master.get();
        s.mix.iter_mut().for_each(|m| *m *= master);

        let mix_mic = self.mix_mic.load(Ordering::Relaxed);
        let hear_self = self.hear_self.load(Ordering::Relaxed);
        s.mic.clear();
        s.mic.resize(frames, 0.0);
        if mix_mic || hear_self {
            {
                let mut buf = self.mic_buf.lock();
                let got = frames.min(buf.len());
                for (d, v) in s.mic.iter_mut().zip(buf.drain(..got)) {
                    *d = v;
                }
            }
            let mut fx = self.voice.lock();
            if let Some(effect) = fx.as_mut() {
                // um efeito com defeito nao pode derrubar o stream: desliga o efeito
                if catch_unwind(AssertUnwindSafe(|| effect.process(&mut s.mic))).is_err() {
                    *fx = None;
                }
            }
            self.mic_level.set(peak(&s.mic));
        } else {
            self.mic_level.set(0.0);
        }

        if self.monitor.load(Ordering::Relaxed) && self.has_monitor.load(Ordering::Relaxed) {
            let mut buf = self.mon_buf.lock();
            for (i, pair) in s.mix.chunks_exact(2).enumerate() {
                let v = if hear_self { s.mic[i] } else { 0.0 };
                buf.push_back((pair[0] + v).clamp(-1.0, 1.0));
                buf.push_back((pair[1] + v).clamp(-1.0, 1.0));
            }
            let extra = buf.len().saturating_sub(MAX_LAG * 2);
            buf.drain(..extra);
        }

        if mix_mic {
            for (pair, v) in s.mix.chunks_exact_mut(2).zip(&s.mic) {
                pair[0] += v;
                pair[1] += v;
            }
        }
        self.out_level.set(peak(&s.mix));
        write_frames(out, ch, &s.mix);
    }

    fn render_monitor(&self, out: &mut [f32], ch: usize, s: &mut Scratch) {
        let frames = out.len() / ch;
        s.mix.clear();
        s.mix.resize(frames * 2, 0.0);
        let mut buf = self.mon_buf.lock();
        let got = (frames * 2).min(buf.len() / 2 * 2);
        for (d, v) in s.mix.iter_mut().zip(buf.drain(..got)) {
            *d = v;
        }
        drop(buf);
        write_frames(out, ch, &s.mix);
    }

    fn reset_buffers(&self) {
        self.mic_buf.lock().clear();
        self.mon_buf.lock().clear();
        self.mic_level.set(0.0);
        self.out_level.set(0.0);
    }

    // ---- streams ----
    /// Fecha os streams anteriores (soltando-os) e marca o engine como parado.
    pub fn close(&self, streams: &mut Vec<cpal::Stream>) {
        streams.clear();
        self.running.store(false, Ordering::Relaxed);
        self.has_monitor.store(false, Ordering::Relaxed);
        self.reset_buffers();
    }

    /// Abre microfone (opcional), saida (obrigatoria) e monitor (opcional), por nome.
    ///
    /// Pode demorar ou travar se um driver nao responder (o Windows nao tem timeout para isso),
    /// entao roda fora da thread da janela. `step` recebe o nome do dispositivo sendo aberto.
    /// Nao liga o engine: quem recebe os streams chama `activate`.
    pub fn open(
        self: &Arc<Self>,
        mic: Option<&str>,
        out: Option<&str>,
        mon: Option<&str>,
        step: &Mutex<String>,
    ) -> Result<OpenedStreams, String> {
        let out = out.ok_or_else(|| t("select_output").to_string())?;
        let host = cpal::default_host();
        let mut streams = Vec::new();
        let mut warnings = Vec::new();
        if let Some(name) = mic {
            *step.lock() = name.to_string();
            trace(&format!("abrindo microfone: {name}"));
            match find_device(&host, name, true).and_then(|dev| self.mic_stream(&dev)) {
                Ok(s) => streams.push(s),
                Err(e) => {
                    trace(&format!("erro no microfone: {e}"));
                    warnings.push(tf("mic_error", &[("e", &e)]));
                }
            }
        }
        // a saida para o cabo e a unica obrigatoria
        *step.lock() = out.to_string();
        trace(&format!("abrindo saida: {out}"));
        let dev = find_device(&host, out, false)?;
        streams.push(self.output_stream(&dev, false)?);
        let mut has_monitor = false;
        if let Some(name) = mon {
            *step.lock() = name.to_string();
            trace(&format!("abrindo monitor: {name}"));
            match find_device(&host, name, false).and_then(|dev| self.output_stream(&dev, true)) {
                Ok(s) => {
                    streams.push(s);
                    has_monitor = true;
                }
                Err(e) => {
                    trace(&format!("erro no monitor: {e}"));
                    warnings.push(tf("monitor_error", &[("e", &e)]));
                }
            }
        }
        self.reset_buffers();
        for s in &streams {
            trace("iniciando stream");
            s.play().map_err(|e| e.to_string())?;
        }
        Ok(OpenedStreams { streams, has_monitor, warnings })
    }

    /// Liga o engine com os streams recem-abertos (`open`).
    pub fn activate(&self, has_monitor: bool) {
        self.has_monitor.store(has_monitor, Ordering::Relaxed);
        self.running.store(true, Ordering::Relaxed);
        self.device_lost.store(false, Ordering::Relaxed);
    }

    fn error_handler(self: &Arc<Self>) -> impl FnMut(cpal::Error) + Send + 'static {
        let engine = Arc::downgrade(self);
        move |err| {
            eprintln!("audio: {err}");
            if let Some(e) = engine.upgrade() {
                e.device_lost.store(true, Ordering::Relaxed);
            }
        }
    }

    fn output_stream(self: &Arc<Self>, dev: &cpal::Device, monitor: bool) -> Result<cpal::Stream, String> {
        // 48 kHz estereo float: o WASAPI converte para o formato do dispositivo
        let mut last_err = String::new();
        let native = dev.default_output_config().map(|c| c.channels()).unwrap_or(2);
        for channels in [2, native] {
            let config = StreamConfig { channels, sample_rate: SR, buffer_size: cpal::BufferSize::Default };
            let engine = self.clone();
            let ch = channels as usize;
            let mut scratch = Scratch::default();
            let result = dev.build_output_stream::<f32, _, _>(
                config,
                move |data: &mut [f32], _| {
                    if monitor {
                        engine.render_monitor(data, ch, &mut scratch)
                    } else {
                        engine.render(data, ch, &mut scratch)
                    }
                },
                self.error_handler(),
                None,
            );
            match result {
                Ok(s) => return Ok(s),
                Err(e) => last_err = e.to_string(),
            }
        }
        Err(last_err)
    }

    fn mic_stream(self: &Arc<Self>, dev: &cpal::Device) -> Result<cpal::Stream, String> {
        let supported = dev.default_input_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        match supported.sample_format() {
            SampleFormat::F32 => self.mic_stream_typed::<f32>(dev, config),
            SampleFormat::I16 => self.mic_stream_typed::<i16>(dev, config),
            SampleFormat::I32 => self.mic_stream_typed::<i32>(dev, config),
            SampleFormat::U8 => self.mic_stream_typed::<u8>(dev, config),
            SampleFormat::F64 => self.mic_stream_typed::<f64>(dev, config),
            other => Err(format!("{other:?}")),
        }
    }

    fn mic_stream_typed<T>(self: &Arc<Self>, dev: &cpal::Device, config: StreamConfig) -> Result<cpal::Stream, String>
    where
        T: SizedSample + Send + 'static,
        f32: FromSample<T>,
    {
        let engine = self.clone();
        let ch = config.channels.max(1) as usize;
        let mut resampler = LinearResampler::new(config.sample_rate, SR);
        let mut mono = Vec::new();
        let mut out = Vec::new();
        dev.build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                mono.clear();
                mono.extend(
                    data.chunks_exact(ch)
                        .map(|f| f.iter().map(|&s| f32::from_sample(s)).sum::<f32>() / ch as f32),
                );
                out.clear();
                resampler.process(&mono, &mut out);
                engine.push_mic(&out);
            },
            self.error_handler(),
            None,
        )
        .map_err(|e| e.to_string())
    }
}

#[derive(Default)]
struct Scratch {
    mix: Vec<f32>,
    mic: Vec<f32>,
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0f32, |m, v| m.max(v.abs())).min(1.0)
}

/// Estereo intercalado -> `ch` canais, com limite em [-1, 1].
fn write_frames(out: &mut [f32], ch: usize, stereo: &[f32]) {
    match ch {
        2 => {
            for (o, v) in out.iter_mut().zip(stereo) {
                *o = v.clamp(-1.0, 1.0);
            }
        }
        1 => {
            for (o, pair) in out.iter_mut().zip(stereo.chunks_exact(2)) {
                *o = ((pair[0] + pair[1]) * 0.5).clamp(-1.0, 1.0);
            }
        }
        _ => {
            for (frame, pair) in out.chunks_exact_mut(ch).zip(stereo.chunks_exact(2)) {
                frame.fill(0.0);
                frame[0] = pair[0].clamp(-1.0, 1.0);
                frame[1] = pair[1].clamp(-1.0, 1.0);
            }
        }
    }
}

/// Reamostragem linear em fluxo (mono), para o microfone em tempo real.
struct LinearResampler {
    step: f64,
    pos: f64,
    prev: f32,
}

impl LinearResampler {
    fn new(from: u32, to: u32) -> Self {
        Self { step: from as f64 / to as f64, pos: 0.0, prev: 0.0 }
    }

    fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let n = input.len();
        if n == 0 {
            return;
        }
        if self.step == 1.0 {
            out.extend_from_slice(input);
            return;
        }
        // posicao -1 = ultima amostra do bloco anterior
        let at = |i: isize| if i < 0 { self.prev } else { input[i as usize] };
        while self.pos < (n - 1) as f64 {
            let i = self.pos.floor();
            let frac = (self.pos - i) as f32;
            let (a, b) = (at(i as isize), at(i as isize + 1));
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        self.pos -= n as f64;
        self.prev = input[n - 1];
    }
}

// ---- dispositivos ----

pub fn device_name(dev: &cpal::Device) -> Option<String> {
    dev.description().ok().map(|d| d.name().to_string())
}

/// Nomes dos dispositivos WASAPI: (entradas, saidas).
pub fn list_devices() -> (Vec<String>, Vec<String>) {
    let host = cpal::default_host();
    let names = |devs: Option<Vec<cpal::Device>>| -> Vec<String> {
        let mut v: Vec<String> = devs.unwrap_or_default().iter().filter_map(device_name).collect();
        v.dedup();
        v
    };
    (
        names(host.input_devices().ok().map(|d| d.collect())),
        names(host.output_devices().ok().map(|d| d.collect())),
    )
}

pub fn default_device_names() -> (Option<String>, Option<String>) {
    let host = cpal::default_host();
    (
        host.default_input_device().as_ref().and_then(device_name),
        host.default_output_device().as_ref().and_then(device_name),
    )
}

/// `SOUNDMOD_DEBUG_AUDIO=1`: mostra no stderr cada passo da abertura dos dispositivos.
pub fn trace(msg: &str) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var_os("SOUNDMOD_DEBUG_AUDIO").is_some()) {
        eprintln!("audio: {msg}");
    }
}

pub fn find_device(host: &cpal::Host, name: &str, input: bool) -> Result<cpal::Device, String> {
    let devices = if input { host.input_devices() } else { host.output_devices() };
    devices
        .map_err(|e| e.to_string())?
        .find(|d| device_name(d).as_deref() == Some(name))
        .ok_or_else(|| tf("device_missing", &[("name", &name)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine_with(cid: ChannelId, frames: usize) -> Arc<Engine> {
        let e = Arc::new(Engine::default());
        let data = Arc::new((0..frames * 2).map(|i| if i % 2 == 0 { 0.5 } else { -0.5 }).collect::<Vec<f32>>());
        e.channels.lock().insert(cid, Channel::new(data, 1.0));
        e
    }

    #[test]
    fn toggle_play_stop_and_solo() {
        let e = engine_with(1, 4800);
        let d = Arc::new(vec![0.1f32; 9600]);
        e.channels.lock().insert(2, Channel::new(d, 1.0));
        assert_eq!(e.toggle(1), Some(true));
        assert_eq!(e.toggle(2), Some(true));
        assert!(!e.state(1).unwrap().2, "playing another sound stops the first");
        assert_eq!(e.toggle(2), Some(false));
        assert_eq!(e.toggle(99), None);
    }

    #[test]
    fn render_mixes_and_rewinds_at_end() {
        let e = engine_with(1, 480);
        e.mix_mic.store(false, Ordering::Relaxed);
        e.play(1);
        let mut out = vec![0f32; 960];
        let mut s = Scratch::default();
        e.render(&mut out, 2, &mut s);
        assert_eq!(&out[..4], &[0.5, -0.5, 0.5, -0.5]);
        let (pos, _, playing) = e.state(1).unwrap();
        assert!(!playing && pos == 0.0);
    }

    #[test]
    fn pause_resume_seek_and_range() {
        let e = engine_with(1, 48_000);
        e.play(1);
        e.seek(1, 0.5);
        assert!((e.state(1).unwrap().0 - 0.5).abs() < 1e-6);
        e.pause(1);
        assert!(!e.state(1).unwrap().2);
        e.pause(1);
        assert!(e.state(1).unwrap().2);
        e.set_range(1, 0.2, 0.3);
        let (_, s, en) = e.channel_audio(1).map(|(_, s, en)| ((), s, en)).unwrap();
        assert!((s - 0.2).abs() < 1e-6 && (en - 0.3).abs() < 1e-6);
        assert!(e.state(1).unwrap().0 >= 0.2);
    }

    #[test]
    fn mic_goes_to_output_and_monitor() {
        let e = Arc::new(Engine::default());
        e.has_monitor.store(true, Ordering::Relaxed);
        e.hear_self.store(true, Ordering::Relaxed);
        e.push_mic(&[0.25; 480]);
        let mut out = vec![0f32; 960];
        e.render(&mut out, 2, &mut Scratch::default());
        assert!(out.iter().all(|v| (*v - 0.25).abs() < 1e-6));
        let mut mon = vec![0f32; 960];
        e.render_monitor(&mut mon, 2, &mut Scratch::default());
        assert!(mon.iter().all(|v| (*v - 0.25).abs() < 1e-6));
    }

    #[test]
    fn pitch_changes_frequency_and_speed() {
        // 1 s de 440 Hz; +12 semitons = 880 Hz e metade da duracao
        let n = SR as usize;
        let tone: Vec<f32> = (0..n)
            .flat_map(|i| {
                let v = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / SR as f32).sin() * 0.5;
                [v, v]
            })
            .collect();
        let e = Arc::new(Engine::default());
        e.mix_mic.store(false, Ordering::Relaxed);
        e.channels.lock().insert(1, Channel::new(Arc::new(tone), 1.0));
        e.set_pitch(1, 12.0);
        e.play(1);
        let mut out = vec![0f32; 480 * 2];
        let mut left = Vec::new();
        let mut s = Scratch::default();
        for _ in 0..200 {
            e.render(&mut out, 2, &mut s);
            left.extend(out.iter().step_by(2).copied());
            if !e.state(1).unwrap().2 {
                break;
            }
        }
        let played = left.iter().rposition(|v| v.abs() > 1e-4).unwrap() + 1;
        assert!((played as i64 - n as i64 / 2).abs() < 960, "duracao {played}");
        let seg = &left[4800..4800 + 9600];
        let rising = seg.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        let freq = rising as f32 / 0.2;
        assert!((freq - 880.0).abs() < 15.0, "{freq} Hz");
        // -12: oitava abaixo
        e.set_pitch(1, -12.0);
        e.play(1);
        let mut low = Vec::new();
        for _ in 0..20 {
            e.render(&mut out, 2, &mut s);
            low.extend(out.iter().step_by(2).copied());
        }
        let rising = low[..9600].windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((rising as f32 / 0.2 - 220.0).abs() < 15.0, "{} Hz", rising as f32 / 0.2);
    }

    #[test]
    fn mic_lag_is_capped() {
        let e = Engine::default();
        e.push_mic(&vec![0.0; SR as usize]);
        assert_eq!(e.mic_buf.lock().len(), MAX_LAG);
    }

    /// Teste real com o VB-CABLE: toca um tom baixo no "CABLE Input", grava pelo loopback do
    /// mesmo dispositivo (recorder) e confere a frequencia no WAV salvo. Nao sai nos alto-falantes.
    /// Rodar com `cargo test --release -- --ignored`.
    #[test]
    #[ignore]
    fn cable_roundtrip() {
        let (_, outs) = list_devices();
        let Some(cable) = outs.iter().find(|n| n.contains("CABLE Input")).cloned() else {
            eprintln!("VB-CABLE nao instalado: teste ignorado");
            return;
        };
        let e = Arc::new(Engine::default());
        e.mix_mic.store(false, Ordering::Relaxed);
        let n = (SR as f32 * 0.6) as usize;
        let tone: Vec<f32> = (0..n)
            .flat_map(|i| {
                let v = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SR as f32).sin() * 0.1;
                [v, v]
            })
            .collect();
        e.channels.lock().insert(1, Channel::new(Arc::new(tone), 1.0));
        let mut streams = e.open(None, Some(&cable), None, &Mutex::new(String::new())).expect("abrir saida").streams;
        e.activate(false);
        let rec = Arc::new(crate::recorder::Recorder::new(5));
        let _loopback = rec.start(Some(&cable)).expect("abrir loopback");
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(e.play(1));
        std::thread::sleep(std::time::Duration::from_millis(1000));
        assert!(!e.state(1).unwrap().2, "o som deveria ter terminado");
        let path = std::env::temp_dir().join("soundmod-roundtrip.wav");
        let secs = rec.save(&path).expect("salvar clipe");
        e.close(&mut streams);
        let data = codec::decode(&path).expect("ler clipe");
        let left: Vec<f32> = data.iter().step_by(2).copied().collect();
        let start = left.iter().position(|v| v.abs() > 0.05).expect("o tom nao chegou ao loopback");
        let seg = &left[start + 2400..start + 2400 + 14_400]; // 0,3 s no meio do tom
        let rising = seg.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        let freq = rising as f32 / 0.3;
        let peak = seg.iter().fold(0f32, |m, v| m.max(v.abs()));
        eprintln!("clipe {secs:.2}s, {freq:.0} Hz, pico {peak:.3}");
        assert!((freq - 1000.0).abs() < 15.0, "{freq} Hz");
        assert!((peak - 0.1).abs() < 0.03, "pico {peak}");
    }

    #[test]
    fn linear_resampler_rate() {
        let mut r = LinearResampler::new(44_100, 48_000);
        let mut out = Vec::new();
        for _ in 0..100 {
            r.process(&[0.0; 441], &mut out);
        }
        assert!((out.len() as i64 - 48_000).abs() <= 2, "{}", out.len());
    }
}
