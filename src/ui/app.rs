//! Janela principal: cabecalho, lista de sons, clipe do PC, dispositivos e efeitos de voz.
//!
//! A interface e "modo imediato": tudo e redesenhado a cada quadro lendo o estado do `Core` e do
//! engine. As acoes de cada linha sao coletadas durante o desenho e aplicadas depois.

use std::sync::Arc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, CentralPanel, Color32, ComboBox, Frame, Id, Layout, Margin, Panel, RichText, ScrollArea,
    Sense, Shadow, Stroke, TextEdit, Ui, pos2, vec2,
};

use super::icons::{self, Icon};
use super::theme::{self, *};
use super::waveform::{WaveEvent, Waveform};
use super::widgets::{self, Style, Tint};
use crate::codec;
use crate::config::{self, Collection, Slot};
use crate::engine::{self, CLIP};
use crate::hotkeys::{self, Hotkeys};
use crate::i18n::{self, t, tf};
use crate::library;
use crate::logic::{Core, FX_KEYS, KeyTarget, default_sound_name};
use crate::voice;

const SIDEBAR: f32 = 356.0;

enum RowAction {
    Toggle(u64),
    Play(u64),
    Pause(u64),
    Stop(u64),
    Pick(u64),
    Remove(u64),
    Seek(u64, f64),
    Volume(u64, f32, bool),
    Edit(KeyTarget),
    Clear(KeyTarget),
    MoveSlot(u64, isize),
    SetCollection(u64, Option<String>),
    NewCollection(Option<u64>),
    AddInto(String),
    ToggleCollection(String),
    MoveCollection(String, isize),
    RenameCollection(String),
    DeleteCollection(String),
}

/// Para que serve a janelinha de nome aberta.
enum DialogKind {
    SaveSound,
    /// `assign`: som que vai para a colecao nova (menu de colecoes de um som).
    NewCollection { assign: Option<u64> },
    RenameCollection(String),
}

struct NameDialog {
    kind: DialogKind,
    text: String,
    error: Option<String>,
    focus: bool,
}

impl NameDialog {
    fn new(kind: DialogKind, text: String) -> Self {
        Self { kind, text, error: None, focus: true }
    }
}

pub struct SoundApp {
    core: Arc<Core>,
    streams: Vec<cpal::Stream>,
    loopback: Option<cpal::Stream>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    cable_found: bool,
    capture: Option<(KeyTarget, mpsc::Receiver<Option<String>>)>,
    wave: Waveform,
    levels: [f32; 2],
    dialog: Option<NameDialog>,
    secs_text: String,
    /// Aviso unico na barra de status (config da versao anterior importada).
    notice: Option<String>,
}

