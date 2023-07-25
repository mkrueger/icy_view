mod ui;

fn main() {
    eframe::run_native(
        &format!("iCY VIEW {}", env!("CARGO_PKG_VERSION")),
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            let mut fd = ui::MainWindow::new(cc, None);
            fd.refresh();
            Box::new(fd)
        }),
    )
    .unwrap();
}
