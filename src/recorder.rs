//! Captura continua do audio do PC (WASAPI loopback) com buffer dos ultimos N segundos.
//!
//! No WASAPI, abrir um stream de *entrada* num dispositivo de *saida* liga o modo loopback.
//! Sem audio tocando no PC nao chegam amostras, entao o buffer pode estar vazio.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, StreamConfig};
use parking_lot::Mutex;

use crate::codec;
use crate::engine::find_device;
use crate::i18n::t;

struct Buffer {
    data: VecDeque<f32>,
    rate: u32,
    channels: u16,
}

pub struct Recorder {
    buf: Mutex<Buffer>,
    pub seconds: AtomicU32,
}

impl Recorder {
    pub fn new(seconds: u32) -> Self {
        Self {
            buf: Mutex::new(Buffer { data: VecDeque::new(), rate: 48_000, channels: 2 }),
            seconds: AtomicU32::new(seconds),
        }
    }

    fn limit(&self, b: &Buffer) -> usize {
        self.seconds.load(Ordering::Relaxed) as usize * b.rate as usize * b.channels as usize
    }

    /// Comeca a gravar o dispositivo pelo nome (None = alto-falante padrao).
    pub fn start(self: &Arc<Self>, device: Option<&str>) -> Result<cpal::Stream, String> {
        // (pode demorar como o Engine::open: chamar fora da thread da janela)
        let host = cpal::default_host();
        let dev = match device {
            Some(name) => find_device(&host, name, false)?,
            None => host.default_output_device().ok_or_else(|| t("select_output").to_string())?,
        };
        crate::engine::trace("loopback: lendo formato");
        let supported = dev.default_output_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        {
            let mut b = self.buf.lock();
            b.data.clear();
            b.rate = config.sample_rate;
            b.channels = config.channels;
        }
        let stream = match supported.sample_format() {
            SampleFormat::F32 => self.stream::<f32>(&dev, config),
            SampleFormat::I16 => self.stream::<i16>(&dev, config),
            SampleFormat::I32 => self.stream::<i32>(&dev, config),
            SampleFormat::U8 => self.stream::<u8>(&dev, config),
            other => Err(format!("{other:?}")),
        }?;
        crate::engine::trace("loopback: iniciando");
        stream.play().map_err(|e| e.to_string())?;
        crate::engine::trace("loopback: ok");
        Ok(stream)
    }

    fn stream<T>(self: &Arc<Self>, dev: &cpal::Device, config: StreamConfig) -> Result<cpal::Stream, String>
    where
        T: SizedSample + Send + 'static,
        f32: FromSample<T>,
    {
        let rec = self.clone();
        dev.build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                let mut b = rec.buf.lock();
                b.data.extend(data.iter().map(|&s| f32::from_sample(s)));
                let extra = b.data.len().saturating_sub(rec.limit(&b));
                b.data.drain(..extra);
            },
            |err| eprintln!("loopback: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
    }

    /// Salva os ultimos N segundos em WAV. Retorna a duracao em segundos.
    pub fn save(&self, path: &Path) -> Result<f64, String> {
        let (pcm, rate, channels) = {
            let b = self.buf.lock();
            if b.data.is_empty() {
                return Err(t("nothing_recorded").to_string());
            }
            let skip = b.data.len().saturating_sub(self.limit(&b));
            (b.data.iter().skip(skip).copied().collect::<Vec<f32>>(), b.rate, b.channels)
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        codec::write_wav(path, &pcm, rate, channels)?;
        Ok(pcm.len() as f64 / (rate as f64 * channels as f64))
    }
}
