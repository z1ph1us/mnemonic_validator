-----

# Mnemonic Validator

A high-performance command-line tool written in Rust to validate BIP39 mnemonic phrases from a given input file and save the valid ones to an output file. It supports automatic checkpoints for seamless recovery and efficient processing of large files.

-----

## Features

  * **BIP39 Validation**: Accurately checks if mnemonic phrases adhere to the BIP39 standard.
  * **Parallel Processing**: Leverages `rayon` for efficient multi-threaded validation, making it fast even for large input files.
  * **Automatic Checkpointing**: Saves progress periodically and upon `Ctrl+C` interruption, allowing you to resume validation from where you left off. The checkpoint file is hidden and stored in your home directory (e.g., `~/.mnemonic_validator_checkpoint.txt`).
  * **Real-time Progress Updates**: Provides live statistics including percentage complete, lines processed, valid mnemonics found, processing speed (lines/s), and estimated time remaining (ETA).
  * **Error Handling**: Gracefully handles file errors and provides informative messages.

-----

## Installation

### Prerequisites

Before you begin, ensure you have **Rust** and **Cargo** (Rust's package manager) installed. If you don't, you can install them by following the instructions on the [official Rust website](https://www.rust-lang.org/tools/install).

### Build from Source

1.  **Clone the repository:**

    ```bash
    git clone https://github.com/z1ph1us/mnemonic-validator.git
    cd mnemonic-validator
    ```

2.  **Build the project:**

    ```bash
    cargo build --release
    ```

    This will create CLI and GUI version executables in the `target/release/` directory.

-----

## Usage

### Basic Usage

Run the validator with default input and output file paths:

```bash
./target/release/mnemonic_validator
```

  * **Input File (default)**: `input/mnemonics.txt`
  * **Output File (default)**: `output/valid_mnemonics.txt`

### Specifying Input and Output Files

You can specify custom input and output file paths using the `-i` or `--input` and `-o` or `--output` flags:

```bash
./target/release/mnemonic_validator -i my_mnemonics.txt -o my_valid_mnemonics.txt
```

-----

### Checkpoints

The script automatically saves a checkpoint to a hidden file in your home directory (e.g., `~/.mnemonic_validator_checkpoint.txt`). If the script is interrupted (e.g., by pressing `Ctrl+C` or a power outage), it will resume from the last saved checkpoint when you run it again with the same input file. Once the validation is complete, the checkpoint file will be automatically removed.

-----

```

### Dependencies

  * `bip39`: For BIP39 mnemonic phrase parsing and validation.
  * `rayon`: For parallel iteration and processing.
  * `clap`: For parsing command-line arguments.
  * `ctrlc`: For handling Ctrl+C signals for graceful exit and checkpointing.
  * `dirs`: For determining user home directory to store checkpoint files.

```

-----

## Support
If you found this project useful and would like to encourage me to create more tools like it, consider donating:

- **Bitcoin (BTC):** `bc1qg7xmlsdxfgu3muxherh4eee2ywj3gz8qfgct3s`  
- **Ethereum (ETH):** `0x1B449E1D545bD0dc50f361d96706F6C904553929`  
- **Solana (SOL):** `F776tt1it7vifCzD9icrsby3ujkZdJ8EY9917GUM3skr`  
- **Tron (TRX):** `TJj96B4SukPJSC4M2FyssoxduVyviv9aGr`  
- **Polygon (POL):** `0x1B449E1D545bD0dc50f361d96706F6C904553929`  
- **Monero (XMR):** `48aaDb1g4Ms7PB3WMj6ttbMWuEwe171d6Yjao59uFJR38tHa75nKwPqYoPAYmWZPUhXmDbDvqtE6d2FX3YaF1dVE7zhD9Dt`  



