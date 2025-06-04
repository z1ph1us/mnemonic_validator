use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use bip39::{Language, Mnemonic};
use crossbeam_channel::{unbounded, Receiver};
use eframe::{egui, NativeOptions};
use rfd::FileDialog;

#[derive(Clone, Debug)]
struct ProgressUpdate {
    processed: usize,
    valid: usize,
    total: usize,
    speed: usize,
    eta: String,
    status: String,
}

struct AppState {
    input_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    
    is_running: bool,
    cancel_flag: Arc<AtomicBool>,
    
    progress: ProgressUpdate,
    progress_rx: Option<Receiver<ProgressUpdate>>,
    
    // UI state
    show_help: bool,
    auto_output: bool,
}

impl Default for AppState {
    fn default() -> Self {
        // Try to find default input files (now checks any file name)
        let default_input = ["wordlist", "mnemonics", "seeds", "input"]
            .iter()
            .flat_map(|name| {
                vec![
                    PathBuf::from(format!("{}.txt", name)),
                    PathBuf::from(name.to_string()),
                ]
            })
            .find(|p| p.exists());
            
        Self {
            input_path: default_input,
            output_path: None,
            is_running: false,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            progress: ProgressUpdate {
                processed: 0,
                valid: 0,
                total: 0,
                speed: 0,
                eta: "-".to_string(),
                status: String::new(),
            },
            progress_rx: None,
            show_help: false,
            auto_output: true,
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Clean, professional styling
        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::new(24.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Body, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Monospace, egui::FontId::new(14.0, egui::FontFamily::Monospace)),
            (egui::TextStyle::Button, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Small, egui::FontId::new(12.0, egui::FontFamily::Proportional)),
        ].into();
        ctx.set_style(style);
        
        // Help dialog
        if self.show_help {
            egui::Window::new("Help")
                .collapsible(false)
                .resizable(false)
                .default_width(500.0)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.label("This tool validates BIP39 mnemonic seed phrases from any text file.");
                        ui.label("Input: File containing one mnemonic phrase per line");
                        ui.label("Output: File containing only valid mnemonics");
                        
                        ui.add_space(10.0);
                        
                        ui.label("Features:");
                        ui.indent("features", |ui| {
                            ui.label("• Works with any file format (txt, csv, dat, etc.)");
                            ui.label("• Validates 12/15/18/21/24-word mnemonics");
                            ui.label("• Fast processing with progress tracking");
                            ui.label("• Cancellable operation");
                        });
                        
                        ui.add_space(15.0);
                        
                        if ui.button("Close").clicked() {
                            self.show_help = false;
                        }
                    });
                });
        }
        
        // Receive progress updates if any
        let mut should_clear_rx = false;
        if let Some(rx) = &self.progress_rx {
            while let Ok(update) = rx.try_recv() {
                self.progress = update.clone();
                if self.progress.status == "Done." || self.progress.status == "Cancelled." {
                    self.is_running = false;
                    should_clear_rx = true;
                }
            }
        }
        if should_clear_rx {
            self.progress_rx = None;
        }

        // Main panel with clean layout
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 16.0;
            ui.spacing_mut().button_padding = egui::vec2(12.0, 8.0);
            
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                
                // Header
                ui.heading("Mnemonic Validator by z1ph1us");
                ui.separator();
                
                ui.add_space(10.0);
                
                // Input section
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Input File:");
                        if let Some(path) = &self.input_path {
                            ui.label(path.display().to_string());
                        } else {
                            ui.label("No file selected");
                        }
                    });
                    
                    if ui.button("Browse...").clicked() {
                        let current_dir = std::env::current_dir().unwrap_or_default();
                        if let Some(path) = FileDialog::new()
                            .set_directory(&current_dir)
                            .add_filter("All Files", &["*"])
                            .add_filter("Text Files", &["txt", "csv", "dat", "log"])
                            .pick_file() 
                        {
                            self.input_path = Some(path);
                            if self.auto_output {
                                self.update_auto_output();
                            }
                        }
                    }
                });
                
                ui.add_space(10.0);
                
                // Output section
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.auto_output, "Auto output");
                    
                    if !self.auto_output {
                        ui.vertical(|ui| {
                            ui.label("Output File:");
                            if let Some(path) = &self.output_path {
                                ui.label(path.display().to_string());
                            } else {
                                ui.label("No file selected");
                            }
                        });
                        
                        if ui.button("Browse...").clicked() {
                            let current_dir = std::env::current_dir().unwrap_or_default();
                            if let Some(path) = FileDialog::new()
                                .set_directory(&current_dir)
                                .add_filter("Text Files", &["txt"])
                                .save_file()
                            {
                                self.output_path = Some(path);
                            }
                        }
                    }
                });
                
                ui.add_space(20.0);
                
                // Action buttons
                ui.horizontal(|ui| {
                    let can_start = self.input_path.is_some() && (self.auto_output || self.output_path.is_some());
                    
                    if !self.is_running {
                        if ui.add_enabled(can_start, egui::Button::new("Start Validation")).clicked() {
                            if self.auto_output {
                                self.update_auto_output();
                            }
                            self.start_validation();
                        }
                    } else {
                        if ui.button("Cancel").clicked() {
                            self.cancel_flag.store(true, Ordering::SeqCst);
                            self.progress.status = "Cancelling...".to_string();
                        }
                    }
                    
                    if ui.button("Help").clicked() {
                        self.show_help = true;
                    }
                });
                
                ui.add_space(20.0);
                
                // Progress section
                if self.is_running || self.progress.processed > 0 {
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            // Progress bar
                            let frac = if self.progress.total > 0 {
                                self.progress.processed as f32 / self.progress.total as f32
                            } else {
                                0.0
                            };
                            
                            ui.add(egui::ProgressBar::new(frac).show_percentage());
                            
                            // Stats
                            ui.horizontal(|ui| {
                                ui.label(format!("Processed: {}/{}", self.progress.processed, self.progress.total));
                                ui.label(format!("Valid: {}", self.progress.valid));
                            });
                            
                            ui.horizontal(|ui| {
                                ui.label(format!("Speed: {}/sec", self.progress.speed));
                                ui.label(format!("ETA: {}", self.progress.eta));
                            });
                            
                            if !self.progress.status.is_empty() {
                                ui.label(&self.progress.status);
                            }
                        });
                    });
                }
                
                if !self.is_running && self.progress.processed > 0 && self.progress.status == "Done." {
                    ui.add_space(10.0);
                    ui.label(format!("Done. Found {} valid mnemonics.", self.progress.valid));
                }
            });
        });

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl AppState {
    fn update_auto_output(&mut self) {
        if let Some(input_path) = &self.input_path {
            // Create output directory if it doesn't exist
            let output_dir = Path::new("output");
            if !output_dir.exists() {
                fs::create_dir_all(output_dir).ok();
            }
            
            let mut output_path = output_dir.to_path_buf();
            let stem = input_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            output_path.push(format!("{}_valid.txt", stem));
            self.output_path = Some(output_path);
        }
    }
    
    fn start_validation(&mut self) {
        self.is_running = true;
        self.cancel_flag.store(false, Ordering::SeqCst);

        let input_path = self.input_path.clone().unwrap();
        let output_path = self.output_path.clone().unwrap();
        let cancel_flag = self.cancel_flag.clone();

        let (tx, rx) = unbounded();
        self.progress_rx = Some(rx);

        thread::spawn(move || {
            // Read the entire file first to handle any encoding
            let content = match std::fs::read_to_string(&input_path) {
                Ok(content) => content,
                Err(e) => {
                    let _ = tx.send(ProgressUpdate {
                        processed: 0,
                        valid: 0,
                        total: 0,
                        speed: 0,
                        eta: "-".to_string(),
                        status: format!("Error reading file: {}", e),
                    });
                    return;
                }
            };
            
            let total_lines = content.lines().count();

            let output_file = match OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&output_path)
            {
                Ok(file) => file,
                Err(e) => {
                    let _ = tx.send(ProgressUpdate {
                        processed: 0,
                        valid: 0,
                        total: 0,
                        speed: 0,
                        eta: "-".to_string(),
                        status: format!("Failed to create output file: {}", e),
                    });
                    return;
                }
            };
            
            let mut writer = BufWriter::new(output_file);

            let start_time = Instant::now();
            let mut processed = 0usize;
            let mut valid = 0usize;

            for line in content.lines() {
                if cancel_flag.load(Ordering::SeqCst) {
                    let _ = tx.send(ProgressUpdate {
                        processed,
                        valid,
                        total: total_lines,
                        speed: 0,
                        eta: "-".to_string(),
                        status: "Cancelled.".to_string(),
                    });
                    return;
                }

                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                processed += 1;

                if Mnemonic::parse_in_normalized(Language::English, line).is_ok() {
                    valid += 1;
                    if let Err(e) = writeln!(writer, "{}", line) {
                        let _ = tx.send(ProgressUpdate {
                            processed,
                            valid,
                            total: total_lines,
                            speed: 0,
                            eta: "-".to_string(),
                            status: format!("Error writing output: {}", e),
                        });
                        return;
                    }
                }

                if processed % 200 == 0 || processed == total_lines {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.0 { (processed as f64 / elapsed) as usize } else { 0 };
                    let remaining = total_lines.saturating_sub(processed);
                    let eta_secs = if speed > 0 { remaining / speed } else { 0 };
                    let eta = format!("{:02}:{:02}", eta_secs / 60, eta_secs % 60);

                    let _ = tx.send(ProgressUpdate {
                        processed,
                        valid,
                        total: total_lines,
                        speed,
                        eta,
                        status: "Processing...".to_string(),
                    });
                }
            }

            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 { (processed as f64 / elapsed) as usize } else { 0 };

            let _ = tx.send(ProgressUpdate {
                processed,
                valid,
                total: total_lines,
                speed,
                eta: "00:00".to_string(),
                status: "Done.".to_string(),
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 400.0])
            .with_min_inner_size([500.0, 300.0])
            .with_resizable(true),
        ..Default::default()
    };
    
    eframe::run_native(
        "Mnemonic Validator | by z1ph1us",
        options,
        Box::new(|_cc| Box::new(AppState::default())),
    )
}