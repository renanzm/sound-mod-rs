//! Textos da interface em cada idioma.
//!
//! Para adicionar um idioma: inclua em `LANGUAGES` e crie a tabela com as mesmas chaves e os
//! mesmos campos `{...}`. Nunca escreva texto da interface direto no codigo: crie a chave aqui.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Pt = 0,
    En = 1,
}

/// (idioma, codigo na config, nome no menu)
pub const LANGUAGES: [(Lang, &str, &str); 2] = [(Lang::Pt, "pt", "Português"), (Lang::En, "en", "English")];

static CURRENT: AtomicU8 = AtomicU8::new(Lang::En as u8);

const PT: &[(&str, &str)] = &[
    ("none", "(nenhum)"),
    ("default_speaker", "(alto-falante padrão)"),
    ("mic", "Microfone real"),
    ("output", "Saída → cabo virtual"),
    ("monitor", "Ouvir sons em (fones)"),
    ("loopback", "Capturar áudio do PC"),
    ("mix_mic", "Misturar meu microfone"),
    ("hear_sounds", "Ouvir os sons"),
    ("master_volume", "Volume geral"),
    ("apply", "Aplicar / Reiniciar"),
    ("no_cable", "Cabo virtual não encontrado. Instale o VB-CABLE (vb-audio.com/Cable), reinicie o app e use 'CABLE Output' como microfone no Discord/jogo."),
    ("sounds", "Sons"),
    ("sounds_hint", "▶ toca/para  ·  ✎ muda a tecla  ·  ⌫ remove a tecla  ·  ▲▼ muda a ordem  ·  um som por vez"),
    ("add_sound", "Adicionar som"),
    ("no_file", "(sem arquivo)"),
    ("audio_files", "Áudio"),
    ("all_files", "Todos"),
    ("save_last", "Salvar últimos"),
    ("seconds", "seg"),
    ("clip", "Clipe"),
    ("stop_all", "Parar tudo"),
    ("clip_hint", "Arraste as alças acesas para escolher o trecho que toca; clique na onda para mudar a posição."),
    ("no_clip", "Nenhum clipe ainda: pressione a tecla de salvar"),
    ("selection", "trecho {sel}s de {total}s"),
    ("ready", "Pronto."),
    ("loaded", "Carregado: {name}"),
    ("load_error", "Erro ao carregar {name}: {e}"),
    ("slot_empty", "Esse slot não tem arquivo."),
    ("engine_stopped", "Engine parada: escolha a saída e clique em Aplicar."),
    ("playing", "Tocando: {name}"),
    ("stopped", "Parado: {name}"),
    ("press_key", "tecla..."),
    ("capture_hint", "Pressione a nova tecla (Esc cancela)."),
    ("capture_cancelled", "Troca de tecla cancelada."),
    ("key_removed", "Tecla removida."),
    ("key_set", "Tecla definida: {key}"),
    ("clip_save_error", "Erro ao salvar clipe: {e}"),
    ("clip_saved", "Clipe salvo ({dur}s): {name}"),
    ("clip_open_error", "Erro ao abrir clipe: {e}"),
    ("no_clip_saved", "Ainda não há clipe salvo."),
    ("clip_playing", "Tocando clipe"),
    ("clip_stopped", "Clipe parado"),
    ("warn_output", "AVISO: a saída deve ser o cabo virtual (CABLE Input)"),
    ("monitor_off", "monitor desligado: escolha seus fones em 'Ouvir sons em'"),
    ("audio_on", "áudio ligado"),
    ("audio_error", "áudio: {e}"),
    ("capture_on", "gravação do PC ligada"),
    ("capture_error", "gravação do PC: {e}"),
    ("invalid_key", "Tecla inválida: {keys}"),
    ("select_output", "Selecione o dispositivo de saída (cabo virtual)."),
    ("device_missing", "dispositivo não encontrado: {name}"),
    ("device_lost", "Um dispositivo de áudio foi desconectado: confira e clique em Aplicar."),
    ("nothing_recorded", "Nada gravado ainda (sem áudio tocando no PC)."),
    ("voice", "Efeito de voz"),
    ("hear_self", "Ouvir minha voz"),
    ("fx_normal", "Normal"),
    ("fx_deep", "Grave"),
    ("fx_high", "Agudo"),
    ("fx_robot", "Robô"),
    ("fx_ogre", "Ogro"),
    ("fx_autotune", "Autotune"),
    ("fx_echo", "Eco"),
    ("fx_radio", "Rádio"),
    ("fx_alien", "Alien"),
    ("voice_on", "Efeito de voz: {name}"),
    ("save_as_sound", "Salvar como som"),
    ("save_sound_title", "Salvar clipe como som"),
    ("sound_name", "Nome do som:"),
    ("sound_saved", "Som salvo e adicionado: {name}"),
    ("sound_save_error", "Erro ao salvar som: {e}"),
    ("sounds_added", "Sons adicionados: {n}"),
    ("new_collection", "Nova coleção"),
    ("add_here", "Som"),
    ("no_collection", "(sem coleção)"),
    ("ungrouped", "Sem coleção"),
    ("collection_name", "Nome da coleção:"),
    ("new_collection_title", "Nova coleção"),
    ("rename_collection_title", "Renomear coleção {name}"),
    ("collection_empty", "O nome da coleção não pode ficar vazio."),
    ("collection_exists", "Já existe uma coleção chamada {name}."),
    ("collection_created", "Coleção criada: {name}"),
    ("collection_renamed", "Coleção renomeada: {old} → {name}"),
    ("collection_deleted", "Coleção apagada: {name}. Os sons dela foram para Sem coleção."),
    ("moved_to", "{sound} → {name}"),
    ("config_imported", "Configuração da versão anterior importada de {path}"),
    ("data_folder", "Pasta de dados"),
    ("tip_up", "Subir na lista"),
    ("tip_down", "Descer na lista"),
    ("tip_collection", "Coleção deste som"),
    ("tip_collapse", "Recolher / expandir"),
    ("tip_add_here", "Adicionar sons nesta coleção"),
    ("tip_rename_collection", "Renomear a coleção"),
    ("tip_delete_collection", "Apagar a coleção (os sons vão para Sem coleção)"),
    ("tip_data_folder", "Abrir {path}"),
    ("neon_color", "Cor do neon"),
    ("accent_set", "Cor do neon: {name}"),
    ("accent_green", "Verde"),
    ("accent_cyan", "Ciano"),
    ("accent_blue", "Azul"),
    ("accent_purple", "Roxo"),
    ("accent_pink", "Rosa"),
    ("accent_red", "Vermelho"),
    ("accent_orange", "Laranja"),
    ("accent_yellow", "Amarelo"),
    ("accent_sunset", "Pôr do sol"),
    ("themes", "Temas"),
    ("tagline", "Soundboard direto no seu microfone"),
    ("devices", "Dispositivos"),
    ("live", "Ao vivo"),
    ("offline", "Parado"),
    ("clip_title", "Clipe do áudio do PC"),
    ("file_missing", "Arquivo não encontrado: {name}"),
    ("decode_failed", "formato não suportado"),
    ("cancel", "Cancelar"),
    ("save", "Salvar"),
    ("level_mic", "MIC"),
    ("level_out", "SAÍDA"),
    ("tip_edit", "Mudar a tecla"),
    ("tip_clear", "Remover a tecla"),
    ("tip_file", "Escolher arquivo"),
    ("tip_play", "Tocar (recomeça)"),
    ("tip_pause", "Pausar / continuar"),
    ("tip_stop", "Parar"),
    ("tip_remove", "Remover som"),
    ("tip_volume", "Volume deste som"),
    ("tip_record", "Salvar os últimos segundos do áudio do PC"),
];

