//! Hotkeys globais: funcionam com o app em segundo plano ou minimizado.
//!
//! Usa um gancho de teclado de baixo nivel (WH_KEYBOARD_LL) que apenas observa: a tecla continua
//! chegando ao jogo/aplicativo em foco. O gancho so repassa (tecla, apertou?) para uma thread de
//! trabalho, que monta a combinacao ("ctrl+shift+a", "f1"), ignora a repeticao automatica e
//! dispara a acao registrada. O gancho tem que responder rapido, senao o Windows o remove.
//!
//! Teclas sao guardadas como texto em minusculas, modificadores na ordem ctrl, shift, alt, win.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Slot(u64),
    SaveClip,
    PlayClip,
    StopAll,
}

#[derive(Default)]
pub struct Hotkeys {
    bindings: Mutex<HashMap<String, Vec<Action>>>,
    capture: Mutex<Option<mpsc::Sender<Option<String>>>>,
}

const MODIFIERS: [(&str, &[u32]); 4] =
    [("ctrl", &[0xA2, 0xA3, 0x11]), ("shift", &[0xA0, 0xA1, 0x10]), ("alt", &[0xA4, 0xA5, 0x12]), ("win", &[0x5B, 0x5C])];
const VK_ESCAPE: u32 = 0x1B;

fn modifier_of(vk: u32) -> Option<&'static str> {
    MODIFIERS.iter().find(|(_, vks)| vks.contains(&vk)).map(|(name, _)| *name)
}

/// Nome estavel de uma tecla (virtual-key code do Windows).
pub fn vk_name(vk: u32) -> String {
    let fixed = match vk {
        0x08 => "backspace",
        0x09 => "tab",
        0x0D => "enter",
        0x13 => "pause",
        0x14 => "caps lock",
        0x1B => "esc",
        0x20 => "space",
        0x21 => "page up",
        0x22 => "page down",
        0x23 => "end",
        0x24 => "home",
        0x25 => "left",
        0x26 => "up",
        0x27 => "right",
        0x28 => "down",
        0x2C => "print screen",
        0x2D => "insert",
        0x2E => "delete",
        0x6A => "num *",
        0x6B => "num plus",
        0x6D => "num -",
        0x6E => "num .",
        0x6F => "num /",
        0x90 => "num lock",
        0x91 => "scroll lock",
        0xAD => "volume mute",
        0xAE => "volume down",
        0xAF => "volume up",
        0xB0 => "next track",
        0xB1 => "previous track",
        0xB2 => "stop media",
        0xB3 => "play/pause media",
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xC1 => "abnt /",
        0xC2 => "abnt .",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
        0xE2 => "<",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_string();
    }
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).unwrap().to_ascii_lowercase().to_string(),
        0x60..=0x69 => format!("num {}", vk - 0x60),
        0x70..=0x87 => format!("f{}", vk - 0x6F),
        _ => match modifier_of(vk) {
            Some(m) => m.to_string(),
            None => format!("vk{vk}"),
        },
    }
}

fn is_key_name(name: &str) -> bool {
    if let Some(n) = name.strip_prefix("vk") {
        if n.parse::<u32>().is_ok_and(|v| (1..255).contains(&v)) {
            return true;
        }
    }
    (1..255u32).any(|vk| modifier_of(vk).is_none() && vk_name(vk) == name)
}

