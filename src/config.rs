//! Configuracao persistente e pastas do programa.
//!
//! Os dados do usuario (config.json, sounds\, clips\) ficam em %APPDATA%\SoundMod, fora da pasta
//! do programa: atualizar o programa (substituir a pasta) nao apaga nada. Versoes antigas
//! guardavam tudo ao lado do .exe; isso e importado uma vez (copia, sem apagar o original).
//!
//! Mesmo formato e mesma pasta do Sound Mod em Python: os dois leem a mesma configuracao
//! (dispositivos por nome, `null` = automatico, `""` = nenhum). Campos desconhecidos sao
//! preservados ao salvar, para uma versao nao apagar o que a outra gravou.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::library;

/// Texto antigo que configs em Python guardavam no lugar de "" (nenhum dispositivo).
const LEGACY_NONE: [&str; 2] = ["(nenhum)", "(none)"];
/// Pastas que versoes antigas guardavam ao lado do programa (junto com o config.json).
const LEGACY_DIRS: [&str; 2] = ["sounds", "clips"];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub key: String,
    pub path: String,
    pub volume: f32,
    /// Nome da colecao do som, ou None (sem colecao).
    #[serde(deserialize_with = "de_name")]
    pub collection: Option<String>,
    /// Id do canal no mixer. So existe em memoria.
    #[serde(skip)]
    pub id: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Slot {
    fn default() -> Self {
        Self { key: String::new(), path: String::new(), volume: 1.0, collection: None, id: 0, extra: Map::new() }
    }
}

impl Slot {
    fn with_key(key: &str) -> Self {
        Self { key: key.into(), ..Self::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
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
    /// Colecoes na ordem em que aparecem (ver library.rs).
    #[serde(deserialize_with = "de_collections")]
    pub collections: Vec<Collection>,
    /// A ordem da lista e a ordem na tela (dentro de cada colecao).
    pub slots: Vec<Slot>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
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
            collections: Vec::new(),
            slots: ["f1", "f2", "f3", "f4"].iter().map(|k| Slot::with_key(k)).collect(),
            extra: Map::new(),
        }
    }
}

fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Nome de colecao tolerante: texto ou numero; o resto vira None.
fn de_name<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?.as_ref().and_then(text_of))
}

/// Aceita `["Nome"]` ou `[{"name": "Nome", "collapsed": true}]`; itens invalidos sao ignorados.
fn de_collections<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Collection>, D::Error> {
    let raw = Option::<Vec<Value>>::deserialize(d)?.unwrap_or_default();
    Ok(raw
        .iter()
        .filter_map(|v| match v {
            Value::Object(m) => Some(Collection {
                name: m.get("name").and_then(text_of).unwrap_or_default(),
                collapsed: m.get("collapsed").is_some_and(|c| c.as_bool().unwrap_or(false)),
            }),
            other => text_of(other).map(|name| Collection { name, collapsed: false }),
        })
        .collect())
}

// ---- pastas ----

/// Pasta do programa (onde fica o .exe) e pasta de dados do usuario.
#[derive(Clone, Debug)]
pub struct Dirs {
    pub app: PathBuf,
    pub data: PathBuf,
}

impl Dirs {
    pub fn config(&self) -> PathBuf {
        self.data.join("config.json")
    }
    pub fn clips(&self) -> PathBuf {
        self.data.join("clips")
    }
    /// Clipe do audio do PC (tecla de salvar).
    pub fn clip(&self) -> PathBuf {
        self.clips().join("last_clip.wav")
    }
    /// Clipes salvos como som.
    pub fn sounds(&self) -> PathBuf {
        self.data.join("sounds")
    }
    /// config.json de versoes antigas, ao lado do programa.
    pub fn legacy_config(&self) -> PathBuf {
        self.app.join("config.json")
    }
    /// Caminhos relativos (ex.: sons salvos em sounds/) sao relativos a pasta de dados.
    pub fn resolve(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() { p } else { self.data.join(p) }
    }
}

pub fn dirs() -> &'static Dirs {
    static DIRS: OnceLock<Dirs> = OnceLock::new();
    DIRS.get_or_init(|| Dirs { app: find_app_dir(), data: find_data_dir() })
}

/// Ao lado do .exe; rodando com `cargo run`, a raiz do projeto.
fn find_app_dir() -> PathBuf {
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
}

/// %APPDATA%\SoundMod. `SOUNDMOD_DATA_DIR` troca a pasta (testes, capturas de tela).
fn find_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SOUNDMOD_DATA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| ".".into());
        home.join("AppData").join("Roaming")
    });
    base.join("SoundMod")
}

