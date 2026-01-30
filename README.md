# Baggie

A minimal desktop application for creating BagIt bags from folders.

## Features

- Drag and drop folders to bag them
- SHA-256 checksums for all payload files
- Creates valid BagIt 1.0 format bags with:
  - `bagit.txt` - version declaration
  - `manifest-sha256.txt` - payload checksums
  - `bag-info.txt` - bag metadata (date, software agent, payload oxum)
  - `tagmanifest-sha256.txt` - tag file checksums

## Installation

Requires Rust. Then:

```
cargo build --release
```

The binary will be at `target/release/baggie`.

## Usage

1. Run the application
2. Drag a folder onto the window, or click "Browse..." to select one
3. The folder will be converted to a bag in-place

## License

MIT
