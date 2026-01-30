use crate::bagit::{bag_directory, Progress};
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

#[derive(Default)]
enum AppState {
    #[default]
    Idle,
    Processing {
        total_files: usize,
        current: usize,
        current_file: String,
        stage: String,
    },
    Done {
        path: PathBuf,
        file_count: usize,
    },
    Error {
        message: String,
    },
}

pub struct BagItApp {
    state: AppState,
    progress_rx: Option<Receiver<Progress>>,
}

impl Default for BagItApp {
    fn default() -> Self {
        Self {
            state: AppState::Idle,
            progress_rx: None,
        }
    }
}

impl BagItApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn start_bagging(&mut self, path: PathBuf) {
        let (tx, rx) = channel();
        self.progress_rx = Some(rx);
        self.state = AppState::Processing {
            total_files: 0,
            current: 0,
            current_file: String::new(),
            stage: "Starting...".to_string(),
        };

        thread::spawn(move || {
            if let Err(e) = bag_directory(&path, Some(tx.clone())) {
                let _ = tx.send(Progress::Error {
                    message: e.to_string(),
                });
            }
        });
    }

    fn process_progress(&mut self) {
        let mut clear_rx = false;

        if let Some(ref rx) = self.progress_rx {
            while let Ok(progress) = rx.try_recv() {
                match progress {
                    Progress::Started { total_files } => {
                        self.state = AppState::Processing {
                            total_files,
                            current: 0,
                            current_file: String::new(),
                            stage: "Preparing...".to_string(),
                        };
                    }
                    Progress::Moving { current, filename } => {
                        if let AppState::Processing {
                            total_files,
                            current_file,
                            stage,
                            ..
                        } = &mut self.state
                        {
                            *current_file = filename;
                            *stage = format!("Moving files ({}/{})", current, *total_files);
                        }
                    }
                    Progress::Checksumming { current, filename } => {
                        if let AppState::Processing {
                            total_files,
                            current: curr,
                            current_file,
                            stage,
                        } = &mut self.state
                        {
                            *curr = current;
                            *current_file = filename;
                            *stage = format!("Checksumming ({}/{})", current, *total_files);
                        }
                    }
                    Progress::Done { path } => {
                        let file_count = if let AppState::Processing { total_files, .. } = &self.state {
                            *total_files
                        } else {
                            0
                        };
                        self.state = AppState::Done { path, file_count };
                        clear_rx = true;
                    }
                    Progress::Error { message } => {
                        self.state = AppState::Error { message };
                        clear_rx = true;
                    }
                }
            }
        }

        if clear_rx {
            self.progress_rx = None;
        }
    }
}

impl eframe::App for BagItApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending progress updates
        self.process_progress();

        // Request repaint while processing
        if self.progress_rx.is_some() {
            ctx.request_repaint();
        }

        // Handle dropped files
        let dropped_files: Vec<PathBuf> = ctx
            .input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect()
            });

        if let Some(path) = dropped_files.into_iter().next() {
            if path.is_dir() && matches!(self.state, AppState::Idle | AppState::Done { .. } | AppState::Error { .. }) {
                self.start_bagging(path);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // ui.add_space(40.0);

                match &self.state {
                    AppState::Idle => {
                        // ui.heading("Baggie");
                        // ui.add_space(30.0);

                        // Drop zone
                        let drop_zone = egui::Frame::none()
                            .stroke(egui::Stroke::new(2.0, egui::Color32::GRAY))
                            .rounding(10.0)
                            .inner_margin(40.0);

                        drop_zone.show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("📁").size(48.0));
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new("Drop folder here").size(20.0));
                                ui.label("to create a bag");
                                ui.add_space(20.0);

                                if ui.button("Browse...").clicked() {
                                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                        self.start_bagging(path);
                                    }
                                }
                            });
                        });
                    }

                    AppState::Processing {
                        total_files,
                        current,
                        current_file,
                        stage,
                    } => {
                        ui.heading("Processing...");
                        ui.add_space(30.0);

                        ui.label(stage);
                        ui.add_space(10.0);

                        if *total_files > 0 {
                            let progress = *current as f32 / *total_files as f32;
                            ui.add(egui::ProgressBar::new(progress).show_percentage());
                        } else {
                            ui.spinner();
                        }

                        ui.add_space(10.0);

                        if !current_file.is_empty() {
                            ui.label(
                                egui::RichText::new(current_file)
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    }

                    AppState::Done { path, file_count } => {
                        ui.label(egui::RichText::new("✅").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("Bag Created!");
                        ui.add_space(20.0);

                        ui.label(format!("{} files bagged", file_count));
                        ui.add_space(10.0);

                        ui.label(
                            egui::RichText::new(path.to_string_lossy())
                                .small()
                                .color(egui::Color32::GRAY),
                        );

                        ui.add_space(30.0);

                        if ui.button("Bag Another Folder").clicked() {
                            self.state = AppState::Idle;
                        }
                    }

                    AppState::Error { message } => {
                        ui.label(egui::RichText::new("❌").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("Error");
                        ui.add_space(20.0);

                        ui.label(message);

                        ui.add_space(30.0);

                        if ui.button("Try Again").clicked() {
                            self.state = AppState::Idle;
                        }
                    }
                }
            });
        });
    }
}