impl SoundApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (mut cfg, migrated) = config::load();
        i18n::set_language(cfg.language.as_deref());
        theme::set_accent(&cfg.accent);
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx);
        super::window::style(cc);

        let notice = migrated.map(|path| tf("config_imported", &[("path", &path.display())]));
        let (inputs, outputs) = engine::list_devices();
        let (def_in, def_out) = engine::default_device_names();
        let cable = outputs.iter().find(|n| n.contains("CABLE Input")).cloned();
        // None = automatico: escolhe uma vez e passa a guardar o nome
        if cfg.mic_device.is_none() {
            let mic = inputs
                .iter()
                .find(|n| n.to_lowercase().contains("micro") && !n.contains("CABLE"))
                .cloned()
                .or(def_in.filter(|n| !n.contains("CABLE")));
            cfg.mic_device = Some(mic.unwrap_or_default());
        }
        if cfg.output_device.is_none() {
            cfg.output_device = Some(cable.clone().unwrap_or_default());
        }
        if cfg.monitor_device.is_none() {
            let phones = def_out
                .filter(|n| !n.contains("CABLE"))
                .or_else(|| outputs.iter().find(|n| !n.contains("CABLE")).cloned());
            cfg.monitor_device = Some(phones.unwrap_or_default());
        }
        let secs_text = cfg.clip_seconds.to_string();

        let core = Core::new(cfg);
        core.set_repaint(cc.egui_ctx.clone());
        let handler = core.clone();
        let _ = core.hotkeys.set(Hotkeys::start(move |action| handler.on_action(action)));
        if config::clip_path().exists() {
            let c = core.clone();
            std::thread::spawn(move || c.load_clip(false));
        }

        let mut app = Self {
            core,
            streams: Vec::new(),
            loopback: None,
            inputs,
            outputs,
            cable_found: cable.is_some(),
            capture: None,
            wave: Waveform::default(),
            levels: [0.0; 2],
            dialog: None,
            secs_text,
            notice,
        };
        app.start_all();
        // sons de configs antigas que estao fora da pasta de sons: copia uma vez (depois do
        // start_all, para o aviso nao ser coberto pela mensagem de inicio)
        app.core.adopt_all();
        app
    }

    /// Botao "Aplicar / Reiniciar" e inicio: reabre o audio e a gravacao com a config atual.
    fn start_all(&mut self) {
        self.core.apply_live();
        let (mic, out, mut mon, loopback) = {
            let cfg = self.core.cfg.lock();
            let named = |d: &Option<String>| d.clone().filter(|s| !s.is_empty());
            (named(&cfg.mic_device), named(&cfg.output_device), named(&cfg.monitor_device), cfg.loopback_device.clone())
        };
        let mut msgs: Vec<String> = Vec::new();
        if out.as_deref().is_some_and(|o| !o.contains("CABLE")) {
            msgs.push(t("warn_output").into());
        }
        if mon.as_deref().is_some_and(|m| Some(m) == out.as_deref() || m.contains("CABLE")) {
            // monitor no cabo duplica os sons no microfone (eco) e voce nao ouve nada
            msgs.push(t("monitor_off").into());
            mon = None;
        }
        self.core.engine.close(&mut self.streams);
        match self.core.engine.open(mic.as_deref(), out.as_deref(), mon.as_deref()) {
            Ok(streams) => {
                self.streams = streams;
                msgs.push(t("audio_on").into());
            }
            Err(e) => msgs.push(tf("audio_error", &[("e", &e)])),
        }
        self.loopback = None;
        match self.core.recorder.start(loopback.as_deref()) {
            Ok(s) => {
                self.loopback = Some(s);
                msgs.push(t("capture_on").into());
            }
            Err(e) => msgs.push(tf("capture_error", &[("e", &e)])),
        }
        if let Some(notice) = self.notice.take() {
            msgs.insert(0, notice);
        }
        self.core.register_hotkeys();
        self.core.save_cfg();
        self.core.load_missing();
        self.core.say(msgs.join("  |  "));
    }

    fn poll_capture(&mut self) {
        let Some((target, rx)) = &self.capture else { return };
        let target = *target;
        match rx.try_recv() {
            Ok(Some(key)) => {
                self.capture = None;
                self.core.set_key(target, key.clone());
                self.core.say(tf("key_set", &[("key", &hotkeys::display(&key))]));
            }
            Ok(None) => {
                self.capture = None;
                self.core.say(t("capture_cancelled"));
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.capture = None,
        }
    }

    fn begin_capture(&mut self, target: KeyTarget) {
        if self.capture.is_some() {
            return;
        }
        if let Some(hk) = self.core.hotkeys.get() {
            self.capture = Some((target, hk.begin_capture()));
            self.core.say(t("capture_hint"));
        }
    }

    fn capturing(&self, target: KeyTarget) -> bool {
        matches!(&self.capture, Some((t, _)) if *t == target)
    }

    fn sync_wave(&mut self) {
        let v = self.core.clip_version();
        if v != self.wave.version {
            match self.core.engine.channel_audio(CLIP) {
                Some((data, start, end)) => self.wave.set_audio(v, &data, codec::SR, start, end),
                None => self.wave.version = v,
            }
        }
    }

    fn apply(&mut self, actions: Vec<RowAction>) {
        for action in actions {
            match action {
                RowAction::Toggle(id) => self.core.play_slot(id, true),
                RowAction::Play(id) => self.core.play_slot(id, false),
                RowAction::Pause(id) => self.core.engine.pause(id),
                RowAction::Stop(id) => self.core.engine.stop(id),
                RowAction::Pick(id) => {
                    if let Some(path) = pick_files(false).into_iter().next() {
                        self.core.set_slot_file(id, path);
                    }
                }
                RowAction::Remove(id) => self.core.remove_slot(id),
                RowAction::Seek(id, s) => self.core.engine.seek(id, s),
                RowAction::Volume(id, v, save) => {
                    self.core.set_slot_volume(id, v);
                    if save {
                        self.core.save_cfg();
                    }
                }
                RowAction::Edit(target) => self.begin_capture(target),
                RowAction::Clear(target) => {
                    self.core.set_key(target, String::new());
                    self.core.say(t("key_removed"));
                }
                RowAction::MoveSlot(id, delta) => self.core.move_slot(id, delta),
                RowAction::SetCollection(id, name) => self.core.set_slot_collection(id, name),
                RowAction::NewCollection(assign) => {
                    self.dialog = Some(NameDialog::new(DialogKind::NewCollection { assign }, String::new()));
                }
                RowAction::AddInto(name) => {
                    let files = pick_files(true);
                    self.core.add_sounds(files, Some(name));
                }
                RowAction::ToggleCollection(name) => self.core.toggle_collection(&name),
                RowAction::MoveCollection(name, delta) => self.core.move_collection(&name, delta),
                RowAction::RenameCollection(name) => {
                    self.dialog = Some(NameDialog::new(DialogKind::RenameCollection(name.clone()), name));
                }
                RowAction::DeleteCollection(name) => self.core.delete_collection(&name),
            }
        }
    }

    // ---------- cabecalho ----------
    fn header(&mut self, ui: &mut Ui, time: f64) {
        let mut actions = Vec::new();
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::hover());
            super::logo::paint(ui.painter(), rect, self.levels[1], time);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let (r, _) = ui.allocate_exact_size(vec2(230.0, 30.0), Sense::hover());
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    "SOUND MOD",
                    0.0,
                    egui::TextFormat {
                        font_id: display(29.0),
                        extra_letter_spacing: 3.0,
                        color: Color32::PLACEHOLDER, // cada camada do halo escolhe a cor ao pintar
                        ..Default::default()
                    },
                );
                let galley = ui.painter().layout_job(job);
                let pos = pos2(r.left(), r.center().y - galley.size().y / 2.0);
                let k = theme::pulse(time, 1.3);
                theme::glow(ui.painter(), egui::Rect::from_min_size(pos, galley.size()).shrink2(vec2(12.0, 9.0)), 8.0, neon((40.0 + 25.0 * k) as u8), 34.0);
                for (d, a) in [(3.0, 30), (2.0, 65), (1.0, 140)] {
                    for off in [vec2(-d, 0.0), vec2(d, 0.0), vec2(0.0, -d), vec2(0.0, d)] {
                        ui.painter().galley(pos + off, galley.clone(), neon(a));
                    }
                }
                if theme::has_spectrum() {
                    // nucleo do titulo em degrade, letra a letra
                    let mut job = egui::text::LayoutJob::default();
                    let letters: Vec<char> = "SOUND MOD".chars().collect();
                    for (i, ch) in letters.iter().enumerate() {
                        let c = mix(theme::spectrum_at(i as f32 / (letters.len() - 1) as f32), Color32::WHITE, 0.25);
                        let format = egui::TextFormat { font_id: display(29.0), extra_letter_spacing: 3.0, color: c, ..Default::default() };
                        job.append(&ch.to_string(), 0.0, format);
                    }
                    let colored = ui.painter().layout_job(job);
                    ui.painter().galley(pos, colored, accent_core());
                } else {
                    ui.painter().galley(pos, galley, accent_core());
                }
                ui.label(RichText::new(t("tagline")).font(font(12.5)).color(muted()));
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                // parar tudo (canto direito)
                let key = self.core.key_of(KeyTarget::StopAll);
                let capturing = self.capturing(KeyTarget::StopAll);
                let label = t("stop_all").to_uppercase();
                let label_w = ui.painter().layout_no_wrap(label.clone(), bold(12.0), Color32::PLACEHOLDER).size().x;
                let group_w = 14.0 + label_w + 10.0 + widgets::key_button_width(ui, &key, capturing) + 8.0;
                ui.allocate_ui_with_layout(vec2(group_w, 44.0), Layout::left_to_right(Align::Center), |ui| {
                    Frame::new()
                        .fill(DANGER_DEEP)
                        .stroke(Stroke::new(1.0, alpha_of(DANGER, 90)))
                        .corner_radius(22)
                        .inner_margin(Margin { left: 14, right: 6, top: 5, bottom: 5 })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            ui.label(RichText::new(label).font(bold(12.0)).color(mix(DANGER, txt(), 0.35)));
                            let kr = widgets::key_button(ui, Icon::Stop, &key, false, capturing, self.capture.is_some(), true, time);
                            if kr.action.clicked() {
                                self.core.engine.stop_all();
                            }
                            if kr.edit.clicked() {
                                actions.push(RowAction::Edit(KeyTarget::StopAll));
                            }
                            if kr.clear.clicked() {
                                actions.push(RowAction::Clear(KeyTarget::StopAll));
                            }
                        });
                });
                self.language_menu(ui);
                self.accent_menu(ui, time);
                widgets::level_meter(ui, t("level_out"), self.levels[1]);
                widgets::level_meter(ui, t("level_mic"), self.levels[0]);
                widgets::status_pill(ui, self.core.engine.running(), time);
            });
        });
        self.apply(actions);
    }

    fn language_menu(&mut self, ui: &mut Ui) {
        let current = i18n::current();
        let mut selected = current;
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 34.0;
            ComboBox::from_id_salt("language")
                .width(128.0)
                .selected_text(RichText::new(i18n::display_name(current)).font(bold(13.0)))
                .show_ui(ui, |ui| {
                    for (lang, _, name) in i18n::LANGUAGES {
                        ui.selectable_value(&mut selected, lang, name);
                    }
                });
        });
        if selected != current {
            i18n::set_language(Some(i18n::code(selected)));
            self.core.cfg.lock().language = Some(i18n::code(selected).to_string());
            self.core.save_cfg();
        }
    }

    /// Botao da cor do neon com o menu das 8 cores.
    fn accent_menu(&mut self, ui: &mut Ui, time: f64) {
        let resp = widgets::accent_button(ui, time).on_hover_text(t("neon_color"));
        let current = theme::current_accent().key;
        let mut picked = None;
        egui::Popup::menu(&resp).width(200.0).show(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(t("neon_color").to_uppercase()).font(bold(11.5)).color(muted()));
            ui.add_space(2.0);
            for (themes, a) in theme::ACCENTS.iter().map(|a| (a.spectrum.is_some(), a)) {
                if themes && a.key == theme::ACCENTS.iter().find(|x| x.spectrum.is_some()).map_or("", |x| x.key) {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.label(RichText::new(t("themes").to_uppercase()).font(bold(11.5)).color(muted()));
                    ui.add_space(2.0);
                }
                let name = i18n::accent_name(a.key);
                if widgets::accent_option(ui, a, name, a.key == current).clicked() {
                    picked = Some(a.key);
                }
            }
        });
        if let Some(key) = picked.filter(|k| *k != current) {
            self.set_accent(ui.ctx(), key);
        }
    }

    /// Troca a cor do neon: estilo do egui, borda da janela, icone e config.
    fn set_accent(&self, ctx: &egui::Context, key: &str) {
        theme::set_accent(key);
        theme::apply_style(ctx);
        super::window::recolor();
        ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(Arc::new(super::logo::icon_data()))));
        self.core.cfg.lock().accent = key.to_string();
        self.core.save_cfg();
        self.core.say(tf("accent_set", &[("name", &i18n::accent_name(key))]));
    }

    // ---------- sons ----------
    fn sounds_card(&mut self, ui: &mut Ui, time: f64) {
        let w = ui.available_width();
        let h = ui.available_height().max(190.0);
        let mut actions = Vec::new();
        ui.allocate_ui_with_layout(vec2(w, h), Layout::top_down(Align::Min), |ui| {
            widgets::card(ui, |ui| {
                ui.set_min_size(vec2(w - 34.0, h - 34.0));
                ui.set_max_width(w - 34.0);
                ui.horizontal(|ui| {
                    widgets::title(ui, Icon::Waves, t("sounds"));
                    ui.add_space(8.0);
                    let hint_w = (ui.available_width() - 500.0).max(40.0);
                    widgets::label_truncated(ui, t("sounds_hint"), hint_w, font(12.5), muted()).on_hover_text(t("sounds_hint"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::button(ui, t("add_sound"), Some(Icon::Plus), Style::Primary, 0.0).clicked() {
                            let files = pick_files(true);
                            self.core.add_sounds(files, None);
                        }
                        if widgets::button(ui, t("new_collection"), Some(Icon::Layers), Style::Ghost, 0.0).clicked() {
                            actions.push(RowAction::NewCollection(None));
                        }
                        let dir = config::sounds_dir();
                        let tip = tf("tip_open_folder", &[("path", &dir.display())]);
                        if widgets::button(ui, t("open_folder"), Some(Icon::Folder), Style::Ghost, 0.0).on_hover_text(tip).clicked() {
                            let _ = std::fs::create_dir_all(&dir);
                            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                        }
                    });
                });
                ui.add_space(8.0);
                let list_h = ui.available_height();
                // copia: a lista e desenhada sem segurar o lock da config
                let cfg = self.core.cfg.lock().clone();
                let labels = library::names(&cfg);
                ScrollArea::vertical()
                    .id_salt("slots")
                    .auto_shrink([false, false])
                    .max_height(list_h)
                    .show(ui, |ui| {
                        ui.add_space(4.0);
                        let ncols = cfg.collections.len();
                        for group in library::groups(&cfg) {
                            // sem nenhuma colecao, a lista fica sem cabecalhos
                            if ncols > 0 && (group.collection.is_some() || !group.slots.is_empty()) {
                                let index = group.collection.and_then(|c| cfg.collections.iter().position(|x| x == c));
                                self.collection_header(ui, group.collection, group.slots.len(), index, ncols, &mut actions);
                            }
                            if group.collection.is_some_and(|c| c.collapsed) {
                                continue; // recolhida: some da lista, mas as teclas continuam valendo
                            }
                            let n = group.slots.len();
                            for (i, slot) in group.slots.iter().enumerate() {
                                self.slot_row(ui, slot, (i > 0, i + 1 < n), &labels, time, &mut actions);
                            }
                        }
                    });
            });
        });
        self.apply(actions);
    }

    /// Cabecalho de um grupo: nome e quantidade; nas colecoes, recolher, adicionar som, mudar de
    /// posicao, renomear e apagar. `coll` None = grupo dos sons sem colecao.
    fn collection_header(
        &self,
        ui: &mut Ui,
        coll: Option<&Collection>,
        count: usize,
        index: Option<usize>,
        total: usize,
        actions: &mut Vec<RowAction>,
    ) {
        Frame::new()
            .fill(header_fill())
            .corner_radius(12)
            .inner_margin(Margin { left: 6, right: 8, top: 4, bottom: 4 })
            .outer_margin(Margin { left: 4, right: 10, top: 6, bottom: 6 })
            .stroke(Stroke::new(1.0, if coll.is_some() { neon(55) } else { border() }))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let Some(c) = coll else {
                        ui.add_space(8.0);
                        ui.label(RichText::new(t("ungrouped").to_uppercase()).font(bold(12.5)).color(muted()));
                        widgets::count_badge(ui, count, false);
                        return;
                    };
                    let chevron = if c.collapsed { Icon::ChevronRight } else { Icon::ChevronDown };
                    let toggle = widgets::icon_button(ui, chevron, 28.0, Tint::Flat, true).on_hover_text(t("tip_collapse"));
                    let name = widgets::collection_name(ui, &c.name, !c.collapsed).on_hover_text(t("tip_collapse"));
                    if toggle.clicked() || name.clicked() {
                        actions.push(RowAction::ToggleCollection(c.name.clone()));
                    }
                    widgets::count_badge(ui, count, true);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let i = index.unwrap_or(0);
                        let del = widgets::icon_button(ui, Icon::Trash, 28.0, Tint::Danger, true);
                        if del.on_hover_text(t("tip_delete_collection")).clicked() {
                            actions.push(RowAction::DeleteCollection(c.name.clone()));
                        }
                        let ren = widgets::icon_button(ui, Icon::Edit, 28.0, Tint::Flat, true);
                        if ren.on_hover_text(t("tip_rename_collection")).clicked() {
                            actions.push(RowAction::RenameCollection(c.name.clone()));
                        }
                        if widgets::icon_button(ui, Icon::ArrowDown, 28.0, Tint::Flat, i + 1 < total).clicked() {
                            actions.push(RowAction::MoveCollection(c.name.clone(), 1));
                        }
                        if widgets::icon_button(ui, Icon::ArrowUp, 28.0, Tint::Flat, i > 0).clicked() {
                            actions.push(RowAction::MoveCollection(c.name.clone(), -1));
                        }
                        ui.add_space(4.0);
                        let add = widgets::button_sized(ui, t("add_here"), Some(Icon::Plus), Style::Ghost, 0.0, 28.0);
                        if add.on_hover_text(t("tip_add_here")).clicked() {
                            actions.push(RowAction::AddInto(c.name.clone()));
                        }
                    });
                });
            });
    }

    /// Uma linha de som. `order` = (pode subir, pode descer) dentro do grupo.
    fn slot_row(
        &self,
        ui: &mut Ui,
        slot: &Slot,
        order: (bool, bool),
        collections: &[String],
        time: f64,
        actions: &mut Vec<RowAction>,
    ) {
        let id = slot.id;
        let state = self.core.channel_state(id);
        let (pos, dur, playing) = state.unwrap_or((0.0, 0.0, false));
        let k = theme::pulse(time, 5.0);
        let frame = Frame::new()
            .fill(if playing { mix(item(), accent_deep(), 0.55) } else { item() })
            .corner_radius(14)
            .inner_margin(Margin { left: 6, right: 10, top: 7, bottom: 7 })
            .outer_margin(Margin { left: 4, right: 10, top: 0, bottom: 0 })
            .stroke(Stroke::new(1.0, if playing { neon((150.0 + 100.0 * k) as u8) } else { border() }))
            .shadow(if playing {
                Shadow { offset: [0, 0], blur: 20, spread: 0, color: neon((45.0 + 55.0 * k) as u8) }
            } else {
                Shadow::NONE
            });
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                // setas de ordem empilhadas
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let up = widgets::icon_button_sized(ui, Icon::ArrowUp, vec2(24.0, 15.0), Tint::Flat, order.0);
                    if up.on_hover_text(t("tip_up")).clicked() {
                        actions.push(RowAction::MoveSlot(id, -1));
                    }
                    let down = widgets::icon_button_sized(ui, Icon::ArrowDown, vec2(24.0, 15.0), Tint::Flat, order.1);
                    if down.on_hover_text(t("tip_down")).clicked() {
                        actions.push(RowAction::MoveSlot(id, 1));
                    }
                });
                let target = KeyTarget::Slot(id);
                let kr = widgets::key_button(
                    ui,
                    Icon::Play,
                    &slot.key,
                    playing,
                    self.capturing(target),
                    self.capture.is_some(),
                    false,
                    time,
                );
                if kr.action.clicked() {
                    actions.push(RowAction::Toggle(id));
                }
                if kr.edit.clicked() {
                    actions.push(RowAction::Edit(target));
                }
                if kr.clear.clicked() {
                    actions.push(RowAction::Clear(target));
                }
                if widgets::icon_button(ui, Icon::Folder, 30.0, Tint::Normal, true).on_hover_text(t("tip_file")).clicked() {
                    actions.push(RowAction::Pick(id));
                }
                self.collection_menu(ui, slot, collections, actions);

                let compact = ui.available_width() < 640.0;
                let (time_w, prog_w, vol_w) = if compact { (76.0, 80.0, 56.0) } else { (86.0, 150.0, 80.0) };
                let fixed = time_w + prog_w + 18.0 + vol_w + 4.0 * 30.0 + 9.0 * 6.0;
                let name_w = (ui.available_width() - fixed).max(50.0);
                let empty = slot.path.is_empty();
                let name = if empty { t("no_file").to_string() } else { config::file_name(&slot.path) };
                let color = if empty { muted() } else if playing { accent_core() } else { txt() };
                let r = widgets::label_truncated(ui, &name, name_w, bold(13.5), color);
                if !empty {
                    r.on_hover_text(&slot.path);
                }

                let time_text = if state.is_some() {
                    format!("{} / {}", theme::mmss(pos), theme::mmss(dur))
                } else {
                    "-:-- / -:--".to_string()
                };
                let (tr, _) = ui.allocate_exact_size(vec2(time_w, 20.0), Sense::hover());
                let time_color = if playing { accent_core() } else { muted() };
                ui.painter().text(tr.right_center(), Align2::RIGHT_CENTER, time_text, font(12.0), time_color);

                let mut p = pos as f32;
                let seek = widgets::slider(ui, &mut p, 0.0..=(dur.max(0.01) as f32), prog_w, false, state.is_some());
                if seek.changed() {
                    actions.push(RowAction::Seek(id, p as f64));
                }

                let (vr, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                icons::paint(ui.painter(), vr.shrink(1.0), Icon::Volume, muted());
                let mut vol = slot.volume;
                let vresp = widgets::slider(ui, &mut vol, 0.0..=1.5, vol_w, true, true).on_hover_text(t("tip_volume"));
                if vresp.changed() {
                    actions.push(RowAction::Volume(id, vol, false));
                }
                if vresp.drag_stopped() || vresp.clicked() {
                    actions.push(RowAction::Volume(id, vol, true));
                }

                if widgets::icon_button(ui, Icon::Play, 30.0, Tint::Normal, true).on_hover_text(t("tip_play")).clicked() {
                    actions.push(RowAction::Play(id));
                }
                if widgets::icon_button(ui, Icon::Pause, 30.0, Tint::Normal, true).on_hover_text(t("tip_pause")).clicked() {
                    actions.push(RowAction::Pause(id));
                }
                if widgets::icon_button(ui, Icon::Stop, 30.0, Tint::Normal, true).on_hover_text(t("tip_stop")).clicked() {
                    actions.push(RowAction::Stop(id));
                }
                if widgets::icon_button(ui, Icon::Trash, 30.0, Tint::Danger, true).on_hover_text(t("tip_remove")).clicked() {
                    actions.push(RowAction::Remove(id));
                }
            });
        });
        ui.add_space(7.0);
    }

    /// Botao de colecao do som: menu com "(sem colecao)", as colecoes e "Nova colecao".
    fn collection_menu(&self, ui: &mut Ui, slot: &Slot, collections: &[String], actions: &mut Vec<RowAction>) {
        let current = slot.collection.clone();
        let tip = format!("{}: {}", t("tip_collection"), current.as_deref().unwrap_or(t("no_collection")));
        let tint = if current.is_some() { Tint::Normal } else { Tint::Flat };
        let resp = widgets::icon_button(ui, Icon::Layers, 30.0, tint, true).on_hover_text(tip);
        egui::Popup::menu(&resp).width(220.0).show(|ui| {
            ui.style_mut().override_font_id = Some(font(13.5));
            if ui.selectable_label(current.is_none(), t("no_collection")).clicked() && current.is_some() {
                actions.push(RowAction::SetCollection(slot.id, None));
            }
            for name in collections {
                let here = current.as_deref() == Some(name.as_str());
                if ui.selectable_label(here, name.as_str()).clicked() && !here {
                    actions.push(RowAction::SetCollection(slot.id, Some(name.clone())));
                }
            }
            ui.separator();
            if ui.selectable_label(false, format!("+ {}", t("new_collection"))).clicked() {
                actions.push(RowAction::NewCollection(Some(slot.id)));
            }
        });
    }

    // ---------- clipe ----------
    fn clip_card(&mut self, ui: &mut Ui, time: f64) {
        let mut actions = Vec::new();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::title(ui, Icon::Record, t("clip_title"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, t("save_as_sound"), Some(Icon::Save), Style::Primary, 0.0).clicked() {
                        if self.core.engine.has(CLIP) {
                            self.dialog = Some(NameDialog::new(DialogKind::SaveSound, String::new()));
                        } else {
                            self.core.say(t("no_clip_saved"));
                        }
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                widgets::caption(ui, t("save_last"));
                let r = ui.add(
                    TextEdit::singleline(&mut self.secs_text)
                        .desired_width(34.0)
                        .align(Align2::CENTER_CENTER)
                        .font(bold(13.0)),
                );
                if r.changed() {
                    if let Ok(n) = self.secs_text.trim().parse::<u32>() {
                        let n = n.clamp(1, 60);
                        self.core.cfg.lock().clip_seconds = n;
                        self.core.apply_live();
                        self.core.save_cfg();
                    }
                }
                if r.lost_focus() {
                    self.secs_text = self.core.cfg.lock().clip_seconds.to_string();
                }
                widgets::caption(ui, t("seconds"));
                ui.add_space(2.0);
                let key = self.core.key_of(KeyTarget::SaveClip);
                let kr = widgets::key_button(
                    ui,
                    Icon::Record,
                    &key,
                    false,
                    self.capturing(KeyTarget::SaveClip),
                    self.capture.is_some(),
                    false,
                    time,
                );
                if kr.action.on_hover_text(t("tip_record")).clicked() {
                    self.core.save_clip();
                }
                if kr.edit.clicked() {
                    actions.push(RowAction::Edit(KeyTarget::SaveClip));
                }
                if kr.clear.clicked() {
                    actions.push(RowAction::Clear(KeyTarget::SaveClip));
                }

                ui.add_space(18.0);
                widgets::caption(ui, t("clip"));
                let playing = self.core.engine.state(CLIP).is_some_and(|s| s.2);
                let key = self.core.key_of(KeyTarget::PlayClip);
                let kr = widgets::key_button(
                    ui,
                    Icon::Play,
                    &key,
                    playing,
                    self.capturing(KeyTarget::PlayClip),
                    self.capture.is_some(),
                    false,
                    time,
                );
                if kr.action.clicked() {
                    self.core.toggle_clip();
                }
                if kr.edit.clicked() {
                    actions.push(RowAction::Edit(KeyTarget::PlayClip));
                }
                if kr.clear.clicked() {
                    actions.push(RowAction::Clear(KeyTarget::PlayClip));
                }
                if widgets::icon_button(ui, Icon::Pause, 30.0, Tint::Normal, true).on_hover_text(t("tip_pause")).clicked() {
                    self.core.engine.pause(CLIP);
                }
                if widgets::icon_button(ui, Icon::Stop, 30.0, Tint::Normal, true).on_hover_text(t("tip_stop")).clicked() {
                    self.core.engine.stop(CLIP);
                }
            });
            ui.add_space(8.0);
            let st = self.core.engine.state(CLIP);
            match self.wave.ui(ui, 112.0, st.map(|s| s.0), st.is_some_and(|s| s.2), time) {
                Some(WaveEvent::Range(start, end)) => self.core.set_clip_range(start, end),
                Some(WaveEvent::Seek(at)) => self.core.engine.seek(CLIP, at),
                None => {}
            }
            ui.add_space(4.0);
            widgets::caption(ui, t("clip_hint"));
        });
        self.apply(actions);
    }

    // ---------- lateral ----------
    fn devices_card(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::title(ui, Icon::Sliders, t("devices"));
            ui.add_space(4.0);
            let none = t("none").to_string();
            let mut cfg = self.core.cfg.lock().clone();
            let mut changed = false;
            changed |= device_combo(ui, "mic", t("mic"), &mut cfg.mic_device, &self.inputs, &none);
            changed |= device_combo(ui, "out", t("output"), &mut cfg.output_device, &self.outputs, &none);
            changed |= device_combo(ui, "mon", t("monitor"), &mut cfg.monitor_device, &self.outputs, &none);
            // loopback: None = alto-falante padrao
            let speaker = t("default_speaker").to_string();
            let mut lb = Some(cfg.loopback_device.clone().unwrap_or_default());
            if device_combo(ui, "loop", t("loopback"), &mut lb, &self.outputs, &speaker) {
                cfg.loopback_device = lb.filter(|s| !s.is_empty());
                changed = true;
            }
            ui.add_space(8.0);
            let mut toggled = widgets::toggle(ui, &mut cfg.mix_mic, t("mix_mic")).changed();
            toggled |= widgets::toggle(ui, &mut cfg.monitor, t("hear_sounds")).changed();
            ui.add_space(4.0);
            let (mut volume_moved, mut save_volume) = (false, false);
            ui.horizontal(|ui| {
                widgets::caption(ui, t("master_volume"));
                let w = ui.available_width();
                let r = widgets::slider(ui, &mut cfg.master_volume, 0.0..=1.0, w, false, true);
                volume_moved = r.changed();
                save_volume = r.drag_stopped() || r.clicked();
            });
            {
                let mut shared = self.core.cfg.lock();
                shared.mic_device = cfg.mic_device.clone();
                shared.output_device = cfg.output_device.clone();
                shared.monitor_device = cfg.monitor_device.clone();
                shared.loopback_device = cfg.loopback_device.clone();
                shared.mix_mic = cfg.mix_mic;
                shared.monitor = cfg.monitor;
                shared.master_volume = cfg.master_volume;
            }
            if toggled || volume_moved {
                self.core.apply_live();
            }
            // o volume so e gravado ao soltar o slider, nao a cada quadro do arraste
            if changed || toggled || save_volume {
                self.core.save_cfg();
            }
            ui.add_space(10.0);
            let w = ui.available_width();
            if widgets::button(ui, t("apply"), Some(Icon::Power), Style::Primary, w).clicked() {
                self.start_all();
            }
        });
    }

    fn voice_card(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::title(ui, Icon::Mic, t("voice"));
            ui.add_space(6.0);
            let current = self.core.cfg.lock().voice_effect.clone();
            let w = (ui.available_width() - 12.0) / 3.0;
            for (row, names) in voice::EFFECTS.chunks(3).enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (col, name) in names.iter().enumerate() {
                        let label = t(FX_KEYS[row * 3 + col]);
                        if widgets::choice(ui, label, current == *name, w).clicked() && current != *name {
                            self.core.set_voice(name);
                        }
                    }
                });
                ui.add_space(-2.0);
            }
            ui.add_space(8.0);
            let mut hear = self.core.cfg.lock().hear_self;
            if widgets::toggle(ui, &mut hear, t("hear_self")).changed() {
                self.core.cfg.lock().hear_self = hear;
                self.core.apply_live();
                self.core.save_cfg();
            }
        });
    }

    // ---------- status ----------
    fn status_bar(&self, ui: &mut Ui, time: f64) {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
            let k = theme::pulse(time, 2.0);
            theme::glow(ui.painter(), egui::Rect::from_center_size(dot.center(), vec2(5.0, 5.0)), 3.0, neon((60.0 + 60.0 * k) as u8), 8.0);
            ui.painter().circle_filled(dot.center(), 2.5, accent());
            let w = (ui.available_width() - 320.0).max(80.0);
            widgets::label_truncated(ui, &self.core.status(), w, font(12.5), mix(muted(), txt(), 0.5));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("v{} · Rust", env!("CARGO_PKG_VERSION"))).font(font(11.5)).color(dim()));
                ui.add_space(6.0);
                let dir = config::data_dir();
                let tip = tf("tip_data_folder", &[("path", &dir.display())]);
                let open = widgets::button_sized(ui, t("data_folder"), Some(Icon::Folder), Style::Ghost, 0.0, 26.0);
                if open.on_hover_text(tip).clicked() {
                    let _ = std::fs::create_dir_all(dir);
                    let _ = std::process::Command::new("explorer").arg(dir).spawn();
                }
            });
        });
    }

    /// Janelinha de nome: salvar clipe como som, nova colecao ou renomear colecao.
    fn name_modal(&mut self, ctx: &egui::Context) {
        let Some(dlg) = self.dialog.as_mut() else { return };
        let (title, label, icon, hint) = match &dlg.kind {
            DialogKind::SaveSound => (t("save_sound_title").to_string(), t("sound_name"), Icon::Save, default_sound_name()),
            DialogKind::NewCollection { .. } => {
                (t("new_collection_title").to_string(), t("collection_name"), Icon::Layers, String::new())
            }
            DialogKind::RenameCollection(old) => {
                (tf("rename_collection_title", &[("name", old)]), t("collection_name"), Icon::Edit, String::new())
            }
        };
        let (mut submit, mut close) = (false, false);
        let focus = std::mem::take(&mut dlg.focus);
        let modal = egui::Modal::new(Id::unique("name_dialog"))
            .backdrop_color(Color32::from_black_alpha(170))
            .frame(
                Frame::new()
                    .fill(panel())
                    .stroke(Stroke::new(1.0, neon(150)))
                    .corner_radius(18)
                    .inner_margin(Margin::same(22))
                    .shadow(Shadow { offset: [0, 0], blur: 60, spread: 0, color: neon(55) }),
            )
            .show(ctx, |ui| {
                ui.set_width(400.0);
                widgets::title(ui, icon, &title);
                ui.add_space(10.0);
                widgets::caption(ui, label);
                let r = ui.add(
                    TextEdit::singleline(&mut dlg.text)
                        .hint_text(hint)
                        .desired_width(f32::INFINITY)
                        .font(font(15.0))
                        .margin(Margin::symmetric(10, 8)),
                );
                if focus {
                    r.request_focus();
                }
                if r.changed() {
                    dlg.error = None;
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if let Some(err) = &dlg.error {
                    ui.add_space(4.0);
                    ui.label(RichText::new(err.as_str()).font(font(12.5)).color(DANGER));
                }
                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = if matches!(dlg.kind, DialogKind::SaveSound) { t("save") } else { "OK" };
                    if widgets::button(ui, ok, Some(Icon::Save), Style::Primary, 110.0).clicked() {
                        submit = true;
                    }
                    if widgets::button(ui, t("cancel"), None, Style::Ghost, 100.0).clicked() {
                        close = true;
                    }
                });
            });
        if modal.should_close() {
            close = true;
        }
        if submit {
            let result = match &dlg.kind {
                DialogKind::SaveSound => {
                    self.core.save_as_sound(&dlg.text);
                    Ok(())
                }
                DialogKind::NewCollection { assign } => self.core.create_collection(&dlg.text, *assign),
                DialogKind::RenameCollection(old) => self.core.rename_collection(old, &dlg.text),
            };
            match result {
                Ok(()) => self.dialog = None,
                Err(e) => {
                    dlg.error = Some(e);
                    dlg.focus = true;
                }
            }
        } else if close {
            self.dialog = None;
        }
    }
}

