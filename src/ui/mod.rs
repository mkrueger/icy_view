use eframe::{
    egui::{self, CentralPanel, Context, Response},
    epaint::{Color32, Vec2},
    App, Frame,
};

use egui::Ui;
use icy_engine::Buffer;
use icy_engine_egui::BufferView;

use std::{path::PathBuf, sync::Arc, time::Duration};

use self::file_view::{Command, FileView};

mod file_view;

pub struct MainWindow {
    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    pub file_view: FileView,
    pub start_time: std::time::Instant,
    pub in_scroll: bool,
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
        let mut view = BufferView::new(
            gl,
            glow::NEAREST as i32,
            icy_engine_egui::FontExtension::Off,
        );
        view.caret.is_visible = false;

        Self {
            buffer_view: Arc::new(eframe::epaint::mutex::Mutex::new(view)),
            file_view: FileView::new(initial_path),
            start_time: std::time::Instant::now(),
            in_scroll: false,
        }
    }

    fn ui_in_window(&mut self, ctx: &Context, ui: &mut Ui) {
        // Rows with files.

        egui::TopBottomPanel::bottom("bottom_panel")
            //   egui::SidePanel::left("left_panel")
            .min_height(300.)
            .resizable(true)
            .show_inside(ui, |ui| {
                let command = self.file_view.show_ui(ui);
                if let Some(command) = command {
                    match command {
                        Command::Select(file) => {
                            self.file_view.selected_file = Some(file);
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
        if self.in_scroll {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }

    fn custom_painting(&mut self, ui: &mut egui::Ui) -> Response {
        let sp = (self.start_time.elapsed().as_millis() as f32 / 6.0).floor();
        let opt = icy_engine_egui::TerminalOptions {
            focus_lock: true,
            stick_to_bottom: false,
            scale: Some(Vec2::new(2.0, 2.0)),
            font_extension: icy_engine_egui::FontExtension::Off,
            use_terminal_height: false,
            scroll_offset: if self.in_scroll { Some(sp) } else { None },
            ..Default::default()
        };
        let (response, calc) =
            icy_engine_egui::show_terminal_area(ui, self.buffer_view.clone(), opt);

        // stop scrolling when reached the end.
        if sp > calc.font_height * (calc.char_height - calc.buffer_char_height).max(0.0) {
            self.in_scroll = false;
        }

        self.in_scroll &= !calc.set_scroll_position_set_by_user;
        response
    }

    fn open_selected(&mut self, file: usize) {
        let entry = &self.file_view.files[file];
        if entry.path.is_file() {
            if let Ok(buf) = Buffer::load_buffer(&entry.path, true) {
                self.start_time = std::time::Instant::now();
                self.in_scroll = true;
                self.buffer_view.lock().set_buffer(buf);
            }
        }
    }
}
