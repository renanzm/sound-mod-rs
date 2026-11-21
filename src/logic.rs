//! Logica do app, compartilhada entre a interface e a thread das hotkeys.
//!
//! As hotkeys chamam `Core::on_action` direto da thread delas, entao tudo funciona com a janela
//! minimizada. Trabalho demorado (decodificar, gravar arquivos) vai para threads proprias.
//! Nada aqui toca na interface: mensagens vao para `status` e a janela le a cada quadro.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use crate::config::{self, Config, Slot};
use crate::engine::{CLIP, ChannelId, Engine};
use crate::hotkeys::{Action, Hotkeys};
use crate::i18n::{t, tf};
use crate::recorder::Recorder;

/// Qual tecla configuravel esta sendo trocada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyTarget {
    Slot(u64),
    SaveClip,
    PlayClip,
    StopAll,
}

pub struct Core {
    pub engine: Arc<Engine>,
    pub recorder: Arc<Recorder>,
    pub cfg: Mutex<Config>,
    pub hotkeys: OnceLock<Arc<Hotkeys>>,
    status: Mutex<String>,
    /// Muda a cada clipe carregado, para a interface redesenhar a forma de onda.
    clip_version: AtomicU64,
    next_id: AtomicU64,
    repaint: OnceLock<eframe::egui::Context>,
}

impl Core {
    pub fn new(mut cfg: Config) -> Arc<Self> {
        let mut id = 0;
        for slot in &mut cfg.slots {
            id += 1;
            slot.id = id;
        }
        let engine = Arc::new(Engine::default());
        engine.set_voice(&cfg.voice_effect);
        let recorder = Arc::new(Recorder::new(cfg.clip_seconds));
        let core = Arc::new(Self {
            engine,
            recorder,
            cfg: Mutex::new(cfg),
            hotkeys: OnceLock::new(),
            status: Mutex::new(t("ready").to_string()),
            clip_version: AtomicU64::new(0),
            next_id: AtomicU64::new(id + 1),
            repaint: OnceLock::new(),
        });
        core.apply_live();
        core
    }

    pub fn set_repaint(&self, ctx: eframe::egui::Context) {
        let _ = self.repaint.set(ctx);
    }

    fn repaint(&self) {
        if let Some(ctx) = self.repaint.get() {
            ctx.request_repaint();
        }
    }

    // ---- status ----
    pub fn say(&self, msg: impl Into<String>) {
        // uma linha so: mensagens de erro com varias linhas esticariam a barra
        let msg: String = msg.into().split_whitespace().collect::<Vec<_>>().join(" ");
        *self.status.lock() = msg;
        self.repaint();
    }

    pub fn status(&self) -> String {
        self.status.lock().clone()
    }

    pub fn clip_version(&self) -> u64 {
        self.clip_version.load(Ordering::Relaxed)
    }

    // ---- config ----
    pub fn save_cfg(&self) {
        let cfg = self.cfg.lock().clone();
        if let Err(e) = config::save(&cfg) {
            eprintln!("config: {e}");
        }
    }

    /// Volume, mistura do mic e monitor: valem na hora, sem reiniciar o audio.
    pub fn apply_live(&self) {
        let cfg = self.cfg.lock();
        self.engine.mix_mic.store(cfg.mix_mic, Ordering::Relaxed);
        self.engine.monitor.store(cfg.monitor, Ordering::Relaxed);
        self.engine.hear_self.store(cfg.hear_self, Ordering::Relaxed);
        self.engine.set_master(cfg.master_volume);
        self.recorder.seconds.store(cfg.clip_seconds, Ordering::Relaxed);
    }

    pub fn slot(&self, id: u64) -> Option<Slot> {
        self.cfg.lock().slots.iter().find(|s| s.id == id).cloned()
    }

    pub fn slots(&self) -> Vec<Slot> {
        self.cfg.lock().slots.clone()
    }

    // ---- hotkeys ----
    pub fn register_hotkeys(&self) {
        let Some(hk) = self.hotkeys.get() else { return };
        let list = {
            let cfg = self.cfg.lock();
            let mut list: Vec<(String, Action)> = cfg.slots.iter().map(|s| (s.key.clone(), Action::Slot(s.id))).collect();
            list.push((cfg.save_clip_key.clone(), Action::SaveClip));
            list.push((cfg.play_clip_key.clone(), Action::PlayClip));
            list.push((cfg.stop_key.clone(), Action::StopAll));
            list
        };
        let errors = hk.set_bindings(&list);
        if !errors.is_empty() {
            self.say(tf("invalid_key", &[("keys", &errors.join("; "))]));
        }
    }

    pub fn on_action(self: &Arc<Self>, action: Action) {
        match action {
            Action::Slot(id) => self.play_slot(id, true),
            Action::SaveClip => self.save_clip(),
            Action::PlayClip => self.toggle_clip(),
            Action::StopAll => self.engine.stop_all(),
        }
        self.repaint();
    }