pub fn app_dir() -> &'static Path {
    &dirs().app
}

pub fn data_dir() -> &'static Path {
    &dirs().data
}

pub fn clip_path() -> PathBuf {
    dirs().clip()
}

pub fn sounds_dir() -> PathBuf {
    dirs().sounds()
}

pub fn resolve(path: &str) -> PathBuf {
    dirs().resolve(path)
}

pub fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Data e hora locais: "20261121-013000".
pub fn timestamp() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        // SAFETY: GetLocalTime so preenche a estrutura passada.
        let st = unsafe {
            let mut st = std::mem::zeroed();
            GetLocalTime(&mut st);
            st
        };
        format!("{:04}{:02}{:02}-{:02}{:02}{:02}", st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond)
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        secs.to_string()
    }
}

// ---- carregar e salvar ----

/// Le a config (importando a da versao antiga se for a primeira vez).
/// Retorna a config e, se houve importacao, a pasta de onde ela veio.
pub fn load() -> (Config, Option<PathBuf>) {
    load_in(dirs())
}

pub fn load_in(d: &Dirs) -> (Config, Option<PathBuf>) {
    // sem permissao etc.: segue com o que houver
    let migrated = migrate_legacy(d).unwrap_or_else(|e| {
        eprintln!("config: importacao falhou: {e}");
        None
    });
    let mut cfg = match std::fs::read(d.config()) {
        Err(_) => Config::default(),
        Ok(bytes) => {
            let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes); // BOM do Bloco de Notas
            serde_json::from_slice(text).unwrap_or_else(|e| {
                // corrompido: guarda uma copia antes que o proximo save grave os padroes por cima
                eprintln!("config: {e}");
                let backup = d.data.join(format!("config-corrompido-{}.json", timestamp()));
                let _ = std::fs::copy(d.config(), backup);
                Config::default()
            })
        }
    };
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
    library::normalize(&mut cfg);
    (cfg, migrated)
}

/// Copia config.json, sounds\ e clips\ da pasta do programa (versoes antigas) para a pasta de
/// dados. So quando a pasta de dados ainda nao tem config; nao apaga nem sobrescreve nada.
fn migrate_legacy(d: &Dirs) -> io::Result<Option<PathBuf>> {
    if d.config().exists() || !d.legacy_config().is_file() {
        return Ok(None);
    }
    std::fs::create_dir_all(&d.data)?;
    for name in LEGACY_DIRS {
        let src = d.app.join(name);
        if src.is_dir() {
            copy_missing(&src, &d.data.join(name))?;
        }
    }
    // config por ultimo: se a copia acima falhar, a proxima execucao tenta de novo
    std::fs::copy(d.legacy_config(), d.config())?;
    Ok(Some(d.app.clone()))
}

fn copy_missing(src: &Path, dst: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_missing(&entry.path(), &target)?;
        } else if !target.exists() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

pub fn save(cfg: &Config) -> io::Result<()> {
    save_in(dirs(), cfg)
}