const EN: &[(&str, &str)] = &[
    ("none", "(none)"),
    ("default_speaker", "(default speaker)"),
    ("mic", "Real microphone"),
    ("output", "Output → virtual cable"),
    ("monitor", "Hear sounds on (headphones)"),
    ("loopback", "Capture PC audio"),
    ("mix_mic", "Mix my microphone"),
    ("hear_sounds", "Hear the sounds"),
    ("master_volume", "Master volume"),
    ("apply", "Apply / Restart"),
    ("no_cable", "Virtual cable not found. Install VB-CABLE (vb-audio.com/Cable), restart the app and use 'CABLE Output' as the microphone in Discord/your game."),
    ("sounds", "Sounds"),
    ("sounds_hint", "▶ play/stop  ·  ✎ change key  ·  ⌫ remove key  ·  ▲▼ change order  ·  one sound at a time"),
    ("add_sound", "Add sound"),
    ("no_file", "(no file)"),
    ("audio_files", "Audio"),
    ("all_files", "All files"),
    ("save_last", "Save last"),
    ("seconds", "sec"),
    ("clip", "Clip"),
    ("stop_all", "Stop all"),
    ("clip_hint", "Drag the glowing handles to choose the part that plays; click the waveform to change the position."),
    ("no_clip", "No clip yet: press the save key"),
    ("selection", "selection {sel}s of {total}s"),
    ("ready", "Ready."),
    ("loaded", "Loaded: {name}"),
    ("load_error", "Error loading {name}: {e}"),
    ("slot_empty", "This slot has no file."),
    ("engine_stopped", "Audio engine stopped: choose the output and click Apply."),
    ("playing", "Playing: {name}"),
    ("stopped", "Stopped: {name}"),
    ("press_key", "key..."),
    ("capture_hint", "Press the new key (Esc cancels)."),
    ("capture_cancelled", "Key change cancelled."),
    ("key_removed", "Key removed."),
    ("key_set", "Key set: {key}"),
    ("clip_save_error", "Error saving clip: {e}"),
    ("clip_saved", "Clip saved ({dur}s): {name}"),
    ("clip_open_error", "Error opening clip: {e}"),
    ("no_clip_saved", "No clip saved yet."),
    ("clip_playing", "Playing clip"),
    ("clip_stopped", "Clip stopped"),
    ("warn_output", "WARNING: the output should be the virtual cable (CABLE Input)"),
    ("monitor_off", "monitor off: choose your headphones in 'Hear sounds on'"),
    ("audio_on", "audio on"),
    ("audio_error", "audio: {e}"),
    ("capture_on", "PC audio capture on"),
    ("capture_error", "PC audio capture: {e}"),
    ("invalid_key", "Invalid key: {keys}"),
    ("select_output", "Select the output device (virtual cable)."),
    ("device_missing", "device not found: {name}"),
    ("device_lost", "An audio device was disconnected: check it and click Apply."),
    ("nothing_recorded", "Nothing recorded yet (no audio playing on the PC)."),
    ("voice", "Voice effect"),
    ("hear_self", "Hear my voice"),
    ("fx_normal", "Normal"),
    ("fx_deep", "Deep"),
    ("fx_high", "High"),
    ("fx_robot", "Robot"),
    ("fx_ogre", "Ogre"),
    ("fx_autotune", "Autotune"),
    ("fx_echo", "Echo"),
    ("fx_radio", "Radio"),
    ("fx_alien", "Alien"),
    ("voice_on", "Voice effect: {name}"),
    ("save_as_sound", "Save as sound"),
    ("save_sound_title", "Save clip as sound"),
    ("sound_name", "Sound name:"),
    ("sound_saved", "Sound saved and added: {name}"),
    ("sound_save_error", "Error saving sound: {e}"),
    ("sounds_added", "Sounds added: {n}"),
    ("new_collection", "New collection"),
    ("add_here", "Sound"),
    ("no_collection", "(no collection)"),
    ("ungrouped", "No collection"),
    ("collection_name", "Collection name:"),
    ("new_collection_title", "New collection"),
    ("rename_collection_title", "Rename collection {name}"),
    ("collection_empty", "The collection name can't be empty."),
    ("collection_exists", "There is already a collection called {name}."),
    ("collection_created", "Collection created: {name}"),
    ("collection_renamed", "Collection renamed: {old} → {name}"),
    ("collection_deleted", "Collection deleted: {name}. Its sounds moved to No collection."),
    ("moved_to", "{sound} → {name}"),
    ("config_imported", "Settings from the previous version imported from {path}"),
    ("data_folder", "Data folder"),
    ("tip_up", "Move up"),
    ("tip_down", "Move down"),
    ("tip_collection", "Collection of this sound"),
    ("tip_collapse", "Collapse / expand"),
    ("tip_add_here", "Add sounds to this collection"),
    ("tip_rename_collection", "Rename the collection"),
    ("tip_delete_collection", "Delete the collection (its sounds move to No collection)"),
    ("tip_data_folder", "Open {path}"),
    ("neon_color", "Neon color"),
    ("accent_set", "Neon color: {name}"),
    ("accent_green", "Green"),
    ("accent_cyan", "Cyan"),
    ("accent_blue", "Blue"),
    ("accent_purple", "Purple"),
    ("accent_pink", "Pink"),
    ("accent_red", "Red"),
    ("accent_orange", "Orange"),
    ("accent_yellow", "Yellow"),
    ("accent_sunset", "Sunset"),
    ("themes", "Themes"),
    ("tagline", "Soundboard straight into your mic"),
    ("devices", "Devices"),
    ("live", "Live"),
    ("offline", "Stopped"),
    ("clip_title", "PC audio clip"),
    ("file_missing", "File not found: {name}"),
    ("decode_failed", "unsupported format"),
    ("cancel", "Cancel"),
    ("save", "Save"),
    ("level_mic", "MIC"),
    ("level_out", "OUT"),
    ("tip_edit", "Change the key"),
    ("tip_clear", "Remove the key"),
    ("tip_file", "Choose file"),
    ("tip_play", "Play (restarts)"),
    ("tip_pause", "Pause / resume"),
    ("tip_stop", "Stop"),
    ("tip_remove", "Remove sound"),
    ("tip_volume", "Volume of this sound"),
    ("tip_record", "Save the last seconds of PC audio"),
];

