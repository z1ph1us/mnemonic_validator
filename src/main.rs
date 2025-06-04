use bip39::{Mnemonic, Language};
use rayon::prelude::*;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, BufRead, Write, BufReader},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}},
    time::{Instant, Duration},
};
use std::sync::atomic::AtomicBool;
use ctrlc;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(
    name = "mnemonic_validator",
    about = "Validates BIP39 mnemonic phrases from a file.",
    long_about = "Reads mnemonic phrases from an input file, validates them, and writes the valid ones to an output file.  Supports automatic checkpoints and Ctrl+C handling."
)]
struct Cli {
    /// The path to the input file containing mnemonic phrases (one per line).
    #[clap(short, long, value_parser, default_value = "input/mnemonics.txt")]
    input: String,

    /// The path to the output file for valid mnemonic phrases.
    #[clap(short, long, value_parser, default_value = "output/valid_mnemonics.txt")]
    output: String,
}

fn is_valid(mnemonic: &str) -> bool {
    Mnemonic::parse_in_normalized(Language::English, mnemonic).is_ok()
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

fn estimate_remaining(processed: usize, total: usize, elapsed: Duration) -> String {
    if processed == 0 {
        return "Calculating...".to_string();
    }

    let lines_per_sec = processed as f64 / elapsed.as_secs_f64();
    if lines_per_sec < 0.01 {
        return "Calculating...".to_string();
    }

    let remaining_lines = total.saturating_sub(processed);
    let remaining_secs = remaining_lines as f64 / lines_per_sec;

    format_duration(Duration::from_secs_f64(remaining_secs))
}

fn process_file(
    input_path: &Path,
    output_path: &Path,
    checkpoint_path: &Path, // Now always a hidden path
) -> Result<(), Box<dyn std::error::Error>> {
    // Create output directory only in the current working directory
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Create checkpoint directory if it doesn't exist
    if let Some(cp_parent) = checkpoint_path.parent() {
        fs::create_dir_all(cp_parent)?;
    }

    // Load checkpoint
    let checkpoint = if checkpoint_path.exists() {
        fs::read_to_string(checkpoint_path)?.parse().unwrap_or(0)
    } else {
        0
    };

    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut total_lines = 0;
    for _ in reader.lines() {
        total_lines += 1;
    }

    let file = File::open(input_path)?; // reopen
    let reader = BufReader::new(file);

    println!("Total lines: {}, Starting from checkpoint: {}", total_lines, checkpoint);

    let output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(output_path)?;
    let writer = Arc::new(Mutex::new(BufWriter::new(output)));

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let cp_path = checkpoint_path.to_path_buf();

    // Setup current progress tracking for checkpoint on Ctrl+C
    let current_position = Arc::new(AtomicUsize::new(checkpoint));
    let pos_for_handler = current_position.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        let pos = pos_for_handler.load(Ordering::SeqCst);
        println!("\nReceived Ctrl+C! Saving checkpoint at position: {}", pos);
        fs::write(&cp_path, pos.to_string()).expect("Failed to write checkpoint on exit");
        println!("Checkpoint saved. Exiting safely.");
        std::process::exit(0);
    }).expect("Error setting Ctrl+C handler");

    println!("Starting validation process...");
    let start_time = Instant::now();
    let last_status_update = Arc::new(Mutex::new(Instant::now()));

    // Counters for statistics
    let processed = AtomicUsize::new(0);
    let valid_count = AtomicUsize::new(0);

    // Process lines in parallel
    reader
        .lines()
        .enumerate()
        .filter(|(i, _)| *i >= checkpoint && running.load(Ordering::Relaxed))
        .par_bridge()
        .for_each(|(i, result_line)| {
            if !running.load(Ordering::Relaxed) {
                return;
            }

            processed.fetch_add(1, Ordering::Relaxed);
            current_position.store(i, Ordering::SeqCst);
            
             match result_line { // handle the Result from lines()
                Ok(line) => {
                    if is_valid(&line) {
                        let mut w = writer.lock().unwrap();
                        writeln!(w, "{}", line).expect("Failed to write");
                        valid_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading line {}: {}", i, e);
                }
            }

            // Update checkpoint every 10000 lines
            if i % 10000 == 0 && i > checkpoint {
                // Write checkpoint
                fs::write(checkpoint_path, i.to_string()).expect("Checkpoint update failed");

                // Only update status every 3 seconds to reduce terminal spam
                let mut last_update = last_status_update.lock().unwrap();
                if last_update.elapsed() >= Duration::from_secs(3) {
                    // Get statistics
                    let elapsed = start_time.elapsed();
                    let valid = valid_count.load(Ordering::Relaxed);
                    let proc = processed.load(Ordering::Relaxed);
                    let percent_done = (i * 100) / total_lines.max(1);
                    let speed = if elapsed.as_secs() > 0 { proc / elapsed.as_secs() as usize } else { proc };
                    let eta = estimate_remaining(proc, total_lines, elapsed);

                    // Clear previous line and print progress
                    print!("\r\x1B[K");
                    print!(
                        "[{:3}%] {}/{} lines, {} valid, {} lines/s, ETA: {}",
                        percent_done,
                        i,
                        total_lines,
                        valid,
                        speed,
                        eta
                    );
                    std::io::stdout().flush().expect("Failed to flush stdout");

                    *last_update = Instant::now();
                }
            }
        });

    // Final statistics
    let elapsed = start_time.elapsed();
    let valid = valid_count.load(Ordering::Relaxed);
    let processed_total = processed.load(Ordering::Relaxed);

    println!("\nValidation complete!");
    println!("Valid mnemonics found: {}", valid);
    println!("Time taken: {}", format_duration(elapsed));
    println!("Processing speed: {} lines/s", if elapsed.as_secs() > 0 { processed_total / elapsed.as_secs() as usize } else { processed_total });
    println!("Made by z1ph1us.");
    // Final checkpoint update
    fs::write(checkpoint_path, total_lines.to_string())?;
    
    //remove checkpoint file.
    fs::remove_file(checkpoint_path)?;

    // Make sure we've written everything
    writer.lock().unwrap().flush()?;

    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let input_path = Path::new(&cli.input);
    let output_path = Path::new(&cli.output);

    // Construct the checkpoint path in the user's home directory as a hidden file.
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")); // Use current dir if home dir is not found.
    let checkpoint_path = home_dir.join(".mnemonic_validator_checkpoint.txt");
    

    if !input_path.exists() {
        eprintln!("Error: Input file not found at '{}'", input_path.display());
        std::process::exit(1);
    }

    if let Err(e) = process_file(input_path, output_path, &checkpoint_path) {
        eprintln!("\nError: {}", e);
        std::process::exit(1);
    }
}