impl eframe::App for SoundApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let time = ctx.input(|i| i.time);
        ctx.request_repaint_after(Duration::from_millis(33));
        self.poll_capture();
        if self.core.engine.take_device_lost() {
            self.core.say(t("device_lost"));
        }
        let (mic, out) = self.core.engine.levels();
        let decay = 0.86;
        self.levels = [(self.levels[0] * decay).max(mic), (self.levels[1] * decay).max(out)];
        if !self.core.engine.running() {
            self.levels = [0.0; 2];
        }
        self.sync_wave();

        paint_background(ui, time);

        Panel::top("header")
            .frame(Frame::NONE.inner_margin(Margin { left: 22, right: 22, top: 16, bottom: 12 }))
            .show_separator_line(false)
            .resizable(false)
            .show(ui, |ui| self.header(ui, time));
        if !self.cable_found {
            Panel::top("cable")
                .frame(Frame::NONE.inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 10 }))
                .show_separator_line(false)
                .resizable(false)
                .show(ui, |ui| {
                    ui.label(RichText::new(t("no_cable")).font(font(12.5)).color(WARN));
                });
        }
        Panel::bottom("status")
            .frame(Frame::NONE.inner_margin(Margin { left: 22, right: 22, top: 4, bottom: 10 }))
            .show_separator_line(false)
            .resizable(false)
            .show(ui, |ui| self.status_bar(ui, time));
        Panel::right("side")
            .exact_size(SIDEBAR)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::NONE.inner_margin(Margin { left: 6, right: 20, top: 2, bottom: 6 }))
            .show(ui, |ui| {
                ScrollArea::vertical().id_salt("side").auto_shrink([false, false]).show(ui, |ui| {
                    self.devices_card(ui);
                    ui.add_space(14.0);
                    self.voice_card(ui);
                });
            });
        CentralPanel::no_frame()
            .frame(Frame::NONE.inner_margin(Margin { left: 20, right: 8, top: 2, bottom: 6 }))
            .show(ui, |ui| {
                // o clipe fica embaixo com a altura que precisar; os sons ocupam o resto
                Panel::bottom("clip_panel")
                    .frame(Frame::NONE.inner_margin(Margin { left: 0, right: 0, top: 14, bottom: 0 }))
                    .show_separator_line(false)
                    .resizable(false)
                    .show(ui, |ui| self.clip_card(ui, time));
                self.sounds_card(ui, time);
            });
        self.name_modal(&ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        bg().to_normalized_gamma_f32()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(hk) = self.core.hotkeys.get() {
            hk.cancel_capture();
        }
        self.core.save_cfg();
        self.core.engine.close(&mut self.streams);
        self.loopback = None;
    }
}

