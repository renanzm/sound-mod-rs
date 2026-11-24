//! Sound Mod: soundboard para Windows que toca sons direto no canal do microfone.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod codec;
mod config;
mod engine;
mod hotkeys;
mod i18n;
mod icon_raster;
mod library;
mod logic;
mod recorder;
mod ui;
mod voice;

use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result {
    install_panic_log();
    // o icone da janela usa a cor do neon salva (o SoundApp le a config completa depois)
    ui::theme::set_accent(&config::peek_accent().unwrap_or_default());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Sound Mod")
            .with_inner_size(initial_size())
            .with_min_inner_size([ui::app::MIN_WIDTH, 760.0])
            .with_icon(Arc::new(ui::logo::icon_data())),
        ..Default::default()
    };
    eframe::run_native("Sound Mod", options, Box::new(|cc| Ok(Box::new(ui::app::SoundApp::new(cc)))))
}

/// 1440x900, ou menos se a area de trabalho for menor (ex.: monitor em pe com 1080 px de
/// largura). Antes do eframe o processo ainda nao e DPI-aware, entao o Windows ja devolve a
/// area em pixels logicos, a mesma unidade do egui.
fn initial_size() -> [f32; 2] {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETWORKAREA, SystemParametersInfoW};
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: SPI_GETWORKAREA escreve um RECT no ponteiro passado
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&raw mut r).cast(), 0) } != 0;
    let (w, h) = if ok { ((r.right - r.left) as f32, (r.bottom - r.top) as f32) } else { (f32::MAX, f32::MAX) };
    // folga para a borda e a barra de titulo da janela
    [1440f32.min(w - 24.0).max(ui::app::MIN_WIDTH), 900f32.min(h - 60.0).max(760.0)]
}

/// Sem console no .exe: um erro fatal vai para soundmod-error.log na pasta de dados.
fn install_panic_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = format!("{info}\n{}\n", std::backtrace::Backtrace::force_capture());
        let _ = std::fs::create_dir_all(config::data_dir());
        let _ = std::fs::write(config::data_dir().join("soundmod-error.log"), msg);
        default(info);
    }));
}
