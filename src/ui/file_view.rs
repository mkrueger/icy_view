use directories::UserDirs;
use eframe::egui;
use egui::{ScrollArea, TextEdit, Ui};
use egui_extras::{Column, RetainedImage, TableBuilder};
use icy_engine::SauceData;

use std::{
    env,
    fs::{self, File},
    io::{self, Error, Read},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};

pub enum Command {
    Select(usize),
    Open(usize),
    Refresh,
    ParentFolder,
}

#[derive(Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub file_data: Option<Vec<u8>>,
    pub read_sauce: bool,
    pub sauce: Option<SauceData>,
}

impl FileEntry {
    pub fn get_data<T>(&self, func: fn(&PathBuf, &[u8]) -> T) -> io::Result<T> {
        if let Some(data) = &self.file_data {
            return Ok(func(&self.path, data));
        }

        let file = File::open(&self.path)?;
        let mmap = unsafe { memmap::MmapOptions::new().map(&file)? };
        Ok(func(&self.path, &mmap))
    }

    pub fn read_image(
        &self,
        func: fn(&PathBuf, &[u8]) -> Result<RetainedImage, String>,
    ) -> JoinHandle<io::Result<RetainedImage>> {
        let path = self.path.clone();
        if let Some(data) = &self.file_data {
            let data = data.clone();
            thread::spawn(move || {
                if let Ok(ri) = func(&path, &data) {
                    return Ok(ri);
                }
                Err(io::Error::new(io::ErrorKind::Other, "can't read image"))
            })
        } else {
            thread::spawn(move || {
                let file = File::open(&path)?;
                let mmap = unsafe { memmap::MmapOptions::new().map(&file)? };
                if let Ok(ri) = func(&path, &mmap) {
                    return Ok(ri);
                }
                Err(io::Error::new(io::ErrorKind::Other, "can't read image"))
            })
        }
    }

    pub fn is_file(&self) -> bool {
        self.file_data.is_some() || self.path.is_file()
    }

    fn load_sauce(&mut self) {
        if self.read_sauce {
            return;
        }
        self.read_sauce = true;

        if let Ok(Ok(data)) = self.get_data(|_, data| SauceData::extract(data)) {
            self.sauce = Some(data);
        }
    }
}

pub struct FileView {
    /// Current opened path.
    path: PathBuf,
    /// Selected file path
    pub selected_file: Option<usize>,
    pub scroll_pos: Option<usize>,
    /// Files in directory.
    pub files: Vec<FileEntry>,

    pub filter: String,
    pre_select_file: Option<String>,
}

impl FileView {
    pub fn new(initial_path: Option<PathBuf>) -> Self {
        let mut path = if let Some(path) = initial_path {
            path
        } else if let Some(user_dirs) = UserDirs::new() {
            user_dirs.home_dir().to_path_buf()
        } else {
            env::current_dir().unwrap_or_default()
        };

        let mut pre_select_file = None;

        if !path.exists() {
            pre_select_file = Some(path.file_name().unwrap().to_string_lossy().to_string());
            path.pop();
        }

        if path.is_file()
            && path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                != "zip"
        {
            pre_select_file = Some(path.file_name().unwrap().to_string_lossy().to_string());
            path.pop();
        }

        Self {
            path,
            selected_file: None,
            pre_select_file,
            scroll_pos: None,
            files: Vec::new(),
            filter: String::new(),
        }
    }

    pub(crate) fn show_ui(&mut self, ui: &mut Ui) -> Option<Command> {
        let mut command: Option<Command> = None;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.path.parent().is_some(), |ui| {
                let response = ui.button("⬆").on_hover_text("Parent Folder");
                if response.clicked() {
                    command = Some(Command::ParentFolder);
                }
            });

