use eframe::{
    egui::{self, CentralPanel, Context, Response},
    epaint::{Color32, Vec2},
    App, Frame,
};

use egui::{Layout, ScrollArea, TextEdit, Ui};
use icy_engine::Buffer;
use icy_engine_egui::{BufferView, MonitorSettings};

use std::{
    env, fs,
    io::Error,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct MainWindow {
    pub buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,

    /// Current opened path.
    path: PathBuf,
    /// Selected file path
    selected_file: Option<usize>,
    scroll_pos: Option<f32>,
    /// Files in directory.
    files: Vec<PathBuf>,

    // Show hidden files on unix systems.
    #[cfg(unix)]
    show_hidden: bool,
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
        let mut path = initial_path.unwrap_or_else(|| env::current_dir().unwrap_or_default());

        if path.is_file() {
            path.pop();
        }

        let gl = cc
            .gl
            .as_ref()
            .expect("You need to run eframe with the glow backend");
        let mut view = BufferView::new(gl, glow::NEAREST as i32);
        view.caret.is_visible = false;

        Self {
            buffer_view: Arc::new(eframe::epaint::mutex::Mutex::new(view)),
            path,
            selected_file: None,
            scroll_pos: None,
            files: Vec::new(),

            #[cfg(unix)]
            show_hidden: false,
        }
    }

    fn ui_in_window(&mut self, ctx: &Context, ui: &mut Ui) {
        enum Command {
            OpenSelected,
            Refresh,
            Select(usize),
            UpDirectory,
        }
        let mut command: Option<Command> = None;

        // Rows with files.

        egui::TopBottomPanel::bottom("bottom_panel")
            .default_height(400.)
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(self.path.parent().is_some(), |ui| {
                        let response = ui.button("⬆").on_hover_text("Parent Folder");
                        if response.clicked() {
                            command = Some(Command::UpDirectory);
                        }
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        let response = ui.button("⟲").on_hover_text("Refresh");
                        if response.clicked() {
                            command = Some(Command::Refresh);
                        }
                        let mut path_edit = self.path.to_str().unwrap().to_string();
                        let _response =
                            ui.add_sized(ui.available_size(), TextEdit::singleline(&mut path_edit));

                        /*
                        if response.lost_focus() {
                            let path = PathBuf::from(&self.path_edit);
                            command = Some(Command::Open(path));
                        };*/
                    });
                });
                ui.add_space(ui.spacing().item_spacing.y);

                if self.selected_file.is_none() && !self.files.is_empty() {
                    command = Some(Command::Select(0));
                }

                let mut area = ScrollArea::vertical();
                let row_height = ui.text_style_height(&egui::TextStyle::Body);

                if let Some(sel) = self.scroll_pos {
                    area = area.vertical_scroll_offset(sel);
                    self.scroll_pos = None;
                }
                let mut r = std::ops::Range::<usize> { start: 0, end: 0 };
                area.show_rows(ui, row_height, self.files.len(), |ui, range| {
                    r = range.clone();
                    ui.with_layout(ui.layout().with_cross_justify(true), |ui| {
                        let first = range.start;
                        for (i, path) in self.files[range].iter().enumerate() {
                            let label = match path.is_dir() {
                                true => "🗀 ",
                                false => "🗋 ",
                            }
                            .to_string()
                                + get_file_name(path);

                            let is_selected = Some(first + i) == self.selected_file;
                            let selectable_label = ui.selectable_label(is_selected, label);
                            if selectable_label.clicked() {
                                command = Some(Command::Select(first + i));
                            }

                            if (selectable_label.double_clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                && path.is_dir()
                            {
                                command = Some(Command::OpenSelected);
                            }
                        }
                    })
                    .response
                });

                if let Some(s) = self.selected_file {
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && s > 0 {
                        command = Some(Command::Select(s - 1));
                        if r.start > s - 1 {
                            let spacing = ui.spacing().item_spacing;
                            let pos = (row_height + spacing.y) * (s - 1) as f32;
                            self.scroll_pos = Some(pos);
                        }
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && s + 1 < self.files.len()
                    {
                        command = Some(Command::Select(s + 1));
                        if r.end.saturating_sub(10) <= s {
                            let spacing = ui.spacing().item_spacing;
                            let pos = (row_height + spacing.y) * (s + 1) as f32;
                            self.scroll_pos = Some(pos);
                        }
                    }
                }
            });

        let frame_no_margins = egui::containers::Frame::none()
            .inner_margin(egui::style::Margin::same(0.0))
            .fill(Color32::BLACK);
        egui::CentralPanel::default()
            .frame(frame_no_margins)
            .show_inside(ui, |ui| self.custom_painting(ui));

        if let Some(command) = command {
            match command {
                Command::Select(file) => self.select(Some(file)),
                Command::OpenSelected => {
                    self.open_selected();
                    ctx.request_repaint();
                }
                Command::Refresh => self.refresh(),
                Command::UpDirectory => {
                    if self.path.pop() {
                        self.refresh();
                    }
                }
            };
        }
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

    fn open_selected(&mut self) {
        if let Some(idx) = &self.selected_file {
            let path = &self.files[*idx];
            if path.is_dir() {
                self.set_path(path.clone())
            }
        }
    }

    pub fn set_path(&mut self, path: impl Into<PathBuf>) {
        self.path = path.into();
        self.refresh();
    }

    pub fn refresh(&mut self) {
        self.files = read_folder(
            &self.path,
            #[cfg(unix)]
            self.show_hidden,
        )
        .unwrap();
        self.select(None);
    }

    fn select(&mut self, file: Option<usize>) {
        if let Some(idx) = &file {
            let path = &self.files[*idx];
            if path.is_file() {
                if let Ok(buf) = Buffer::load_buffer(path, true) {
                    self.buffer_view.lock().set_buffer(buf);
                }
            }
        };

        self.selected_file = file;
    }
}

