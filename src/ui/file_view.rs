use directories::UserDirs;
use eframe::egui::{self, Context, RichText};
use egui::{ScrollArea, TextEdit, Ui};
use egui_extras::{Column, RetainedImage, TableBuilder};
use i18n_embed_fl::fl;
use icy_engine::SauceData;

use std::{
    env,
    fs::{self, File},
    io::{self, Error, Read},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};

pub enum Message {
    Select(usize, bool),
    Open(usize),
    Refresh,
    ParentFolder,
    ToggleAutoScroll,
    ShowSauce(usize),
    ShowHelpDialog,
    ChangeScrollSpeed,
}

#[derive(Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub file_data: Option<Vec<u8>>,
    pub read_sauce: bool,
    pub sauce: Option<SauceData>,
    is_dir: Option<bool>,
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

    pub(crate) fn is_dir(&self) -> bool {
        if let Some(is_dir) = self.is_dir {
            return is_dir;
        }
        self.path.is_dir()
    }

    fn is_dir_or_archive(&self) -> bool {
        if let Some(ext) = self.path.extension() {
            if ext.to_string_lossy().to_ascii_lowercase() == "zip" {
                return true;
            }
        }

        self.is_dir()
    }

    pub(crate) fn get_sauce(&self) -> Option<SauceData> {
        if !self.read_sauce {
            return None;
        }
        self.sauce.clone()
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

    pub auto_scroll_enabled: bool,
    pub scroll_speed: usize,
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
            auto_scroll_enabled: true,
            scroll_speed: 1,
        }
    }

    pub(crate) fn show_ui(&mut self, ctx: &Context, ui: &mut Ui) -> Option<Message> {
        let mut command: Option<Message> = None;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.path.parent().is_some(), |ui| {
                let response = ui.button("⬆").on_hover_text("Parent Folder");
                if response.clicked() {
                    command = Some(Message::ParentFolder);
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
                    ui.colored_label(
                        ui.style().visuals.error_fg_color,
                        fl!(crate::LANGUAGE_LOADER, "error-invalid-path"),
                    );
                }
            }
            let response = ui
                .button("⟲")
                .on_hover_text(fl!(crate::LANGUAGE_LOADER, "tooltip-refresh"));
            if response.clicked() {
                command = Some(Message::Refresh);
            }
            ui.separator();
            ui.add_sized(
                [250.0, 20.0],
                TextEdit::singleline(&mut self.filter)
                    .hint_text(fl!(crate::LANGUAGE_LOADER, "filter-entries-hint-text")),
            );
            let response = ui
                .button("🗙")
                .on_hover_text(fl!(crate::LANGUAGE_LOADER, "tooltip-reset-filter-button"));
            if response.clicked() {
                self.filter.clear();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("…", |ui| {
                    let r = ui.hyperlink_to(
                        fl!(crate::LANGUAGE_LOADER, "menu-item-discuss"),
                        "https://github.com/mkrueger/icy_view/discussions",
                    );
                    if r.clicked() {
                        ui.close_menu();
                    }
                    let r = ui.hyperlink_to(
                        fl!(crate::LANGUAGE_LOADER, "menu-item-report-bug"),
                        "https://github.com/mkrueger/icy_view/issues/new",
                    );
                    if r.clicked() {
                        ui.close_menu();
                    }
                    let r = ui.hyperlink_to(
                        fl!(crate::LANGUAGE_LOADER, "menu-item-check-releases"),
                        "https://github.com/mkrueger/icy_view/releases/latest",
                    );
                    if r.clicked() {
                        ui.close_menu();
                    }
                    ui.separator();
                    let mut b = self.auto_scroll_enabled;
                    if ui
                        .checkbox(&mut b, fl!(crate::LANGUAGE_LOADER, "menu-item-auto-scroll"))
                        .clicked()
                    {
                        command = Some(Message::ToggleAutoScroll);
                        ui.close_menu();
                    }
                    let title = match self.scroll_speed {
                        2 => fl!(crate::LANGUAGE_LOADER, "menu-item-scroll-speed-slow"),
                        0 => fl!(crate::LANGUAGE_LOADER, "menu-item-scroll-speed-medium"),
                        1 => fl!(crate::LANGUAGE_LOADER, "menu-item-scroll-speed-fast"),
                        _ => panic!(),
                    };

                    let r = ui.selectable_label(false, title);
                    if r.clicked() {
                        command = Some(Message::ChangeScrollSpeed);
                        ui.close_menu();
                    }
                });
            });
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
                            ui.strong(fl!(crate::LANGUAGE_LOADER, "heading-file"));
                        });
                        header.col(|ui| {
                            ui.strong(fl!(crate::LANGUAGE_LOADER, "heading-title"));
                        });
                        header.col(|ui| {
                            ui.strong(fl!(crate::LANGUAGE_LOADER, "heading-author"));
                        });
                        header.col(|ui| {
                            ui.strong(fl!(crate::LANGUAGE_LOADER, "heading-group"));
                        });
                        header.col(|ui| {
                            ui.strong(fl!(crate::LANGUAGE_LOADER, "heading-screen-mode"));
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
                            let is_selected = Some(first + i) == self.selected_file;
                            let text_color = if is_selected {
                                ctx.style().visuals.strong_text_color()
                            } else {
                                ctx.style().visuals.text_color()
                            };

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    if is_selected
                                        || ui.is_rect_visible(ui.available_rect_before_wrap())
                                    {
                                        entry.load_sauce();
                                        let label = match entry.is_dir_or_archive() {
                                            true => "🗀 ",
                                            false => "🗋 ",
                                        }
                                        .to_string()
                                            + get_file_name(&entry.path);

                                        let selectable_label = ui.selectable_label(
                                            is_selected,
                                            RichText::new(label).color(text_color),
                                        );
                                        if selectable_label.clicked() {
                                            command = Some(Message::Select(first + i, false));
                                        }
                                        if let Some(sel) = self.scroll_pos {
                                            if sel == i {
                                                ui.scroll_to_rect(selectable_label.rect, None);
                                                self.scroll_pos = None;
                                            }
                                        }

                                        if selectable_label.double_clicked() {
                                            command = Some(Message::Open(first + i));
                                        }
                                    }
                                });

                                row.col(|ui| {
                                    if ui.is_rect_visible(ui.available_rect_before_wrap()) {
                                        if let Some(sauce) = &entry.sauce {
                                            ui.label(
                                                RichText::new(sauce.title.to_string())
                                                    .color(text_color),
                                            );
                                        } else {
                                            ui.label("");
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    if ui.is_rect_visible(ui.available_rect_before_wrap()) {
                                        if let Some(sauce) = &entry.sauce {
                                            ui.label(
                                                RichText::new(sauce.author.to_string())
                                                    .color(text_color),
                                            );
                                        } else {
                                            ui.label("");
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    if ui.is_rect_visible(ui.available_rect_before_wrap()) {
                                        if let Some(sauce) = &entry.sauce {
                                            ui.label(
                                                RichText::new(sauce.group.to_string())
                                                    .color(text_color),
                                            );
                                        } else {
                                            ui.label("");
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    if ui.is_rect_visible(ui.available_rect_before_wrap()) {
                                        if entry.is_dir() {
                                            ui.label("");
                                        } else if let Some(sauce) = &entry.sauce {

                                            let mut flags: String = String::new();
                                            if sauce.use_ice {
                                                flags.push_str("ICE");
                                            }
                    
                                            if sauce.use_letter_spacing {
                                                if !flags.is_empty() {
                                                    flags.push(',');
                                                }
                                                flags.push_str("9px");
                                            }
                    
                                            if sauce.use_aspect_ratio {
                                                if !flags.is_empty() {
                                                    flags.push(',');
                                                }
                                                flags.push_str("AR");
                                            }

                                            if flags.is_empty() {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{}x{}",
                                                        sauce.buffer_size.width,
                                                        sauce.buffer_size.height
                                                    ))
                                                    .color(text_color),
                                                );
                                            } else {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{}x{} ({})",
                                                        sauce.buffer_size.width,
                                                        sauce.buffer_size.height,
                                                        flags
                                                    ))
                                                    .color(text_color),
                                                );
                                            }
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

        if ui.is_enabled() {
            if ui.input(|i| i.key_pressed(egui::Key::PageUp) && i.modifiers.alt) {
                return Some(Message::ParentFolder);
            }

            if ui.input(|i| i.key_pressed(egui::Key::F1)) {
                return Some(Message::ShowHelpDialog);
            }

            if ui.input(|i| i.key_pressed(egui::Key::F2)) {
                return Some(Message::ToggleAutoScroll);
            }

            if ui.input(|i| i.key_pressed(egui::Key::F3)) {
                return Some(Message::ChangeScrollSpeed);
            }

            if let Some(s) = self.selected_file {
                if ui.input(|i| i.key_pressed(egui::Key::F4)) {
                    return Some(Message::ShowSauce(s));
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)&& i.modifiers.is_none()) && s > 0 {
                    command = Some(Message::Select(s.saturating_sub(1), false));
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)&& i.modifiers.is_none()) && s + 1 < self.files.len() {
                    command = Some(Message::Select(s.saturating_add(1), false));
                }

                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    command = Some(Message::Open(s));
                }

                if !self.files.is_empty() {
                    if ui.input(|i: &egui::InputState| i.key_pressed(egui::Key::Home)&& i.modifiers.is_none()) {
                        command = Some(Message::Select(0, false));
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::End)&& i.modifiers.is_none()) {
                        command = Some(Message::Select(self.files.len().saturating_sub(1), false));
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::PageUp)&& i.modifiers.is_none()) {
                        let page_size = (area_res.inner_rect.height() / row_height) as usize;
                        command = Some(Message::Select(s.saturating_sub(page_size), false));
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::PageDown)&& i.modifiers.is_none()) {
                        let page_size = (area_res.inner_rect.height() / row_height) as usize;
                        command = Some(Message::Select(
                            (s.saturating_add(page_size)).min(self.files.len() - 1),
                            false,
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
                    command = Some(Message::Select(0, false));
                }

                if ui.input(|i| i.key_pressed(egui::Key::Home)) {
                    command = Some(Message::Select(0, false));
                }

                if ui.input(|i| i.key_pressed(egui::Key::End)) {
                    command = Some(Message::Select(self.files.len().saturating_sub(1), false));
                }
            }
        }
        command
    }

    pub fn get_path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn set_path(&mut self, path: impl Into<PathBuf>) -> Option<Message> {
        self.path = path.into();
        self.refresh()
    }

    pub fn refresh(&mut self) -> Option<Message> {
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
                                        is_dir: Some(file.is_dir()),
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
                            is_dir: None,
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
                    return Message::Select(i, false).into();
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
