use eframe::{
    egui::{self, Context, CursorIcon, RichText, ScrollArea},
    epaint::{mutex::Mutex, Color32, Vec2},
    App, Frame,
};

use egui_extras::RetainedImage;
use i18n_embed_fl::fl;
use icy_engine::Buffer;
use icy_engine_egui::{animations::Animator, BufferView, MonitorSettings};

use std::{env::current_dir, io, path::PathBuf, sync::Arc, thread::JoinHandle, time::Duration};

use self::file_view::{FileEntry, FileView, Message};

mod file_view;
mod help_dialog;
mod sauce_dialog;

pub struct MainWindow {
    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    pub file_view: FileView,
    pub in_scroll: bool,
    cur_scroll_pos: f32,
    drag_vel: f32,
    key_vel: f32,
    drag_started: bool,

    pub error_text: Option<String>,

    full_screen_mode: bool,
    loaded_buffer: bool,

    image_loading_thread: Option<JoinHandle<io::Result<RetainedImage>>>,
    retained_image: Option<RetainedImage>,

    sauce_dialog: Option<sauce_dialog::SauceDialog>,
    help_dialog: Option<help_dialog::HelpDialog>,

    toasts: egui_notify::Toasts,
    is_closed: bool,
    pub opened_file: Option<FileEntry>,

    // animations
    animation: Option<Arc<Mutex<Animator>>>,
}
const SCROLL_SPEED: [f32; 3] = [80.0, 160.0, 320.0];
const EXT_WHITE_LIST: [&str; 5] = ["seq", "diz", "nfo", "ice", "bbs"];

const EXT_BLACK_LIST: [&str; 8] = ["zip", "rar", "gz", "tar", "7z", "pdf", "exe", "com"];

