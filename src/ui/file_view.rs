use eframe::egui;
use egui::{Layout, ScrollArea, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};
use icy_engine::SauceData;

use std::{
    env,
    fs::{self, File},
    io::{Error, Read},
    path::{Path, PathBuf},
};

pub enum Command {
    Select(usize),
}

#[derive(Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub file_data: Option<Vec<u8>>,
    pub read_sauce: bool,
    pub sauce: Option<SauceData>,
}

impl FileEntry {
    pub fn get_data(&self) -> Vec<u8> {
        if let Some(data) = &self.file_data {
            return data.clone();
        }
        fs::read(&self.path).expect("Folder icon file donest exist")
    }

    pub fn is_file(&self) -> bool {
        self.file_data.is_some() || self.path.is_file()
    }
}

pub struct FileView {
    /// Current opened path.
    path: PathBuf,
    /// Selected file path
    pub selected_file: Option<usize>,
    scroll_pos: Option<usize>,
    /// Files in directory.
    pub files: Vec<FileEntry>,

    // Show hidden files on unix systems.
    #[cfg(unix)]
    show_hidden: bool,
}

impl FileView {
    pub fn new(initial_path: Option<PathBuf>) -> Self {
        let mut path: PathBuf =
            initial_path.unwrap_or_else(|| env::current_dir().unwrap_or_default());

        if path.is_file()
            && path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                != "zip"
        {
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

        let area = ScrollArea::vertical();
        // let row_height = ui.text_style_height(&egui::TextStyle::Body);
        let row_height = ui.text_style_height(&egui::TextStyle::Body);

        area.show(ui, |ui| {
            ui.with_layout(ui.layout().with_cross_justify(true), |ui| {
                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::remainder())
                    .min_scrolled_height(0.0);

                table
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("File");
                        });
                        header.col(|ui| {
                            ui.strong("Title");
                        });
                        header.col(|ui| {
                            ui.strong("Author");
                        });
                        header.col(|ui| {
                            ui.strong("Group");
                        });
                        header.col(|ui| {
                            ui.strong("Screen mode");
                        });
                    })
                    .body(|mut body| {
                        let first = 0;
                        let f = self.files.clone();
                        for (i, entry) in f.iter().enumerate() {
                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    let label = match entry.path.is_dir() {
                                        true => "🗀 ",
                                        false => "🗋 ",
                                    }
                                    .to_string()
                                        + get_file_name(&entry.path);
                                    let is_selected = Some(first + i) == self.selected_file;
                                    let selectable_label = ui.selectable_label(is_selected, label);
                                    if selectable_label.clicked() && entry.is_file() {
                                        command = Some(Command::Select(first + i));
                                    }
                                    if let Some(sel) = self.scroll_pos {
                                        if sel == i {
                                            ui.scroll_to_rect(selectable_label.rect, None);
                                            self.scroll_pos = None;
                                        }
                                    }

                                    if (selectable_label.double_clicked()
                                        || ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                        && entry.path.is_dir()
                                    {
                                        self.open(first + i);
                                    }
                                });

                                row.col(|ui| {
                                    if let Some(sauce) = &entry.sauce {
                                        ui.label(sauce.title.to_string());
                                    } else {
                                        ui.label("");
                                    }
                                });
                                row.col(|ui| {
                                    if let Some(sauce) = &entry.sauce {
                                        ui.label(sauce.author.to_string());
                                    } else {
                                        ui.label("");
                                    }
                                });
                                row.col(|ui| {
                                    if let Some(sauce) = &entry.sauce {
                                        ui.label(sauce.group.to_string());
                                    } else {
                                        ui.label("");
                                    }
                                });
                                row.col(|ui| {
                                    if entry.path.is_dir() {
                                        ui.label("");
                                    } else if let Some(sauce) = &entry.sauce {
                                        ui.label(format!(
                                            "{}x{} {}",
                                            sauce.buffer_size.width,
                                            sauce.buffer_size.height,
                                            if sauce.use_ice { "(ICE)" } else { "" }
                                        ));
                                    } else {
                                        ui.label("");
                                    }
                                });
                            });
                        }
                    });
            })
            .response
        });

        if let Some(s) = self.selected_file {
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && s > 0 {
                command = Some(Command::Select(s - 1));
                self.scroll_pos = Some(s - 1);
            }

            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && s + 1 < self.files.len() {
                command = Some(Command::Select(s + 1));
                self.scroll_pos = Some(s + 1);
            }
        }
        command
    }

    fn open(&mut self, idx: usize) {
        let entry = &self.files[idx];
        if entry.path.is_dir() {
            self.set_path(entry.path.clone())
        }
    }
    pub fn set_path(&mut self, path: impl Into<PathBuf>) {
        self.path = path.into();
        self.refresh();
    }

    pub fn refresh(&mut self) {
        if self.path.is_file() {
            self.files.clear();
            let file = fs::File::open(&self.path).unwrap();
            let mut archive = zip::ZipArchive::new(file).unwrap();
            for i in 0..archive.len() {
                let mut file = archive.by_index(i).unwrap();
                let mut data = Vec::new();
                file.read_to_end(&mut data).unwrap();
                let sauce = SauceData::extract(&data).ok();

                let entry = FileEntry {
                    path: file
                        .enclosed_name()
                        .unwrap_or(Path::new("unknown"))
                        .to_path_buf(),
                    file_data: Some(data),
                    read_sauce: true,
                    sauce,
                };
                self.files.push(entry);
            }
            return;
        }

        self.files = read_folder(
            &self.path,
            #[cfg(unix)]
            self.show_hidden,
        )
        .unwrap()
        .iter()
        .map(|f| FileEntry {
            path: f.clone(),
            read_sauce: false,
            sauce: None,
            file_data: None,
        })
        .collect();

        for entry in &mut self.files {
            if !entry.read_sauce {
                entry.read_sauce = true;

                let file = File::open(&entry.path);

                if let Ok(file) = file {
                    let mmap = unsafe { memmap::MmapOptions::new().map(&file) };
                    if let Ok(map) = mmap {
                        if let Ok(data) = SauceData::extract(&map) {
                            entry.sauce = Some(data);
                        }
                    }
                }
            }
        }

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