    pub fn key_of(&self, target: KeyTarget) -> String {
        let cfg = self.cfg.lock();
        match target {
            KeyTarget::Slot(id) => cfg.slots.iter().find(|s| s.id == id).map(|s| s.key.clone()).unwrap_or_default(),
            KeyTarget::SaveClip => cfg.save_clip_key.clone(),
            KeyTarget::PlayClip => cfg.play_clip_key.clone(),
            KeyTarget::StopAll => cfg.stop_key.clone(),
        }
    }

    pub fn set_key(&self, target: KeyTarget, key: String) {
        {
            let mut cfg = self.cfg.lock();
            let field = match target {
                KeyTarget::Slot(id) => match cfg.slots.iter_mut().find(|s| s.id == id) {
                    Some(s) => &mut s.key,
                    None => return,
                },
                KeyTarget::SaveClip => &mut cfg.save_clip_key,
                KeyTarget::PlayClip => &mut cfg.play_clip_key,
                KeyTarget::StopAll => &mut cfg.stop_key,
            };
            *field = key;
        }
        self.register_hotkeys();
        self.save_cfg();
    }

    // ---- slots ----
    /// Decodifica e associa o som ao canal do slot (bloqueia: chamar fora da interface).
    pub fn load_slot(&self, id: u64, quiet: bool) -> bool {
        let Some(slot) = self.slot(id) else { return false };
        let name = config::file_name(&slot.path);
        let path = config::resolve(&slot.path);
        if !path.exists() {
            self.say(tf("file_missing", &[("name", &name)]));
            return false;
        }
        match self.engine.set_sound(id, &path, slot.volume) {
            Ok(_) => {
                if !quiet {
                    self.say(tf("loaded", &[("name", &name)]));
                }
                self.repaint();
                true
            }
            Err(e) => {
                self.say(tf("load_error", &[("name", &name), ("e", &e)]));
                false
            }
        }
    }

    pub fn spawn_load(self: &Arc<Self>, id: u64, quiet: bool) {
        let core = self.clone();
        std::thread::spawn(move || core.load_slot(id, quiet));
    }

    pub fn load_missing(self: &Arc<Self>) {
        for slot in self.slots() {
            if !slot.path.is_empty() && !self.engine.has(slot.id) {
                self.spawn_load(slot.id, true);
            }
        }
    }

    /// ▶: toca (recomeca se ja tocava). Hotkey/botao da tecla (toggle): toca ou, se ja tocava, para.
    pub fn play_slot(self: &Arc<Self>, id: u64, toggle: bool) {
        let Some(slot) = self.slot(id) else { return };
        if slot.path.is_empty() {
            return self.say(t("slot_empty"));
        }
        if !self.engine.running() {
            return self.say(t("engine_stopped"));
        }
        let name = config::file_name(&slot.path);
        let playing = if toggle { self.engine.toggle(id) } else { Some(self.engine.play(id)).filter(|p| *p) };
        match playing {
            Some(p) => self.say(tf(if p { "playing" } else { "stopped" }, &[("name", &name)])),
            None => {
                // ainda nao carregado: carrega e toca
                let core = self.clone();
                std::thread::spawn(move || {
                    if core.load_slot(id, true) && core.engine.play(id) {
                        core.say(tf("playing", &[("name", &name)]));
                    }
                });
            }
        }
    }

    /// Usa o primeiro slot sem arquivo (mantem a tecla dele) ou cria um slot novo.
    pub fn add_sound(self: &Arc<Self>, path: String) -> u64 {
        let id = {
            let mut cfg = self.cfg.lock();
            match cfg.slots.iter_mut().find(|s| s.path.is_empty()) {
                Some(s) => {
                    s.path = path.clone();
                    s.id
                }
                None => {
                    let id = self.next_id.fetch_add(1, Ordering::Relaxed);
                    cfg.slots.push(Slot { key: String::new(), path: path.clone(), volume: 1.0, id });
                    id
                }
            }
        };
        self.engine.invalidate(&config::resolve(&path));
        self.spawn_load(id, true);
        id
    }

    pub fn add_sounds(self: &Arc<Self>, paths: Vec<String>) {
        if paths.is_empty() {
            return;
        }
        let n = paths.len();
        for p in paths {
            self.add_sound(p);
        }
        self.register_hotkeys();
        self.save_cfg();
        self.say(tf("sounds_added", &[("n", &n)]));
    }

    pub fn set_slot_file(self: &Arc<Self>, id: u64, path: String) {
        if let Some(s) = self.cfg.lock().slots.iter_mut().find(|s| s.id == id) {
            s.path = path.clone();
        } else {
            return;
        }
        self.engine.invalidate(&config::resolve(&path));
        self.save_cfg();
        self.spawn_load(id, false);
    }

    pub fn set_slot_volume(&self, id: u64, volume: f32) {
        if let Some(s) = self.cfg.lock().slots.iter_mut().find(|s| s.id == id) {
            s.volume = volume;
        }
        self.engine.set_gain(id, volume);
    }

    pub fn remove_slot(&self, id: u64) {
        self.cfg.lock().slots.retain(|s| s.id != id);
        self.engine.remove(id);
        self.register_hotkeys();
        self.save_cfg();
    }