impl App for MainWindow {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        egui::SidePanel::left("bottom_panel")
            .default_width(ctx.available_rect().width() * 3.0 / 2.0)
            .exact_width(250.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.set_enabled(self.sauce_dialog.is_none() && self.help_dialog.is_none());
                let command = self.file_view.show_ui(ui, false);
                self.handle_command(command);
            });
        let frame_no_margins = egui::containers::Frame::none()
            .outer_margin(egui::style::Margin::same(0.0))
            .inner_margin(egui::style::Margin::same(0.0))
            .fill(Color32::BLACK);
        egui::CentralPanel::default()
            .frame(frame_no_margins)
            .show(ctx, |ui| {
                ui.set_enabled(self.sauce_dialog.is_none() && self.help_dialog.is_none());
                self.paint_main_area(ui)
            });
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

        self.toasts.show(ctx);

        if ctx.input(|i| {
            i.key_pressed(egui::Key::F11) || i.key_pressed(egui::Key::Enter) && i.modifiers.alt
        }) {
            self.full_screen_mode = !self.full_screen_mode;
            frame.set_fullscreen(self.full_screen_mode);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Q) && i.modifiers.alt) {
            frame.close();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
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
    pub fn new(gl: &Arc<glow::Context>, mut initial_path: Option<PathBuf>) -> Self {
        let mut view = BufferView::new(gl, glow::NEAREST as i32);
        view.interactive = false;
        view.get_buffer_mut().is_terminal_buffer = false;
        view.get_caret_mut().is_visible = false;
        if let Some(path) = &initial_path {
            if path.is_relative() {
                if let Ok(cur) = current_dir() {
                    initial_path = Some(cur.join(path));
                }
            }
        }
        Self {
            buffer_view: Arc::new(eframe::epaint::mutex::Mutex::new(view)),
            file_view: FileView::new(initial_path),
            in_scroll: false,
            image_loading_thread: None,
            retained_image: None,
            full_screen_mode: false,
            error_text: None,
            loaded_buffer: false,
            sauce_dialog: None,
            help_dialog: None,
            drag_started: false,
            cur_scroll_pos: 0.0,
            drag_vel: 0.0,
            key_vel: 0.0,
            toasts: egui_notify::Toasts::default(),
            opened_file: None,
            is_closed: false,
            animation: None,
        }
    }

    pub fn show_file_chooser(&mut self, ctx: &Context) -> bool {
        self.is_closed = false;
        self.opened_file = None;
        egui::SidePanel::left("bottom_panel")
            .default_width(ctx.available_rect().width() * 3.0 / 2.0)
            .exact_width(250.0)
            .resizable(true)
            .show(ctx, |ui| {
                let command = self.file_view.show_ui(ui, true);
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

        self.toasts.show(ctx);
        self.is_closed
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
                ui.centered_and_justified(|ui| {
                    ui.heading(fl!(crate::LANGUAGE_LOADER, "message-loading-image"))
                });
            }
            return;
        }

        if let Some(img) = &self.retained_image {
            ScrollArea::both().show(ui, |ui| {
                img.show(ui);
            });
            return;
        }

        if let Some(anim) = &self.animation {
            let settings = anim.lock().update_frame(self.buffer_view.clone());
            let (_, _) = self.show_buffer_view(ui, settings);
            return;
        }

        if self.loaded_buffer {
            let (response, calc) = self.show_buffer_view(ui, MonitorSettings::default());

            // stop scrolling when reached the end.
            if self.in_scroll {
                let last_scroll_pos =
                    calc.char_height - calc.buffer_char_height + calc.scroll_remainder_y;
                if last_scroll_pos <= calc.char_scroll_positon.y / calc.font_height {
                    self.in_scroll = false;
                }
            }
            self.cur_scroll_pos = calc.char_scroll_positon.y;

            if ui.input(|i: &egui::InputState| i.key_pressed(egui::Key::Home) && i.modifiers.ctrl) {
                self.cur_scroll_pos = 0.0;
                self.in_scroll = false;
            }

            if ui.input(|i| i.key_pressed(egui::Key::End) && i.modifiers.ctrl) {
                self.cur_scroll_pos = f32::MAX;
                self.in_scroll = false;
            }

            if ui
                .input(|i: &egui::InputState| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl)
            {
                self.key_vel = 500.0;
                self.in_scroll = false;
            }

            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.ctrl) {
                self.key_vel -= 250.0;
                self.in_scroll = false;
            }

            if ui.input(|i: &egui::InputState| i.key_pressed(egui::Key::PageUp) && i.modifiers.ctrl)
            {
                self.key_vel = 5000.0;
                self.in_scroll = false;
            }

            if ui.input(|i| i.key_pressed(egui::Key::PageDown) && i.modifiers.ctrl) {
                self.key_vel -= 2500.0;
                self.in_scroll = false;
            }

            if (self.key_vel - 0.1).abs() > 0.1 {
                let friction_coeff = 10.0;
                let dt = ui.input(|i| i.unstable_dt);
                let friction = friction_coeff * dt;
                self.key_vel -= friction * self.key_vel;
                self.cur_scroll_pos -= self.key_vel * dt;
                ui.ctx().request_repaint();
            }

            if response.drag_started_by(egui::PointerButton::Primary) {
                self.drag_started = false;
                if let Some(mouse_pos) = response.interact_pointer_pos() {
                    if !calc.vert_scrollbar_rect.contains(mouse_pos)
                        && !calc.horiz_scrollbar_rect.contains(mouse_pos)
                    {
                        self.drag_started = true;
                        ui.output_mut(|o| o.cursor_icon = CursorIcon::Grab);
                    }
                }
            }
            if response.drag_released_by(egui::PointerButton::Primary) {
                self.drag_started = false;
            }
            if response.dragged_by(egui::PointerButton::Primary) && self.drag_started {
                ui.input(|input| {
                    self.cur_scroll_pos -= input.pointer.delta().y;
                    self.drag_vel = input.pointer.velocity().y;
                    self.key_vel = 0.0;
                    self.in_scroll = false;
                });
                ui.output_mut(|o| o.cursor_icon = CursorIcon::Grab);
            } else {
                let friction_coeff = 10.0;
                let dt = ui.input(|i| i.unstable_dt);
                let friction = friction_coeff * dt;
                self.drag_vel -= friction * self.drag_vel;
                self.cur_scroll_pos -= self.drag_vel * dt;
                ui.ctx().request_repaint();
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
                        if let Some(idx) = self.file_view.selected_file {
                            if let Some(file_name) = self.file_view.files[idx].path.file_name() {
                                ui.heading(fl!(
                                    crate::LANGUAGE_LOADER,
                                    "message-file-not-supported",
                                    name = file_name.to_string_lossy()
                                ));
                            }
                        }

                        ui.add_space(8.0);
                        if ui
                            .button(RichText::heading(
                                fl!(crate::LANGUAGE_LOADER, "button-load-anyways").into(),
                            ))
                            .clicked()
                        {
                            self.handle_command(Some(Message::Select(file, true)));
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

    fn show_buffer_view(
        &mut self,
        ui: &mut egui::Ui,
        monitor_settings: MonitorSettings,
    ) -> (egui::Response, icy_engine_egui::TerminalCalc) {
        let w = (ui.available_width() / 8.0).floor();
        let scalex = (w / self.buffer_view.lock().get_width() as f32).min(2.0);
        let scaley = if self.buffer_view.lock().get_buffer_mut().use_aspect_ratio() {
            scalex * 1.35
        } else {
            scalex
        };

        let dt = ui.input(|i| i.unstable_dt);
        let sp = if self.in_scroll {
            (self.cur_scroll_pos + SCROLL_SPEED[self.file_view.scroll_speed] * dt).round()
        } else {
            self.cur_scroll_pos.round()
        };
        let opt = icy_engine_egui::TerminalOptions {
            stick_to_bottom: false,
            scale: Some(Vec2::new(scalex, scaley)),
            use_terminal_height: false,
            scroll_offset_y: Some(sp),
            monitor_settings,
            ..Default::default()
        };

        let (response, calc) =
            icy_engine_egui::show_terminal_area(ui, self.buffer_view.clone(), opt);
        (response, calc)
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
        self.animation = None;

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
            if ext == "icyanim" {
                let anim = entry.get_data(|path, data| match String::from_utf8(data.to_vec()) {
                    Ok(data) => {
                        let parent = path.parent().map(|path| path.to_path_buf());

                        match Animator::run(&parent, &data) {
                            Ok(anim) => {
                                anim.lock().set_is_loop(true);
                                anim.lock().set_is_playing(true);
                                Ok(anim)
                            }
                            Err(err) => {
                                log::error!("{err}");
                                Err(anyhow::anyhow!("{err}"))
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Error while parsing icyanim file: {err}");
                        Err(anyhow::anyhow!("Error while parsing icyanim file: {err}"))
                    }
                });
                match anim {
                    Ok(Ok(anim)) => {
                        anim.lock().start_playback(self.buffer_view.clone());
                        self.animation = Some(anim);
                        return;
                    }
                    Ok(Err(err)) | Err(err) => {
                        log::error!("Error while loading icyanim file: {err}");
                        self.error_text = Some(err.to_string())
                    }
                }
            }

            if force_load
                || EXT_WHITE_LIST.contains(&ext.as_str())
                || icy_engine::FORMATS
                    .iter()
                    .any(|f| f.get_file_extension().to_ascii_lowercase() == ext.as_str())
                || !EXT_BLACK_LIST.contains(&ext.as_str()) && !is_binary(entry)
            {
                match entry.get_data(|path, data| Buffer::from_bytes(path, true, data)) {
                    Ok(buf) => match buf {
                        Ok(buf) => {
                            self.buffer_view.lock().set_buffer(buf);
                            self.loaded_buffer = true;
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
        self.cur_scroll_pos = 0.0;
    }

    pub fn handle_command(&mut self, command: Option<Message>) {
        if let Some(command) = command {
            match command {
                Message::Select(file, fore_load) => {
                    if self.file_view.selected_file != Some(file) || fore_load {
                        self.reset_state();
                        if file < self.file_view.files.len() {
                            self.file_view.selected_file = Some(file);
                            self.file_view.scroll_pos = Some(file);
                            self.view_selected(file, fore_load);
                        }
                    }
                }
                Message::Refresh => {
                    self.reset_state();
                    self.file_view.refresh();
                }
                Message::Open(file) => {
                    self.is_closed = !self.open(file);
                }
                Message::Cancel => {
                    self.is_closed = true;
                }
                Message::ParentFolder => {
                    let mut p = self.file_view.get_path();
                    if p.pop() {
                        self.reset_state();
                        self.file_view.set_path(p);
                        self.handle_command(Some(Message::Select(0, false)));
                    }
                }
                Message::ToggleAutoScroll => {
                    self.file_view.auto_scroll_enabled = !self.in_scroll;
                    self.in_scroll = self.file_view.auto_scroll_enabled;

                    if self.file_view.auto_scroll_enabled {
                        self.toasts
                            .info(fl!(crate::LANGUAGE_LOADER, "toast-auto-scroll-on"))
                            .set_duration(Some(Duration::from_secs(3)));
                    } else {
                        self.toasts
                            .info(fl!(crate::LANGUAGE_LOADER, "toast-auto-scroll-off"))
                            .set_duration(Some(Duration::from_secs(3)));
                    }
                }
                Message::ShowSauce(file) => {
                    if file < self.file_view.files.len() {
                        if let Some(sauce) = self.file_view.files[file].get_sauce() {
                            self.sauce_dialog = Some(sauce_dialog::SauceDialog::new(sauce));
                        }
                    }
                }
                Message::ShowHelpDialog => {
                    self.help_dialog = Some(help_dialog::HelpDialog::new());
                }
                Message::ChangeScrollSpeed => {
                    self.file_view.scroll_speed =
                        (self.file_view.scroll_speed + 1) % SCROLL_SPEED.len();

                    match self.file_view.scroll_speed {
                        0 => {
                            self.toasts
                                .info(fl!(crate::LANGUAGE_LOADER, "toast-scroll-slow"))
                                .set_duration(Some(Duration::from_secs(3)));
                        }
                        1 => {
                            self.toasts
                                .info(fl!(crate::LANGUAGE_LOADER, "toast-scroll-medium"))
                                .set_duration(Some(Duration::from_secs(3)));
                        }
                        2 => {
                            self.toasts
                                .info(fl!(crate::LANGUAGE_LOADER, "toast-scroll-fast"))
                                .set_duration(Some(Duration::from_secs(3)));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn open(&mut self, file: usize) -> bool {
        if self.open_selected(file) && !self.file_view.files.is_empty() {
            self.file_view.selected_file = Some(0);
            self.file_view.scroll_pos = Some(0);
            self.view_selected(file, false);
            true
        } else {
            if let Some(file) = self.file_view.files.get(file) {
                self.opened_file = Some(file.clone());
            }

            false
        }
    }
}

fn is_binary(file_entry: &FileEntry) -> bool {
    if let Err(err) = file_entry.get_data(|_, data| {
        for i in data.iter().take(500) {
            if i == &0 || i == &255 {
                return true;
            }
        }
        false
    }) {
        log::warn!("Error while checking if file is binary: {}", err);
    }
    true
}