            match self.path.to_str() {
                Some(path) => {
                    let mut path_edit = path.to_string();
                    ui.add_enabled_ui(false, |ui| {
                        ui.add_sized([250.0, 20.0], TextEdit::singleline(&mut path_edit));
                    });
                }
                None => {
                    ui.colored_label(ui.style().visuals.error_fg_color, "Invalid path");
                }
            }
            let response = ui.button("⟲").on_hover_text("Refresh");
            if response.clicked() {
                command = Some(Command::Refresh);
            }
            ui.separator();
            ui.add_sized(
                [250.0, 20.0],
                TextEdit::singleline(&mut self.filter).hint_text("Filter entries"),
            );
            let response = ui.button("🗙").on_hover_text("Reset filter");
            if response.clicked() {
                self.filter.clear();
            }
        });
        ui.add_space(ui.spacing().item_spacing.y);

        if self.selected_file.is_none() && !self.files.is_empty() {
            //  command = Some(Command::Select(0));
        }

        let area = ScrollArea::vertical();
        // let row_height = ui.text_style_height(&egui::TextStyle::Body);
        let row_height = ui.text_style_height(&egui::TextStyle::Body);

        let area_res = area.show(ui, |ui| {
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
                        let filter = self.filter.to_lowercase();
                        let f = self.files.iter_mut().filter(|p| {
                            if filter.is_empty() {
                                return true;
                            }
                            if let Some(sauce) = &p.sauce {
                                if sauce.title.to_string().to_lowercase().contains(&filter)
                                    || sauce.group.to_string().to_lowercase().contains(&filter)
                                    || sauce.author.to_string().to_lowercase().contains(&filter)
                                {
                                    return true;
                                }
                            }
                            p.path.to_string_lossy().to_lowercase().contains(&filter)
                        });

                        for (i, entry) in f.enumerate() {
                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    let is_selected = Some(first + i) == self.selected_file;

                                    if is_selected || ui.is_rect_visible(ui.available_rect_before_wrap()) {
                                        entry.load_sauce();
                                        let label = match entry.path.is_dir() {
                                            true => "🗀 ",
                                            false => "🗋 ",
                                        }
                                        .to_string()
                                            + get_file_name(&entry.path);
                                        let selectable_label = ui.selectable_label(is_selected, label);
                                        if selectable_label.clicked() {
                                            command = Some(Command::Select(first + i));
                                        }
                                        if let Some(sel) = self.scroll_pos {
                                            if sel == i {
                                                ui.scroll_to_rect(selectable_label.rect, None);
                                                self.scroll_pos = None;
                                            }
                                        }

                                        if selectable_label.double_clicked() {
                                            command = Some(Command::Open(first + i));
                                        }
                                    } 
                                });

                                row.col(|ui| {
                                    if ui.is_rect_visible(ui.available_rect_before_wrap()) {

                                    if let Some(sauce) = &entry.sauce {
                                        ui.label(sauce.title.to_string());
                                    } else {
                                        ui.label("");
                                    }
                                }
                                });
                                row.col(|ui| {
                                    if ui.is_rect_visible(ui.available_rect_before_wrap()) {

                                    if let Some(sauce) = &entry.sauce {
                                        ui.label(sauce.author.to_string());
                                    } else {
                                        ui.label("");
                                    }
                                }
                                });
                                row.col(|ui| {
                                    if ui.is_rect_visible(ui.available_rect_before_wrap()) {

                                    if let Some(sauce) = &entry.sauce {
                                        ui.label(sauce.group.to_string());
                                    } else {
                                        ui.label("");
                                    }
                                }
                                });
                                row.col(|ui| {
                                    if ui.is_rect_visible(ui.available_rect_before_wrap()) {

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
                                }
                                });
                            });
                        }
                    });
            })
            .response
        });

        if ui.input(|i| i.key_pressed(egui::Key::PageUp) && i.modifiers.ctrl) {
            return Some(Command::ParentFolder);
        }

        if let Some(s) = self.selected_file {
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && s > 0 {
                command = Some(Command::Select(s.saturating_sub(1)));
            }

            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && s + 1 < self.files.len() {
                command = Some(Command::Select(s.saturating_add(1)));
            }

            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                command = Some(Command::Open(s));
            }

            if !self.files.is_empty() {
                if ui.input(|i| i.key_pressed(egui::Key::Home)) {
                    command = Some(Command::Select(0));
                }

                if ui.input(|i| i.key_pressed(egui::Key::End)) {
                    command = Some(Command::Select(self.files.len().saturating_sub(1)));
                }

                if ui.input(|i| i.key_pressed(egui::Key::PageUp)) {
                    let page_size = (area_res.inner_rect.height() / row_height) as usize;
                    command = Some(Command::Select(s.saturating_sub(page_size)));
                }

                if ui.input(|i| i.key_pressed(egui::Key::PageDown)) {
                    let page_size = (area_res.inner_rect.height() / row_height) as usize;
                    command = Some(Command::Select(
                        (s.saturating_add(page_size)).min(self.files.len() - 1),
                    ));
                }
            }
        } else if !self.files.is_empty() {
            if ui.input(|i| {
                i.key_pressed(egui::Key::ArrowUp)
                    || i.key_pressed(egui::Key::ArrowDown)
                    || i.key_pressed(egui::Key::PageUp)
                    || i.key_pressed(egui::Key::PageDown)
            }) {
                command = Some(Command::Select(0));
            }

            if ui.input(|i| i.key_pressed(egui::Key::Home)) {
                command = Some(Command::Select(0));
            }

            if ui.input(|i| i.key_pressed(egui::Key::End)) {
                command = Some(Command::Select(self.files.len().saturating_sub(1)));
            }
        }
        command
    }

    pub fn get_path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn set_path(&mut self, path: impl Into<PathBuf>) -> Option<Command> {
        self.path = path.into();
        self.refresh()
    }

    pub fn refresh(&mut self) -> Option<Command> {
        self.files.clear();

        if self.path.is_file() {
            match fs::File::open(&self.path) {
                Ok(file) => match zip::ZipArchive::new(file) {
                    Ok(mut archive) => {
                        for i in 0..archive.len() {
                            match archive.by_index(i) {
                                Ok(mut file) => {
                                    let mut data = Vec::new();
                                    file.read_to_end(&mut data).unwrap_or_default();

                                    let entry = FileEntry {
                                        path: file
                                            .enclosed_name()
                                            .unwrap_or(Path::new("unknown"))
                                            .to_path_buf(),
                                        file_data: Some(data),
                                        read_sauce: false,
                                        sauce: None,
                                    };
                                    self.files.push(entry);
                                }
                                Err(err) => {
                                    log::error!("Error reading zip file: {}", err);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Error reading zip archive: {}", err);
                    }
                },
                Err(err) => {
                    log::error!("Failed to open zip file: {}", err);
                }
            }
        } else {
            let folders = read_folder(&self.path);
            match folders {
                Ok(folders) => {
                    self.files = folders
                        .iter()
                        .map(|f| FileEntry {
                            path: f.clone(),
                            read_sauce: false,
                            sauce: None,
                            file_data: None,
                        })
                        .collect();
                }
                Err(err) => {
                    log::error!("Failed to read folder: {}", err);
                }
            }
        }
        self.selected_file = None;

        if let Some(file) = &self.pre_select_file {
            for (i, entry) in self.files.iter().enumerate() {
                if entry.path.file_name().unwrap().to_string_lossy() == *file {
                    return Command::Select(i).into();
                }
            }
        }
        None
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

fn read_folder(path: &Path) -> Result<Vec<PathBuf>, Error> {
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
                }
                #[cfg(unix)]
                if get_file_name(path).starts_with('.') {
                    return false;
                }
                true
            })
            .collect()
    })
}