    // ---- clipe ----
    pub fn save_clip(self: &Arc<Self>) {
        let core = self.clone();
        std::thread::spawn(move || match core.recorder.save(&config::clip_path()) {
            Ok(dur) => {
                if core.load_clip(true) {
                    let name = config::file_name(&config::clip_path().to_string_lossy());
                    core.say(tf("clip_saved", &[("dur", &format!("{dur:.1}")), ("name", &name)]));
                }
            }
            Err(e) => core.say(tf("clip_save_error", &[("e", &e)])),
        });
    }

    /// Decodifica o clipe e aplica o trecho salvo (bloqueia: chamar fora da interface).
    pub fn load_clip(&self, reset_trim: bool) -> bool {
        let path = config::clip_path();
        self.engine.invalidate(&path);
        if let Err(e) = self.engine.set_sound(CLIP, &path, 1.0) {
            self.say(tf("clip_open_error", &[("e", &e)]));
            return false;
        }
        let trim = {
            let mut cfg = self.cfg.lock();
            if reset_trim {
                cfg.clip_trim = None;
            }
            cfg.clip_trim
        };
        if let Some([start, end]) = trim {
            self.engine.set_range(CLIP, start, end);
        }
        if reset_trim {
            self.save_cfg();
        }
        self.clip_version.fetch_add(1, Ordering::Relaxed);
        self.repaint();
        true
    }

    pub fn set_clip_range(&self, start: f64, end: f64) {
        self.cfg.lock().clip_trim = Some([(start * 1000.0).round() / 1000.0, (end * 1000.0).round() / 1000.0]);
        self.engine.set_range(CLIP, start, end);
        self.save_cfg();
    }

    pub fn toggle_clip(&self) {
        if !self.engine.has(CLIP) {
            return self.say(t("no_clip_saved"));
        }
        if !self.engine.running() {
            return self.say(t("engine_stopped"));
        }
        let playing = self.engine.toggle(CLIP) == Some(true);
        self.say(t(if playing { "clip_playing" } else { "clip_stopped" }));
    }

    /// Grava o trecho escolhido do clipe em sounds/<nome>.wav e adiciona como um som.
    pub fn save_as_sound(self: &Arc<Self>, name: &str) {
        let Some((data, start, end)) = self.engine.channel_audio(CLIP) else {
            return self.say(t("no_clip_saved"));
        };
        let mut clean: String = name.chars().filter(|c| !r#"<>:"/\|?*"#.contains(*c) && !c.is_control()).collect();
        clean = clean.trim_matches(|c: char| c == ' ' || c == '.').to_string();
        if clean.is_empty() {
            clean = default_sound_name();
        }
        let core = self.clone();
        std::thread::spawn(move || {
            let dir = config::sounds_dir();
            let result = std::fs::create_dir_all(&dir).map_err(|e| e.to_string()).and_then(|_| {
                let mut target = dir.join(format!("{clean}.wav"));
                let mut n = 2;
                while target.exists() {
                    target = dir.join(format!("{clean} ({n}).wav"));
                    n += 1;
                }
                let sr = crate::codec::SR as f64;
                let (a, b) = ((start * sr) as usize * 2, ((end * sr) as usize * 2).min(data.len()));
                crate::codec::write_wav(&target, &data[a.min(b)..b], crate::codec::SR, 2)?;
                Ok(target)
            });
            match result {
                Ok(target) => {
                    // relativo a pasta do programa: continua valendo se a pasta for copiada
                    let rel = target
                        .strip_prefix(config::root())
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_else(|_| target.to_string_lossy().into_owned());
                    core.add_sound(rel.clone());
                    core.register_hotkeys();
                    core.save_cfg();
                    core.say(tf("sound_saved", &[("name", &config::file_name(&rel))]));
                }
                Err(e) => core.say(tf("sound_save_error", &[("e", &e)])),
            }
        });
    }

    // ---- voz ----
    pub fn set_voice(&self, name: &str) {
        self.cfg.lock().voice_effect = name.to_string();
        self.engine.set_voice(name);
        self.save_cfg();
        let key = crate::voice::EFFECTS.iter().position(|e| *e == name).map(|i| FX_KEYS[i]).unwrap_or("fx_normal");
        self.say(tf("voice_on", &[("name", &t(key))]));
    }

    pub fn channel_state(&self, cid: ChannelId) -> Option<(f64, f64, bool)> {
        self.engine.state(cid)
    }
}

/// Chave de traducao de cada efeito, na ordem de `voice::EFFECTS`.
pub const FX_KEYS: [&str; 9] =
    ["fx_normal", "fx_deep", "fx_high", "fx_robot", "fx_ogre", "fx_autotune", "fx_echo", "fx_radio", "fx_alien"];

pub fn default_sound_name() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        // SAFETY: GetLocalTime so preenche a estrutura passada.
        let st = unsafe {
            let mut st = std::mem::zeroed();
            GetLocalTime(&mut st);
            st
        };
        format!("clip-{:04}{:02}{:02}-{:02}{:02}{:02}", st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond)
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        format!("clip-{secs}")
    }
}
