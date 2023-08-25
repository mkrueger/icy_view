use eframe::{
    egui::{self, CentralPanel, Context, Response},
    epaint::{Color32, Vec2},
    App, Frame,
};

use egui::Ui;
use icy_engine::Buffer;
use icy_engine_egui::BufferView;

use std::{path::PathBuf, sync::Arc};

use self::file_view::{Command, FileView};

mod file_view;

pub struct MainWindow {
    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    pub file_view: FileView,
}

impl App for MainWindow {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        CentralPanel::default().show(ctx, |ui| {
            self.ui_in_window(ctx, ui);
        });
    }
}

impl MainWindow {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        let gl = cc
            .gl
            .as_ref()
            .expect("You need to run eframe with the glow backend");
        let mut view = BufferView::new(gl, glow::NEAREST as i32);
        view.caret.is_visible = false;

        Self {
            buffer_view: Arc::new(eframe::epaint::mutex::Mutex::new(view)),
            file_view: FileView::new(initial_path),
        }
    }

    fn ui_in_window(&mut self, ctx: &Context, ui: &mut Ui) {
        // Rows with files.

        egui::TopBottomPanel::bottom("bottom_panel")
            .default_height(400.)
            .resizable(true)
            .show_inside(ui, |ui| {
                let command = self.file_view.show_ui(ui);
                if let Some(command) = command {
                    match command {
                        Command::Select(file) => {
                            self.open_selected(file);
                            ctx.request_repaint();
                        }
                    };
                }
            });

        let frame_no_margins = egui::containers::Frame::none()
            .inner_margin(egui::style::Margin::same(0.0))
            .fill(Color32::BLACK);
        egui::CentralPanel::default()
            .frame(frame_no_margins)
            .show_inside(ui, |ui| self.custom_painting(ui));
    }

    fn custom_painting(&mut self, ui: &mut egui::Ui) -> Response {
        let opt = icy_engine_egui::TerminalOptions {
            focus_lock: true,
            stick_to_bottom: false,
            scale: Some(Vec2::new(2.0, 2.0)),
            ..Default::default()
        };
        let (response, _) = icy_engine_egui::show_terminal_area(ui, self.buffer_view.clone(), opt);
        response
    }

    fn open_selected(&mut self, file: usize) {
        let path = &self.file_view.files[file];
        if path.is_file() {
            if let Ok(buf) = Buffer::load_buffer(path, true) {
                self.buffer_view.lock().set_buffer(buf);
            }
        }
    }
}
