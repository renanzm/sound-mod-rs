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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Sound Mod")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1240.0, 760.0])
            .with_icon(Arc::new(ui::logo::icon_data())),
        ..Default::default()
    };
    eframe::run_native("Sound Mod", options, Box::new(|cc| Ok(Box::new(ui::app::SoundApp::new(cc)))))
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
