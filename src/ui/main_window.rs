use eframe::{
    egui::{self, CentralPanel, Context, CursorIcon, PointerButton},
    epaint::{Color32, Rect},
    App, Frame,
};

use egui::{vec2, Align2, Layout, Pos2, ScrollArea, TextEdit, Ui, Vec2};
use icy_engine::Buffer;

use std::{
    cmp::max,
    env, fs,
    io::Error,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::BufferView;

pub struct MainWindow {
    pub buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,

    /// Current opened path.
    path: PathBuf,
    /// Selected file path
    selected_file: Option<usize>,

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
        let view = BufferView::new(gl);

        Self {
            buffer_view: Arc::new(eframe::epaint::mutex::Mutex::new(view)),
            path,
            selected_file: None,
            files: Vec::new(),

            #[cfg(unix)]
            show_hidden: false,
        }
    }

    fn ui_in_window(&mut self, ctx: &Context, ui: &mut Ui) {
        enum Command {
            Open(usize),
            OpenSelected,
            Refresh,
            Select(usize),
            UpDirectory,
        }
        let mut command: Option<Command> = None;

        // Rows with files.
        egui::SidePanel::left("left_panel").show_inside(ui, |ui| {
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
                    let response = ui.add_sized(
                        ui.available_size(),
                        TextEdit::singleline(&mut path_edit),
                    );
                    
                    /* 
                    if response.lost_focus() {
                        let path = PathBuf::from(&self.path_edit);
                        command = Some(Command::Open(path));
                    };*/
                });
            });
            ui.add_space(ui.spacing().item_spacing.y);

            if self.selected_file.is_none() {
                if self.files.len() > 0 {
                  command = Some(Command::Select(0));
                }
            }

            let area = ScrollArea::vertical();
            let output = area.show_rows(
                ui,
                ui.text_style_height(&egui::TextStyle::Body),
                self.files.len(),
                |ui, range| {
                    ui.with_layout(ui.layout().with_cross_justify(true), |ui| {
                      let first = range.start;
                      let mut i = 0;
                      for path in self.files[range].iter() {
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

                          if selectable_label.double_clicked()
                              || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                              if path.is_dir() {
                                  command = Some(Command::OpenSelected);
                              }
                          }
                          i += 1;
                      }
                  }) .response     
                },
            );

            if let Some(s) = self.selected_file {
              if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                if s > 0  {
                  command = Some(Command::Select(s - 1));
                }
              }
              if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                if s + 1 < self.files.len() {
                  command = Some(Command::Select(s + 1));
                }
              }
            }

        });

        let top_margin_height: f32 = 0.;

        let frame_no_margins = egui::containers::Frame::none()
            .inner_margin(egui::style::Margin::same(0.0))
            .fill(Color32::from_rgb(0x40, 0x44, 0x4b));
        egui::CentralPanel::default()
            .frame(frame_no_margins)
            .show_inside(ui, |ui| self.custom_painting(ui, top_margin_height));

        if let Some(command) = command {
            match command {
                Command::Select(file) => self.select(Some(file)),
                Command::Open(idx) => {
                    self.select(Some(idx));
                    self.open_selected();
                }
                Command::OpenSelected => self.open_selected(),
                Command::Refresh => self.refresh(),
                Command::UpDirectory => {
                    if self.path.pop() {
                        self.refresh();
                    }
                }
            };
        }
    }

    fn custom_painting(&mut self, ui: &mut egui::Ui, top_margin_height: f32) -> egui::Response {
        let available_rect = ui.available_rect_before_wrap();
        let output = ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(false)
            .drag_to_scroll(true)
            .show_viewport(ui, |ui, viewport| {
                let (id, rect) = ui.allocate_space(available_rect.size());
                let rect = available_rect;
                let mut response = ui.interact(rect, id, egui::Sense::click());
                let size = available_rect.size();
                let buffer_view = self.buffer_view.clone();
                let buf_w = buffer_view.lock().buf.get_buffer_width();
                let buf_h = buffer_view.lock().buf.get_buffer_height();
                // let h = max(buf_h, buffer_view.lock().buf.get_real_buffer_height());

                let font_dimensions = buffer_view.lock().buf.get_font_dimensions();
                let mut scale_x = size.x / font_dimensions.width as f32 / buf_w as f32;
                let mut scale_y = size.y / font_dimensions.height as f32 / buf_h as f32;

                if scale_x < scale_y {
                    scale_y = scale_x;
                } else {
                    scale_x = scale_y;
                }

                let char_size = Vec2::new(
                    font_dimensions.width as f32 * scale_x,
                    font_dimensions.height as f32 * scale_y,
                );

                let rect_w = buf_w as f32 * char_size.x;
                let rect_h = buf_h as f32 * char_size.y;

                let terminal_rect = Rect::from_min_size(
                    rect.left_top()
                        + Vec2::new(
                            3. + (rect.width() - rect_w) / 2.,
                            (-top_margin_height + viewport.top() + (rect.height() - rect_h) / 2.)
                                .floor(),
                        )
                        .ceil(),
                    Vec2::new(rect_w, rect_h),
                );
                let real_height = buffer_view.lock().buf.get_real_buffer_height();
                let max_lines = max(0, real_height - buf_h);
                ui.set_height(scale_y * max_lines as f32 * font_dimensions.height as f32);

                let first_line = (viewport.top() / char_size.y) as i32;
                let scroll_back_line = max(0, first_line);
                if scroll_back_line != buffer_view.lock().scroll_back_line {
                    buffer_view.lock().scroll_back_line = scroll_back_line;
                    buffer_view.lock().redraw_view();
                }
                ui.set_width(rect_w);

                let callback = egui::PaintCallback {
                    rect,
                    callback: std::sync::Arc::new(egui_glow::CallbackFn::new(
                        move |info, painter| {
                            buffer_view.lock().update_buffer(painter.gl());
                            buffer_view
                                .lock()
                                .paint(painter.gl(), info, rect, terminal_rect);
                        },
                    )),
                };
                ui.painter().add(callback);

                if response.hovered() {
                    let hover_pos_opt = ui.input(|i| i.pointer.hover_pos());
                    if let Some(hover_pos) = hover_pos_opt {
                        if terminal_rect.contains(hover_pos) {
                            ui.output_mut(|o| o.cursor_icon = CursorIcon::Text);
                        }
                    }
                }
                response.dragged = false;
                response.drag_released = true;
                response.is_pointer_button_down_on = false;
                response.interact_pointer_pos = None;
                response
            });

        output.inner
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

    fn confirm(&mut self) {}

    fn get_folder(&self) -> &Path {
        if let Some(idx) = &self.selected_file {
          let path = &self.files[*idx];
          if path.is_dir() {
                return path.as_path();
            }
        }

        // No selected file or it's not a folder, so use the current path.
        &self.path
    }

    pub fn refresh(&mut self) {
        self.files = read_folder(
            &self.path,
            #[cfg(unix)]
            self.show_hidden,
        ).unwrap();
        self.select(None);
    }

    fn select(&mut self, file: Option<usize>) {
        if let Some(idx) = &file {
            let path = &self.files[*idx];
            if path.is_file() {
                self.buffer_view.lock().buf = Buffer::load_buffer(path).unwrap()
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