/// Normaliza uma combinacao digitada/salva. None se for invalida.
pub fn normalize(combo: &str) -> Option<String> {
    let mut mods: Vec<&str> = Vec::new();
    let mut key: Option<String> = None;
    for part in combo.split('+') {
        let p = part.trim().to_lowercase();
        let p = match p.as_str() {
            "control" | "left ctrl" | "right ctrl" | "ctl" => "ctrl".to_string(),
            "left shift" | "right shift" => "shift".to_string(),
            "alt gr" | "left alt" | "right alt" | "menu" => "alt".to_string(),
            "windows" | "left windows" | "right windows" | "cmd" | "super" => "win".to_string(),
            "escape" => "esc".to_string(),
            "return" => "enter".to_string(),
            "spacebar" => "space".to_string(),
            "pgup" | "page_up" | "prior" => "page up".to_string(),
            "pgdn" | "page_down" | "next" => "page down".to_string(),
            "del" => "delete".to_string(),
            "ins" => "insert".to_string(),
            _ => p,
        };
        if p.is_empty() {
            return None;
        }
        if let Some(m) = MODIFIERS.iter().map(|(n, _)| *n).find(|n| *n == p) {
            if !mods.contains(&m) {
                mods.push(m);
            }
        } else if key.is_none() && is_key_name(&p) {
            key = Some(p);
        } else {
            return None;
        }
    }
    let mut parts: Vec<String> =
        MODIFIERS.iter().map(|(n, _)| *n).filter(|n| mods.contains(n)).map(String::from).collect();
    match key {
        Some(k) => parts.push(k),
        // so modificador (ex.: "ctrl") tambem vale como tecla
        None if parts.len() == 1 => {}
        None => return None,
    }
    Some(parts.join("+"))
}

/// Texto para mostrar na interface: "CTRL+F1".
pub fn display(combo: &str) -> String {
    combo.to_uppercase()
}

impl Hotkeys {
    /// Instala o gancho e a thread de trabalho. `handler` roda na thread de trabalho.
    pub fn start(handler: impl Fn(Action) + Send + 'static) -> Arc<Self> {
        let hk = Arc::new(Self::default());
        let (tx, rx) = mpsc::channel::<(u32, bool)>();
        if HOOK_TX.set(tx).is_ok() {
            hook::install();
        }
        let worker = hk.clone();
        std::thread::Builder::new()
            .name("hotkeys".into())
            .spawn(move || worker.run(rx, handler))
            .expect("hotkey thread");
        hk
    }

    /// Registra (tecla, acao). Teclas vazias sao ignoradas; invalidas voltam como erro.
    pub fn set_bindings(&self, list: &[(String, Action)]) -> Vec<String> {
        let mut map: HashMap<String, Vec<Action>> = HashMap::new();
        let mut errors = Vec::new();
        for (key, action) in list {
            if key.trim().is_empty() {
                continue;
            }
            match normalize(key) {
                Some(k) => map.entry(k).or_default().push(*action),
                None => errors.push(key.clone()),
            }
        }
        *self.bindings.lock() = map;
        errors
    }

    /// A proxima tecla apertada e devolvida no canal (None = Esc, cancelado), sem disparar acoes.
    pub fn begin_capture(&self) -> mpsc::Receiver<Option<String>> {
        let (tx, rx) = mpsc::channel();
        *self.capture.lock() = Some(tx);
        rx
    }

    pub fn cancel_capture(&self) {
        if let Some(tx) = self.capture.lock().take() {
            let _ = tx.send(None);
        }
    }

    fn run(&self, rx: mpsc::Receiver<(u32, bool)>, handler: impl Fn(Action)) {
        let mut down: HashSet<u32> = HashSet::new();
        for (vk, is_down) in rx {
            if !is_down {
                down.remove(&vk);
                continue;
            }
            if !down.insert(vk) {
                continue; // repeticao automatica de tecla segurada
            }
            // um soltar perdido (ex.: Ctrl+Alt+Del) deixaria um modificador "preso"
            down.retain(|&k| k == vk || modifier_of(k).is_none() || hook::is_pressed(k));
            let held: Vec<&str> = MODIFIERS
                .iter()
                .filter(|(_, vks)| vks.iter().any(|v| down.contains(v) && *v != vk))
                .map(|(n, _)| *n)
                .collect();
            let combo = match modifier_of(vk) {
                Some(m) if held.is_empty() => m.to_string(),
                Some(_) => continue, // modificador apertado junto com outros: espera a tecla
                None => {
                    let mut parts: Vec<String> = held.iter().map(|s| s.to_string()).collect();
                    parts.push(vk_name(vk));
                    parts.join("+")
                }
            };
            // o lock e solto antes de devolver o canal (dentro do `if let` ele ainda estaria preso)
            let capture = self.capture.lock().take();
            if let Some(tx) = capture {
                if modifier_of(vk).is_some() {
                    // so um modificador: continua esperando a tecla de verdade
                    *self.capture.lock() = Some(tx);
                    continue;
                }
                let _ = tx.send(if vk == VK_ESCAPE && held.is_empty() { None } else { Some(combo) });
                continue;
            }
            if debug() {
                eprintln!("hotkeys: {combo}");
            }
            let actions = self.bindings.lock().get(&combo).cloned();
            for action in actions.unwrap_or_default() {
                handler(action);
            }
        }
    }
}

