//! Ajustes da janela nativa no Windows 11: barra de titulo grafite e borda verde neon.

pub fn style(cc: &eframe::CreationContext<'_>) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = cc.window_handle() {
            if let RawWindowHandle::Win32(w) = handle.as_raw() {
                apply(w.hwnd.get() as windows_sys::Win32::Foundation::HWND);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = cc;
}

#[cfg(windows)]
fn apply(hwnd: windows_sys::Win32::Foundation::HWND) {
    use super::theme::{BG, NEON, NEON_CORE};
    use eframe::egui::Color32;
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    const DARK_MODE: u32 = 20; // DWMWA_USE_IMMERSIVE_DARK_MODE
    const BORDER_COLOR: u32 = 34; // DWMWA_BORDER_COLOR (Windows 11)
    const CAPTION_COLOR: u32 = 35; // DWMWA_CAPTION_COLOR (Windows 11)
    const TEXT_COLOR: u32 = 36; // DWMWA_TEXT_COLOR (Windows 11)
    let colorref = |c: Color32| c.r() as u32 | (c.g() as u32) << 8 | (c.b() as u32) << 16;
    for (attr, value) in [(DARK_MODE, 1), (CAPTION_COLOR, colorref(BG)), (BORDER_COLOR, colorref(NEON)), (TEXT_COLOR, colorref(NEON_CORE))] {
        // SAFETY: hwnd e a janela do app; o valor e um u32 vivo durante a chamada. No Windows 10
        // os atributos 34-36 nao existem e a chamada so falha, sem efeito.
        unsafe {
            DwmSetWindowAttribute(hwnd, attr, &value as *const u32 as *const core::ffi::c_void, 4);
        }
    }
}
