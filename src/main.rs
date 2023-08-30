#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;

mod ui;
use clap::Parser;
use eframe::egui;
#[derive(Parser, Debug)]
pub struct Cli {
    path: Option<PathBuf>,
}

use rust_embed::RustEmbed;
#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};

use once_cell::sync::Lazy;
pub static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

fn main() {
    let args = Cli::parse();

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1284. + 8., 839.)),
        multisampling: 0,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        &format!("iCY VIEW {}", env!("CARGO_PKG_VERSION")),
        options,
        Box::new(|cc| {
            let mut fd = ui::MainWindow::new(cc, args);
            let cmd = fd.file_view.refresh();
            fd.handle_command(cmd);
            Box::new(fd)
        }),
    )
    .unwrap();
}
