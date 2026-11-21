//! Configuracao persistente (config.json ao lado do executavel) e caminhos do programa.
//!
//! O formato e compativel com o config.json do Sound Mod em Python: os mesmos campos e o mesmo
//! significado (dispositivos por nome, `null` = automatico, `""` = nenhum).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Texto antigo que configs em Python guardavam no lugar de "" (nenhum dispositivo).
const LEGACY_NONE: [&str; 2] = ["(nenhum)", "(none)"];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub key: String,
    pub path: String,
    pub volume: f32,
    /// Id do canal no mixer. So existe em memoria.
    #[serde(skip)]
    pub id: u64,
}

impl Default for Slot {
    fn default() -> Self {
        Self { key: String::new(), path: String::new(), volume: 1.0, id: 0 }
    }
}

impl Slot {
    fn with_key(key: &str) -> Self {
        Self { key: key.into(), ..Self::default() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// "pt", "en" ou None = idioma do Windows.
    pub language: Option<String>,
    // dispositivos: None = escolher automaticamente, Some("") = nenhum
    pub mic_device: Option<String>,
    pub output_device: Option<String>,
    pub monitor_device: Option<String>,
    /// None = alto-falante padrao.
    pub loopback_device: Option<String>,
    pub mix_mic: bool,
    pub monitor: bool,
    pub master_volume: f32,
    pub clip_seconds: u32,
    pub voice_effect: String,
    pub hear_self: bool,
    /// [inicio, fim] em segundos do trecho do clipe que toca, ou None = inteiro.
    pub clip_trim: Option<[f64; 2]>,
    pub save_clip_key: String,
    pub play_clip_key: String,
    pub stop_key: String,
    pub slots: Vec<Slot>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: None,
            mic_device: None,
            output_device: None,
            monitor_device: None,
            loopback_device: None,
            mix_mic: true,
            monitor: true,
            master_volume: 1.0,
            clip_seconds: 5,
            voice_effect: "normal".into(),
            hear_self: false,
            clip_trim: None,
            save_clip_key: "f5".into(),
            play_clip_key: "f6".into(),
            stop_key: "f12".into(),
            slots: ["f1", "f2", "f3", "f4"].iter().map(|k| Slot::with_key(k)).collect(),
        }
    }
}

/// Pasta do programa: ao lado do .exe. Rodando com `cargo run`, a raiz do projeto.
pub fn root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        let dir = exe.unwrap_or_else(|| PathBuf::from("."));
        let in_target = dir.parent().and_then(|p| p.file_name()).is_some_and(|n| n == "target")
            && dir.file_name().is_some_and(|n| n == "debug" || n == "release");
        if in_target {
            if let Some(project) = dir.parent().and_then(Path::parent) {
                if project.join("Cargo.toml").exists() {
                    return project.to_path_buf();
                }
            }
        }
        dir
    })
}

pub fn config_path() -> PathBuf {
    root().join("config.json")
}

pub fn clips_dir() -> PathBuf {
    root().join("clips")
}

/// Clipe do audio do PC (tecla de salvar).
pub fn clip_path() -> PathBuf {
    clips_dir().join("last_clip.wav")
}

/// Clipes salvos como som.
pub fn sounds_dir() -> PathBuf {
    root().join("sounds")
}

/// Caminhos relativos (ex.: sons salvos em sounds/) sao relativos a pasta do programa.
pub fn resolve(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() { p } else { root().join(p) }
}

pub fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn load() -> Config {
    let mut cfg: Config = std::fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    for dev in [&mut cfg.mic_device, &mut cfg.output_device, &mut cfg.monitor_device] {
        if dev.as_deref().is_some_and(|d| LEGACY_NONE.contains(&d)) {
            *dev = Some(String::new());
        }
    }
    if cfg.loopback_device.as_deref().is_some_and(|d| d.is_empty() || d.starts_with('(')) {
        cfg.loopback_device = None;
    }
    cfg.clip_seconds = cfg.clip_seconds.clamp(1, 60);
    cfg.master_volume = cfg.master_volume.clamp(0.0, 1.0);
    cfg
}

pub fn save(cfg: &Config) -> std::io::Result<()> {
    std::fs::create_dir_all(clips_dir())?;
    let text = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    // grava num temporario e renomeia: um travamento no meio nao corrompe a config
    let tmp = root().join("config.json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, config_path())
}
