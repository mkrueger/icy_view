use eframe::epaint::Vec2;

mod ui;

fn main() {
    let mut options = eframe::NativeOptions::default();
    options.initial_window_size = Some(Vec2::new(1416., 807.));
    eframe::run_native(
        &format!("iCY VIEW {}", env!("CARGO_PKG_VERSION")),
        options,
        Box::new(|cc| {
            let mut fd = ui::MainWindow::new(cc, None);
            fd.file_view.refresh();
            Box::new(fd)
        }),
    )
    .unwrap();
}
