//! Decodifica arquivos de audio e grava WAV.
//!
//! Formato interno unico: float32 estereo intercalado a 48 kHz (`[l, r, l, r, ...]`).
//! A decodificacao usa o Symphonia (Rust puro: mp3, wav, ogg/vorbis, flac, m4a/aac, alac, aiff,
//! caf, mkv/webm). Para o que ele nao abrir (ex.: wma), usa o `ffmpeg.exe` se existir ao lado do
//! programa ou no PATH.

use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::config;
use crate::i18n::t;

pub const SR: u32 = 48_000;

/// Extensoes oferecidas no seletor de arquivos.
pub const AUDIO_EXTENSIONS: [&str; 12] =
    ["mp3", "wav", "ogg", "oga", "flac", "m4a", "aac", "mp4", "opus", "aif", "aiff", "wma"];

/// Qualquer formato -> estereo intercalado a 48 kHz.
pub fn decode(path: &Path) -> Result<Vec<f32>, String> {
    match decode_symphonia(path) {
        Ok((data, rate)) if !data.is_empty() => Ok(resample(&data, 2, rate, SR)),
        first => match decode_ffmpeg(path) {
            Some(Ok(data)) => Ok(data),
            Some(Err(e)) => Err(e),
            None => Err(match first {
                Err(e) => e,
                Ok(_) => t("decode_failed").to_string(),
            }),
        },
    }
}

fn decode_symphonia(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|_| t("decode_failed").to_string())?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| t("decode_failed").to_string())?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(SR);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|_| t("decode_failed").to_string())?;

    let mut out = Vec::new();
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::ResetRequired) => break,
            Err(SymError::IoError(_)) => break, // fim do arquivo
            Err(e) if out.is_empty() => return Err(e.to_string()),
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue, // quadro corrompido: pula
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(e.to_string()),
        };
        let spec = *decoded.spec();
        rate = spec.rate;
        let frames = decoded.capacity() as u64;
        if buf.as_ref().is_none_or(|b| (b.capacity() as u64) < frames * spec.channels.count() as u64) {
            buf = Some(SampleBuffer::new(frames, spec));
        }
        let b = buf.as_mut().unwrap();
        b.copy_interleaved_ref(decoded);
        push_stereo(&mut out, b.samples(), spec.channels.count());
    }
    Ok((out, rate))
}

/// Converte `channels` canais intercalados para estereo e acrescenta em `out`.
fn push_stereo(out: &mut Vec<f32>, samples: &[f32], channels: usize) {
    match channels {
        0 => {}
        1 => out.extend(samples.iter().flat_map(|&s| [s, s])),
        2 => out.extend_from_slice(samples),
        n => {
            // multicanal: frente esquerda/direita + centro (se houver) dividido entre os dois
            for f in samples.chunks_exact(n) {
                let c = f[2] * 0.707;
                out.push(f[0] + c);
                out.push(f[1] + c);
            }
        }
    }
}

fn ffmpeg_exe() -> Option<std::path::PathBuf> {
    let local = config::app_dir().join("ffmpeg.exe");
    if local.exists() {
        return Some(local);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join("ffmpeg.exe")).find(|p| p.exists())
}

/// None se nao houver ffmpeg disponivel.
fn decode_ffmpeg(path: &Path) -> Option<Result<Vec<f32>, String>> {
    let exe = ffmpeg_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(["-v", "error", "-i"]).arg(path).args(["-f", "f32le", "-ac", "2", "-ar", &SR.to_string(), "-"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = match cmd.output() {
        Ok(o) => o,
        Err(e) => return Some(Err(e.to_string())),
    };
    if !out.status.success() || out.stdout.is_empty() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Some(Err(if msg.is_empty() { t("decode_failed").to_string() } else { msg }));
    }
    let data = out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    Some(Ok(data))
}

/// Reamostragem offline com sinc janelado (polifasica), para os sons carregados.
pub fn resample(data: &[f32], channels: usize, from: u32, to: u32) -> Vec<f32> {
    if from == to || data.is_empty() || from == 0 || to == 0 {
        return data.to_vec();
    }
    const PHASES: usize = 512;
    let n_in = data.len() / channels;
    let ratio = to as f64 / from as f64;
    let n_out = (n_in as f64 * ratio).round() as usize;
    let scale = ratio.min(1.0); // ao reduzir a taxa, o corte desce junto (anti-aliasing)
    let cutoff = 0.5 * scale * 0.94;
    let half = (12.0 / scale).ceil() as isize;
    let taps = (2 * half) as usize;

    // tabela: para cada fase fracionaria, os pesos das amostras i0-half+1 .. i0+half
    let mut table = vec![0f32; (PHASES + 1) * taps];
    for p in 0..=PHASES {
        let frac = p as f64 / PHASES as f64;
        let row = &mut table[p * taps..(p + 1) * taps];
        let mut sum = 0.0;
        for (j, w) in row.iter_mut().enumerate() {
            let k = j as isize - half + 1;
            let x = k as f64 - frac;
            let sinc = if x.abs() < 1e-9 { 1.0 } else { (std::f64::consts::PI * 2.0 * cutoff * x).sin() / (std::f64::consts::PI * x) };
            let r = x / half as f64;
            let win = if r.abs() >= 1.0 {
                0.0
            } else {
                0.42 + 0.5 * (std::f64::consts::PI * r).cos() + 0.08 * (2.0 * std::f64::consts::PI * r).cos()
            };
            let v = sinc * win;
            *w = v as f32;
            sum += v;
        }
        if sum.abs() > 1e-12 {
            row.iter_mut().for_each(|w| *w = (*w as f64 / sum) as f32);
        }
    }

    let mut out = vec![0f32; n_out * channels];
    for j in 0..n_out {
        let pos = j as f64 / ratio;
        let i0 = pos.floor() as isize;
        let phase = ((pos - i0 as f64) * PHASES as f64).round() as usize;
        let row = &table[phase * taps..(phase + 1) * taps];
        let first = i0 - half + 1;
        for c in 0..channels {
            let mut acc = 0f32;
            for (k, w) in row.iter().enumerate() {
                let i = first + k as isize;
                if i >= 0 && (i as usize) < n_in {
                    acc += data[i as usize * channels + c] * w;
                }
            }
            out[j * channels + c] = acc;
        }
    }
    out
}

/// Grava float32 intercalado em WAV PCM 16 bits.
pub fn write_wav(path: &Path, data: &[f32], rate: u32, channels: u16) -> Result<(), String> {
    let spec = hound::WavSpec { channels, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in data {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: u32, secs: f32) -> Vec<f32> {
        (0..(rate as f32 * secs) as usize)
            .flat_map(|i| {
                let s = (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin() * 0.5;
                [s, s]
            })
            .collect()
    }

    #[test]
    fn resample_keeps_length_and_pitch() {
        let x = sine(1000.0, 44_100, 1.0);
        let y = resample(&x, 2, 44_100, SR);
        assert_eq!(y.len() / 2, 48_000);
        // conta cruzamentos por zero no canal esquerdo: ~2000 por segundo a 1 kHz
        let left: Vec<f32> = y.iter().step_by(2).copied().collect();
        let crossings = left.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        assert!((1990..=2010).contains(&crossings), "{crossings}");
    }

    #[test]
    fn wav_roundtrip() {
        let dir = std::env::temp_dir().join("soundmod-test-wav");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let x = sine(440.0, SR, 0.5);
        write_wav(&path, &x, SR, 2).unwrap();
        let y = decode(&path).unwrap();
        assert_eq!(y.len(), x.len());
        let err = x.iter().zip(&y).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(err < 1e-3, "{err}");
    }
}