#[cfg(windows)]
fn is_drive_root(path: &Path) -> bool {
    path.to_str()
        .filter(|path| &path[1..] == ":\\")
        .and_then(|path| path.chars().next())
        .map_or(false, |ch| ch.is_ascii_uppercase())
}

fn get_file_name(path: &Path) -> &str {
    #[cfg(windows)]
    if path.is_dir() && is_drive_root(path) {
        return path.to_str().unwrap_or_default();
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

#[cfg(windows)]
extern "C" {
    pub fn GetLogicalDrives() -> u32;
}

fn read_folder(path: &Path, #[cfg(unix)] show_hidden: bool) -> Result<Vec<PathBuf>, Error> {
    #[cfg(windows)]
    let drives = {
        let mut drives = unsafe { GetLogicalDrives() };
        let mut letter = b'A';
        let mut drive_names = Vec::new();
        while drives > 0 {
            if drives & 1 != 0 {
                drive_names.push(format!("{}:\\", letter as char).into());
            }
            drives >>= 1;
            letter += 1;
        }
        drive_names
    };

    fs::read_dir(path).map(|paths| {
        let mut result: Vec<PathBuf> = paths
            .filter_map(|result| result.ok())
            .map(|entry| entry.path())
            .collect();
        result.sort_by(|a, b| {
            let da = a.is_dir();
            let db = b.is_dir();
            match da == db {
                true => a.file_name().cmp(&b.file_name()),
                false => db.cmp(&da),
            }
        });

        #[cfg(windows)]
        let result = {
            let mut items = drives;
            items.reserve(result.len());
            items.append(&mut result);
            items
        };

        result
            .into_iter()
            .filter(|path| {
                if !path.is_dir() {
                    // Do not show system files.
                    if !path.is_file() {
                        return false;
                    }
                    // Filter.
                    /* if let Some(filter) = filter.as_ref() {
                      if !filter(path) {
                        return false;
                      }
                    } else if dialog_type == DialogType::SelectFolder {
                      return false;
                    }*/
                }
                #[cfg(unix)]
                if !show_hidden && get_file_name(path).starts_with('.') {
                    return false;
                }
                true
            })
            .collect()
    })
}