/// Fundo: grafite com grade sutil e luzes verdes difusas nos cantos.
fn paint_background(ui: &Ui, time: f64) {
    let rect = ui.max_rect();
    let p = ui.painter();
    p.rect_filled(rect, 0.0, bg());
    let grid = Stroke::new(1.0, Color32::from_white_alpha(4));
    let step = 34.0;
    let mut x = rect.left() + step;
    while x < rect.right() {
        p.vline(x, rect.y_range(), grid);
        x += step;
    }
    let mut y = rect.top() + step;
    while y < rect.bottom() {
        p.hline(rect.x_range(), y, grid);
        y += step;
    }
    let k = theme::pulse(time, 0.7);
    // no tema com degrade: magenta no alto, amarelo quente embaixo (ceu do por do sol)
    let (top, low) = (theme::spectrum_at(0.0), theme::spectrum_at(1.0));
    theme::ambient(p, pos2(rect.left() + 170.0, rect.top() + 20.0), vec2(460.0, 110.0), alpha_of(top, (16.0 + 12.0 * k) as u8));
    theme::ambient(p, pos2(rect.right() - 220.0, rect.bottom() - 10.0), vec2(560.0, 120.0), alpha_of(low, 14));
    // linha de luz no topo da janela
    let top = egui::Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + 2.0));
    p.rect_filled(top, 0.0, neon(90));
}

fn device_combo(ui: &mut Ui, id: &str, label: &str, value: &mut Option<String>, options: &[String], none: &str) -> bool {
    ui.add_space(2.0);
    widgets::caption(ui, label);
    let current = value.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| none.to_string());
    let mut selected = current.clone();
    let w = ui.available_width();
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 32.0;
        ComboBox::from_id_salt(id)
            .width(w)
            .truncate()
            .selected_text(RichText::new(current.as_str()).font(font(13.0)))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, none.to_string(), none);
                for o in options {
                    ui.selectable_value(&mut selected, o.clone(), o.as_str());
                }
            });
    });
    if selected != current {
        *value = Some(if selected == none { String::new() } else { selected });
        true
    } else {
        false
    }
}

/// Seletor de arquivos de audio (um ou varios).
fn pick_files(many: bool) -> Vec<String> {
    let dialog = rfd::FileDialog::new()
        .add_filter(t("audio_files"), &codec::AUDIO_EXTENSIONS)
        .add_filter(t("all_files"), &["*"]);
    let files = if many { dialog.pick_files().unwrap_or_default() } else { dialog.pick_file().into_iter().collect() };
    files.into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
}
