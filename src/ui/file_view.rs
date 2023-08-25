use eframe::egui;
use egui::{Layout, ScrollArea, TextEdit, Ui};

use std::{
    env, fs,
    io::Error,
    path::{Path, PathBuf},
};

pub enum Command {
    Select(usize),
}

pub struct FileView {
    /// Current opened path.
    path: PathBuf,
    /// Selected file path
    selected_file: Option<usize>,
    scroll_pos: Option<usize>,
    /// Files in directory.
    pub files: Vec<PathBuf>,

    // Show hidden files on unix systems.
    #[cfg(unix)]
    show_hidden: bool,
}

impl FileView {
    pub fn new(initial_path: Option<PathBuf>) -> Self {
        let mut path: PathBuf =
            initial_path.unwrap_or_else(|| env::current_dir().unwrap_or_default());

        if path.is_file() {
            path.pop();
        }
        Self {
            path,
            selected_file: None,
            scroll_pos: None,
            files: Vec::new(),

            #[cfg(unix)]
            show_hidden: false,
        }
    }

    pub(crate) fn show_ui(&mut self, ui: &mut Ui) -> Option<Command> {
        let mut command: Option<Command> = None;

        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.path.parent().is_some(), |ui| {
                let response = ui.button("⬆").on_hover_text("Parent Folder");
                if response.clicked() && self.path.pop() {
                    self.refresh();
                }
            });
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                let response = ui.button("⟲").on_hover_text("Refresh");
                if response.clicked() {
                    self.refresh();
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
            //  command = Some(Command::Select(0));
        }

        let mut area = ScrollArea::vertical();
        let row_height = ui.text_style_height(&egui::TextStyle::Body);

        area.show(ui, |ui| {
            ui.with_layout(ui.layout().with_cross_justify(true), |ui| {
                let first = 0;
                let f = self.files.to_vec();
                for (i, path) in f.iter().enumerate().clone() {
                    let label = match path.is_dir() {
                        true => "🗀 ",
                        false => "🗋 ",
                    }
                    .to_string()
                        + get_file_name(path);

                    let is_selected = Some(first + i) == self.selected_file;
                    let selectable_label = ui.selectable_label(is_selected, label);
                    if selectable_label.clicked() {
                        command = self.select(Some(first + i));
                    }
                    if let Some(sel) = self.scroll_pos {
                        if sel == i {
                            ui.scroll_to_rect(selectable_label.rect, None);
                            self.scroll_pos = None;
                        }
                    }

                    if (selectable_label.double_clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        && path.is_dir()
                    {
                        self.open_selected();
                        return;
                    }
                }
            })
            .response
        });

        if let Some(s) = self.selected_file {
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && s > 0 {
                command = self.select(Some(s - 1));
                self.scroll_pos = Some(s - 1);
            }

            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && s + 1 < self.files.len() {
                command = self.select(Some(s + 1));
                self.scroll_pos = Some(s + 1);
            }
        }
        command
    }

    fn select(&mut self, file: Option<usize>) -> Option<Command> {
        let mut res = None;
        if let Some(idx) = &file {
            let path = &self.files[*idx];
            if path.is_file() {
                res = Some(Command::Select(*idx));
            }
        };

        self.selected_file = file;
        res
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
        // self.select(None);
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
