#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;

use clap::Parser;
use eframe::egui;
use view_library::MainWindow;
#[derive(Parser, Debug)]
pub struct Cli {
    path: Option<PathBuf>,
}

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
            let gl = cc
                .gl
                .as_ref()
                .expect("You need to run eframe with the glow backend");
            let mut fd = MainWindow::new(gl, args.path);
            let cmd = fd.file_view.refresh();
            fd.handle_command(cmd);
            Box::new(fd)
        }),
    )
    .unwrap();
}
