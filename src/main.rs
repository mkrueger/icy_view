use std::path::PathBuf;

mod ui;
use clap::Parser;
use eframe::epaint::Vec2;
#[derive(Parser, Debug)]
pub struct Cli {
    path: Option<PathBuf>,
}

#[allow(clippy::field_reassign_with_default)]
fn main() {
    let args = Cli::parse();

    let mut options = eframe::NativeOptions::default();
    options.initial_window_size = Some(Vec2::new(1416., 807.));
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
