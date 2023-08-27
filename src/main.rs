use std::path::PathBuf;

mod ui;
use clap::Parser;
use eframe::egui;
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
            let mut fd = ui::MainWindow::new(cc, args);
            fd.file_view.refresh();
            Box::new(fd)
        }),
    )
    .unwrap();
}
