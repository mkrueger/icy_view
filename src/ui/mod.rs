use eframe::{
    egui::{self, Context, RichText, ScrollArea},
    epaint::{Color32, Vec2},
    App, Frame,
};

use egui_extras::RetainedImage;
use i18n_embed_fl::fl;
use icy_engine::Buffer;
use icy_engine_egui::BufferView;

use std::{io, sync::Arc, thread::JoinHandle, time::Duration};

use crate::Cli;

use self::file_view::{Command, FileEntry, FileView};

mod file_view;
mod help_dialog;
mod sauce_dialog;

pub struct MainWindow {
    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    pub file_view: FileView,
    pub start_time: std::time::Instant,
    pub in_scroll: bool,
    pub error_text: Option<String>,

    full_screen_mode: bool,
    loaded_buffer: bool,

    image_loading_thread: Option<JoinHandle<io::Result<RetainedImage>>>,
    retained_image: Option<RetainedImage>,

    sauce_dialog: Option<sauce_dialog::SauceDialog>,
    help_dialog: Option<help_dialog::HelpDialog>
}

const EXT_WHITE_LIST: [&str; 13] = [
    "bin", "xb", "adf", "idf", "tnd", "ans", "ice", "avt", "pcb", "seq", "asc", "diz", "nfo",
];

const EXT_BLACK_LIST: [&str; 8] = ["zip", "rar", "gz", "tar", "7z", "pdf", "exe", "com"];

impl App for MainWindow {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        egui::TopBottomPanel::bottom("bottom_panel")
            //   egui::SidePanel::left("left_panel")
            .min_height(300.)
            .resizable(true)
            .show(ctx, |ui| {
                let command = self.file_view.show_ui(ctx, ui);
                self.handle_command(command);
            });

        let frame_no_margins = egui::containers::Frame::none()
            .outer_margin(egui::style::Margin::same(0.0))
            .inner_margin(egui::style::Margin::same(0.0))
            .fill(Color32::BLACK);
        egui::CentralPanel::default()
            .frame(frame_no_margins)
            .show(ctx, |ui| self.paint_main_area(ui));
        self.in_scroll &= self.file_view.auto_scroll_enabled;
        if self.in_scroll {
            //   ctx.request_repaint_after(Duration::from_millis(10));
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(150));
        }

        if let Some(sauce_dialog) = &mut self.sauce_dialog {
            if let Some(message) = sauce_dialog.show(ctx) {
                match message {
                    sauce_dialog::Message::CloseDialog => {
                        self.sauce_dialog = None;
                    }
                }
            }
        }

        if let Some(help_dialog) = &mut self.help_dialog {
            if let Some(message) = help_dialog.show(ctx) {
                match message {
                    help_dialog::Message::CloseDialog => {
                        self.help_dialog = None;
                    }
                }
            }
        }

        if ctx.input(|i| {
            i.key_pressed(egui::Key::F11) || i.key_pressed(egui::Key::Enter) && i.modifiers.alt
        }) {
            self.full_screen_mode = !self.full_screen_mode;
            frame.set_fullscreen(self.full_screen_mode);
        }

        if ctx.input(|i| {
            i.key_pressed(egui::Key::Q) && i.modifiers.alt
        }) {
            frame.close();
        }
        if ctx.input(|i| {
            i.key_pressed(egui::Key::Escape)
        }) {
            if self.sauce_dialog.is_some() {
                self.sauce_dialog = None;
            } else if self.help_dialog.is_some() {
                self.help_dialog = None;
            } else {
                frame.close();
            }
        }
    }
}