/// Portugues se o Windows estiver em portugues; senao ingles.
pub fn detect() -> Lang {
    #[cfg(windows)]
    {
        // SAFETY: funcao sem parametros, so le a configuracao do usuario.
        let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        if id & 0x3FF == 0x16 {
            return Lang::Pt;
        }
    }
    Lang::En
}

/// `code` None ou desconhecido usa o idioma do sistema.
pub fn set_language(code: Option<&str>) {
    let lang = LANGUAGES.iter().find(|(_, c, _)| Some(*c) == code).map(|(l, _, _)| *l).unwrap_or_else(detect);
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::Pt as u8 { Lang::Pt } else { Lang::En }
}

pub fn code(lang: Lang) -> &'static str {
    LANGUAGES.iter().find(|(l, _, _)| *l == lang).map(|(_, c, _)| *c).unwrap_or("en")
}

pub fn display_name(lang: Lang) -> &'static str {
    LANGUAGES.iter().find(|(l, _, _)| *l == lang).map(|(_, _, n)| *n).unwrap_or("English")
}

/// Texto da chave no idioma atual (a propria chave se ela nao existir).
pub fn t(key: &'static str) -> &'static str {
    t_in(current(), key)
}

/// Nome traduzido de uma cor do neon (`key` de ui::theme::ACCENTS).
pub fn accent_name(key: &str) -> &'static str {
    match key {
        "cyan" => t("accent_cyan"),
        "blue" => t("accent_blue"),
        "purple" => t("accent_purple"),
        "pink" => t("accent_pink"),
        "red" => t("accent_red"),
        "orange" => t("accent_orange"),
        "yellow" => t("accent_yellow"),
        "sunset" => t("accent_sunset"),
        _ => t("accent_green"),
    }
}

/// Texto da chave num idioma especifico.
pub fn t_in(lang: Lang, key: &'static str) -> &'static str {
    let table = if lang == Lang::Pt { PT } else { EN };
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or(key)
}

/// Texto com campos: `tf("loaded", &[("name", nome)])`.
pub fn tf(key: &'static str, values: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut text = t(key).to_string();
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_keys_in_every_language() {
        let keys = |table: &[(&'static str, &'static str)]| -> Vec<&'static str> {
            let mut k: Vec<&str> = table.iter().map(|(k, _)| *k).collect();
            k.sort_unstable();
            k
        };
        assert_eq!(keys(PT), keys(EN));
    }

    #[test]
    fn same_fields_in_every_language() {
        let fields = |v: &str| {
            let mut f: Vec<String> =
                v.split('{').skip(1).filter_map(|s| s.split('}').next()).map(String::from).collect();
            f.sort();
            f
        };
        for (key, pt) in PT {
            let en = EN.iter().find(|(k, _)| k == key).unwrap().1;
            assert_eq!(fields(pt), fields(en), "{key}");
        }
    }
}