pub fn save_in(d: &Dirs, cfg: &Config) -> io::Result<()> {
    std::fs::create_dir_all(d.clips())?;
    let text = serde_json::to_string_pretty(cfg).map_err(io::Error::other)?;
    // grava num temporario e troca: uma falha no meio nao deixa o config.json pela metade
    let tmp = d.data.join("config.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, d.config())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_CONFIG: &str = r#"{
        "stop_key": "f11",
        "slots": [
            {"key": "f3", "path": "C:/Musicas/a.mp3", "volume": 1.5},
            {"key": "", "path": "sounds/salvo.mp3", "volume": 1.0}
        ]
    }"#;

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn setup(name: &str) -> (Tmp, Dirs) {
        let root = std::env::temp_dir().join(format!("soundmod-cfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let d = Dirs { app: root.join("app"), data: root.join("appdata").join("SoundMod") };
        std::fs::create_dir_all(&d.app).unwrap();
        (Tmp(root), d)
    }

    fn write_old_version(d: &Dirs) {
        std::fs::write(d.app.join("config.json"), OLD_CONFIG).unwrap();
        std::fs::create_dir_all(d.app.join("sounds")).unwrap();
        std::fs::write(d.app.join("sounds").join("salvo.mp3"), b"som").unwrap();
        std::fs::create_dir_all(d.app.join("clips")).unwrap();
        std::fs::write(d.app.join("clips").join("last_clip.wav"), b"clipe").unwrap();
    }

    fn paths(cfg: &Config) -> Vec<(&str, Option<&str>)> {
        cfg.slots.iter().map(|s| (s.path.as_str(), s.collection.as_deref())).collect()
    }

    #[test]
    fn fresh_install_uses_defaults() {
        let (_t, d) = setup("fresh");
        let (cfg, migrated) = load_in(&d);
        assert_eq!(cfg.slots.len(), 4);
        assert!(cfg.collections.is_empty());
        assert!(migrated.is_none());
    }

    #[test]
    fn migrates_old_config_sounds_and_clip() {
        let (_t, d) = setup("migrate");
        write_old_version(&d);
        let (cfg, migrated) = load_in(&d);
        assert_eq!(migrated.as_deref(), Some(d.app.as_path()));
        assert_eq!(cfg.stop_key, "f11");
        assert_eq!(paths(&cfg), [("C:/Musicas/a.mp3", None), ("sounds/salvo.mp3", None)]);
        assert_eq!(std::fs::read(d.sounds().join("salvo.mp3")).unwrap(), b"som");
        assert_eq!(std::fs::read(d.clip()).unwrap(), b"clipe");
        assert_eq!(d.resolve("sounds/salvo.mp3"), d.data.join("sounds/salvo.mp3"));
        // nada e apagado da pasta antiga
        assert!(d.legacy_config().exists());
        assert!(d.app.join("sounds").join("salvo.mp3").exists());
    }

    #[test]
    fn app_folder_replaced_keeps_everything() {
        let (_t, d) = setup("replaced");
        write_old_version(&d);
        let (mut cfg, _) = load_in(&d);
        cfg.collections = vec![Collection { name: "Memes".into(), collapsed: false }];
        cfg.slots.reverse();
        cfg.slots[0].collection = Some("Memes".into());
        save_in(&d, &cfg).unwrap();
        // atualizacao: a pasta do programa e substituida por uma copia nova, sem config
        std::fs::remove_dir_all(&d.app).unwrap();
        std::fs::create_dir_all(&d.app).unwrap();
        let (again, migrated) = load_in(&d);
        assert!(migrated.is_none());
        assert_eq!(again.collections, [Collection { name: "Memes".into(), collapsed: false }]);
        assert_eq!(paths(&again), [("sounds/salvo.mp3", Some("Memes")), ("C:/Musicas/a.mp3", None)]);
    }

    #[test]
    fn old_config_copied_over_again_does_not_win() {
        let (_t, d) = setup("again");
        write_old_version(&d);
        let (mut cfg, _) = load_in(&d);
        cfg.stop_key = "f9".into();
        save_in(&d, &cfg).unwrap();
        let (again, migrated) = load_in(&d);
        assert_eq!(again.stop_key, "f9");
        assert!(migrated.is_none());
    }

    #[test]
    fn migration_does_not_overwrite_existing_sounds() {
        let (_t, d) = setup("nooverwrite");
        write_old_version(&d);
        std::fs::create_dir_all(d.sounds()).unwrap();
        std::fs::write(d.sounds().join("salvo.mp3"), b"novo").unwrap();
        load_in(&d);
        assert_eq!(std::fs::read(d.sounds().join("salvo.mp3")).unwrap(), b"novo");
    }

    #[test]
    fn corrupted_config_is_kept_aside() {
        let (_t, d) = setup("corrupted");
        std::fs::create_dir_all(&d.data).unwrap();
        std::fs::write(d.config(), "{quebrado").unwrap();
        let (cfg, _) = load_in(&d);
        assert_eq!(cfg.slots.len(), 4);
        let backups: Vec<_> = std::fs::read_dir(&d.data)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("config-corrompido-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read_to_string(backups[0].path()).unwrap(), "{quebrado");
    }

    #[test]
    fn save_round_trip_keeps_unknown_fields() {
        let (_t, d) = setup("roundtrip");
        std::fs::create_dir_all(&d.data).unwrap();
        std::fs::write(
            d.config(),
            r#"{"futuro": 7, "collections": ["A"], "slots": [{"path": "C:/ação.mp3", "collection": "A", "tag": "x"}]}"#,
        )
        .unwrap();
        let (cfg, _) = load_in(&d);
        save_in(&d, &cfg).unwrap();
        assert!(!d.data.join("config.tmp").exists());
        let (again, _) = load_in(&d);
        assert_eq!(again.slots[0].path, "C:/ação.mp3");
        assert_eq!(again.extra.get("futuro"), Some(&Value::from(7)));
        assert_eq!(again.slots[0].extra.get("tag"), Some(&Value::from("x")));
        assert_eq!(again.collections, [Collection { name: "A".into(), collapsed: false }]);
    }
}