impl MainWindow {
    pub fn new(cc: &eframe::CreationContext<'_>, cli: Cli) -> Self {
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
            file_view: FileView::new(cli.path),
            start_time: std::time::Instant::now(),
            in_scroll: false,
            image_loading_thread: None,
            retained_image: None,
            full_screen_mode: false,
            error_text: None,
            loaded_buffer: false,
            sauce_dialog: None,
            help_dialog: None
        }
    }

    fn paint_main_area(&mut self, ui: &mut egui::Ui) {
        if let Some(err) = &self.error_text {
            ui.colored_label(ui.style().visuals.error_fg_color, err);
            return;
        }

        if let Some(image_loading_thread) = &self.image_loading_thread {
            if image_loading_thread.is_finished() {
                if let Some(img) = self.image_loading_thread.take() {
                    match img.join() {
                        Ok(img) => match img {
                            Ok(img) => {
                                self.retained_image = Some(img);
                            }
                            Err(err) => {
                                self.error_text = Some(err.to_string());
                            }
                        },
                        Err(err) => {
                            self.error_text = Some(format!("{err:?}"));
                        }
                    }
                } else {
                    self.error_text =
                        Some(fl!(crate::LANGUAGE_LOADER, "error-never-happens").to_string());
                }
            } else {
                ui.centered_and_justified(|ui| ui.heading(fl!(crate::LANGUAGE_LOADER, "message-loading-image")));
            }
            return;
        }

        if let Some(img) = &self.retained_image {
            ScrollArea::both().show(ui, |ui| {
                img.show(ui);
            });
            return;
        }

        if self.loaded_buffer {
            let w = (ui.available_width() / 8.0).floor();
            let scale = (w / self.buffer_view.lock().buf.get_width() as f32).min(2.0);
            let sp = (self.start_time.elapsed().as_millis() as f32 / 6.0).round();
            let opt = icy_engine_egui::TerminalOptions {
                focus_lock: false,
                stick_to_bottom: false,
                scale: Some(Vec2::new(scale, scale)),
                font_extension: icy_engine_egui::FontExtension::Off,
                use_terminal_height: false,
                scroll_offset: if self.in_scroll { Some(sp) } else { None },
                ..Default::default()
            };

            let (_, calc) = icy_engine_egui::show_terminal_area(ui, self.buffer_view.clone(), opt);

            // stop scrolling when reached the end.
            if sp > calc.font_height * (calc.char_height - calc.buffer_char_height).max(0.0) {
                self.in_scroll = false;
            }

            self.in_scroll &= !calc.set_scroll_position_set_by_user;
        } else {
            match self.file_view.selected_file {
                Some(file) => {
                    if self.file_view.files[file].is_dir() {
                        return;
                    }
                    ui.add_space(ui.available_height() / 3.0);
                    ui.vertical_centered(|ui| {
                        ui.heading(fl!(crate::LANGUAGE_LOADER, "message-file-not-supported", name=self.file_view.files[self.file_view.selected_file.unwrap()]
                        .path
                        .file_name()
                        .unwrap()
                        .to_string_lossy())
                    );
                        ui.add_space(8.0);
                        if ui
                            .button(RichText::heading(fl!(crate::LANGUAGE_LOADER, "button-load-anyways").into()))
                            .clicked()
                        {
                            self.handle_command(Some(Command::Select(file, true)));
                        }
                    });
                }
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.heading(fl!(crate::LANGUAGE_LOADER, "message-empty"));
                    });
                }
            }
        }
    }

    fn open_selected(&mut self, file: usize) -> bool {
        if file >= self.file_view.files.len() {
            return false;
        }

        let open_path = if self.file_view.files[file].is_file() {
            if let Some(ext) = self.file_view.files[file].path.extension() {
                ext == "zip"
            } else {
                false
            }
        } else {
            true
        };

        if open_path {
            self.reset_state();
            self.file_view
                .set_path(self.file_view.files[file].path.clone());
        }

        open_path
    }
    fn view_selected(&mut self, file: usize, force_load: bool) {
        if file >= self.file_view.files.len() {
            return;
        }
        let entry = &self.file_view.files[file];
        if entry.is_file() {
            let ext = if let Some(ext) = entry.path.extension() {
                let ext2 = ext.to_ascii_lowercase();
                ext2.to_str().unwrap_or_default().to_string()
            } else {
                String::new()
            };
            if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "gif" || ext == "bmp" {
                self.image_loading_thread = Some(entry.read_image(|path, data| {
                    egui_extras::RetainedImage::from_image_bytes(path.to_string_lossy(), data)
                }));
                return;
            }
            if ext == "svg" {
                self.image_loading_thread = Some(entry.read_image(|path, data| {
                    egui_extras::RetainedImage::from_svg_bytes(path.to_string_lossy(), data)
                }));
                return;
            }
            if force_load
                || EXT_WHITE_LIST.contains(&ext.as_str())
                || !EXT_BLACK_LIST.contains(&ext.as_str()) && !is_binary(entry)
            {
                match entry.get_data(|path, data| Buffer::from_bytes(path, true, data)) {
                    Ok(buf) => match buf {
                        Ok(buf) => {
                            self.buffer_view.lock().set_buffer(buf);
                            self.loaded_buffer = true;
                            self.start_time = std::time::Instant::now();
                            self.in_scroll = true;
                        }
                        Err(err) => self.error_text = Some(err.to_string()),
                    },
                    Err(err) => self.error_text = Some(err.to_string()),
                }
            }
        }
    }

    fn reset_state(&mut self) {
        self.image_loading_thread = None;
        self.retained_image = None;
        self.error_text = None;
        self.loaded_buffer = false;
        self.file_view.selected_file = None;
    }

    pub fn handle_command(&mut self, command: Option<Command>) {
        if let Some(command) = command {
            match command {
                Command::Select(file, fore_load) => {
                    if self.file_view.selected_file != Some(file) || fore_load {
                        self.reset_state();
                        if file < self.file_view.files.len() {
                            self.file_view.selected_file = Some(file);
                            self.file_view.scroll_pos = Some(file);
                            self.view_selected(file, fore_load);
                        }
                    }
                }
                Command::Refresh => {
                    self.reset_state();
                    self.file_view.refresh();
                }
                Command::Open(file) => {
                    if self.open_selected(file) && !self.file_view.files.is_empty() {
                        self.file_view.selected_file = Some(0);
                        self.file_view.scroll_pos = Some(0);
                        self.view_selected(file, false);
                    }
                }
                Command::ParentFolder => {
                    let mut p = self.file_view.get_path();
                    if p.pop() {
                        self.reset_state();
                        self.file_view.set_path(p);
                        self.handle_command(Some(Command::Select(0, false)));
                    }
                }
                Command::ToggleAutoScroll => {
                    self.file_view.auto_scroll_enabled = !self.file_view.auto_scroll_enabled;
//                    self.in_scroll = self.file_view.auto_scroll_enabled;
                }
                Command::ShowSauce(file) => {
                    if file < self.file_view.files.len() {
                        if let Some(sauce) = self.file_view.files[file].get_sauce() {
                            self.sauce_dialog = Some(sauce_dialog::SauceDialog::new(sauce));
                        }
                    }
                }
                Command::ShowHelpDialog => {
                    self.help_dialog = Some(help_dialog::HelpDialog::new());
                }
            };
        }
    }
}

fn is_binary(file_entry: &FileEntry) -> bool {
    file_entry
        .get_data(|_, data| {
            for i in data.iter().take(500) {
                if i == &0 || i == &255 {
                    return true;
                }
            }
            false
        })
        .unwrap()
}