static HOOK_TX: OnceLock<mpsc::Sender<(u32, bool)>> = OnceLock::new();

/// `SOUNDMOD_DEBUG_KEYS=1` mostra no stderr o que o gancho recebe (diagnostico).
fn debug() -> bool {
    static DEBUG: OnceLock<bool> = OnceLock::new();
    *DEBUG.get_or_init(|| std::env::var_os("SOUNDMOD_DEBUG_KEYS").is_some())
}

#[cfg(windows)]
mod hook {
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG, SetWindowsHookExW,
        TranslateMessage, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    use super::HOOK_TX;

    unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            // SAFETY: com HC_ACTION, lparam aponta para um KBDLLHOOKSTRUCT valido.
            let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            let msg = wparam as u32;
            if super::debug() {
                eprintln!("hook: vk {:#x} msg {msg:#x} flags {:#x}", info.vkCode, info.flags);
            }
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            if down || msg == WM_KEYUP || msg == WM_SYSKEYUP {
                if let Some(tx) = HOOK_TX.get() {
                    let _ = tx.send((info.vkCode, down));
                }
            }
        }
        // SAFETY: repassa o evento para os proximos ganchos, como exigido.
        unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
    }

    pub fn install() {
        std::thread::Builder::new()
            .name("keyboard-hook".into())
            .spawn(|| unsafe {
                // SAFETY: chamadas Win32 na propria thread, que mantem o laco de mensagens que o
                // gancho de baixo nivel exige.
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), GetModuleHandleW(null()), 0);
                if super::debug() {
                    eprintln!("hotkeys: gancho {hook:?}");
                }
                if hook.is_null() {
                    eprintln!("hotkeys: SetWindowsHookExW falhou");
                    return;
                }
                let mut msg: MSG = std::mem::zeroed();
                loop {
                    let r = GetMessageW(&mut msg, null_mut(), 0, 0);
                    if r <= 0 {
                        if super::debug() {
                            eprintln!("hotkeys: GetMessageW = {r}, laco encerrado");
                        }
                        break;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .expect("hook thread");
    }

    pub fn is_pressed(vk: u32) -> bool {
        // SAFETY: so le o estado global do teclado.
        (unsafe { GetAsyncKeyState(vk as i32) } as u16) & 0x8000 != 0
    }
}

#[cfg(not(windows))]
mod hook {
    pub fn install() {}
    pub fn is_pressed(_vk: u32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_combos() {
        assert_eq!(normalize("F1").as_deref(), Some("f1"));
        assert_eq!(normalize("shift + Control+a").as_deref(), Some("ctrl+shift+a"));
        assert_eq!(normalize("ctrl").as_deref(), Some("ctrl"));
        assert_eq!(normalize("num 5").as_deref(), Some("num 5"));
        assert_eq!(normalize("page_up").as_deref(), Some("page up"));
        assert_eq!(normalize("a+b"), None);
        assert_eq!(normalize("ctrl+shift"), None);
        assert_eq!(normalize("banana"), None);
    }

    #[test]
    fn names_roundtrip() {
        for vk in 1..255u32 {
            if modifier_of(vk).is_some() {
                continue;
            }
            let name = vk_name(vk);
            assert_eq!(normalize(&name).as_deref(), Some(name.as_str()), "vk {vk}");
        }
    }

    #[test]
    fn invalid_bindings_are_reported() {
        let hk = Hotkeys::default();
        let errors = hk.set_bindings(&[("f1".into(), Action::StopAll), ("".into(), Action::SaveClip), ("??x".into(), Action::PlayClip)]);
        assert_eq!(errors, vec!["??x".to_string()]);
        assert!(hk.bindings.lock().contains_key("f1"));
    }
}
