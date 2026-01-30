use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use walkdir::WalkDir;

#[derive(Debug)]
pub enum BagError {
    NotADirectory,
    IoError(io::Error),
    AlreadyABag,
}

impl std::fmt::Display for BagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BagError::NotADirectory => write!(f, "Path is not a directory"),
            BagError::IoError(e) => write!(f, "IO error: {}", e),
            BagError::AlreadyABag => write!(f, "Directory appears to already be a bag"),
        }
    }
}

impl From<io::Error> for BagError {
    fn from(e: io::Error) -> Self {
        BagError::IoError(e)
    }
}


#[derive(Debug, Clone)]
pub enum Progress {
    Started { total_files: usize },
    Moving { current: usize, filename: String },
    Checksumming { current: usize, filename: String },
    Done { path: PathBuf },
    Error { message: String },
}

fn calculate_sha256(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn calculate_sha256_str(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn bag_directory(path: &Path, progress_tx: Option<Sender<Progress>>) -> Result<(), BagError> {
    // Validate input
    if !path.is_dir() {
        return Err(BagError::NotADirectory);
    }

    // Check if already a bag
    if path.join("bagit.txt").exists() || path.join("data").exists() {
        return Err(BagError::AlreadyABag);
    }

    // Count files first
    let entries: Vec<_> = WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    let total_files = entries.iter().filter(|e| e.file_type().is_file()).count();

    if let Some(ref tx) = progress_tx {
        let _ = tx.send(Progress::Started { total_files });
    }

    // Create data directory
    let data_dir = path.join("data");
    fs::create_dir(&data_dir)?;

    // Get list of items to move (top-level only)
    let items_to_move: Vec<_> = fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != "data")
        .collect();

    // Move all items into data/
    for (i, entry) in items_to_move.iter().enumerate() {
        let filename = entry.file_name();
        let dest = data_dir.join(&filename);

        if let Some(ref tx) = progress_tx {
            let _ = tx.send(Progress::Moving {
                current: i + 1,
                filename: filename.to_string_lossy().to_string(),
            });
        }

        fs::rename(entry.path(), dest)?;
    }

    // Calculate checksums for all files in data/
    let mut manifest_entries = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut file_count: usize = 0;

    let data_files: Vec<_> = WalkDir::new(&data_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    for (i, entry) in data_files.iter().enumerate() {
        let file_path = entry.path();
        let relative_path = file_path.strip_prefix(path).unwrap();

        if let Some(ref tx) = progress_tx {
            let _ = tx.send(Progress::Checksumming {
                current: i + 1,
                filename: relative_path.to_string_lossy().to_string(),
            });
        }

        let checksum = calculate_sha256(file_path)?;
        let metadata = fs::metadata(file_path)?;
        total_bytes += metadata.len();
        file_count += 1;

        // Use forward slashes for manifest (BagIt spec)
        let manifest_path = relative_path.to_string_lossy().replace('\\', "/");
        manifest_entries.push(format!("{}  {}", checksum, manifest_path));
    }

    // Write bagit.txt
    let bagit_content = "BagIt-Version: 0.97\nTag-File-Character-Encoding: UTF-8\n";
    fs::write(path.join("bagit.txt"), bagit_content)?;

    // Write manifest-sha256.txt (sorted for reproducibility, matching Python bagit)
    manifest_entries.sort();
    let manifest_content = manifest_entries.join("\n") + "\n";
    fs::write(path.join("manifest-sha256.txt"), &manifest_content)?;

    // Write bag-info.txt (field order matches Python bagit library)
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let payload_oxum = format!("{}.{}", total_bytes, file_count);
    let bag_info_content = format!(
        "Bag-Software-Agent: baggie 0.1.1\nBagging-Date: {}\nPayload-Oxum: {}\n",
        date, payload_oxum
    );
    fs::write(path.join("bag-info.txt"), &bag_info_content)?;

    // Write tagmanifest-sha256.txt (sorted alphabetically to match Python bagit)
    let bagit_checksum = calculate_sha256_str(bagit_content);
    let manifest_checksum = calculate_sha256_str(&manifest_content);
    let bag_info_checksum = calculate_sha256_str(&bag_info_content);

    let mut tagmanifest_entries = vec![
        format!("{}  bag-info.txt", bag_info_checksum),
        format!("{}  bagit.txt", bagit_checksum),
        format!("{}  manifest-sha256.txt", manifest_checksum),
    ];
    tagmanifest_entries.sort_by(|a, b| {
        // Sort by filename (after the checksum and spaces)
        a.split_whitespace().last().cmp(&b.split_whitespace().last())
    });
    let tagmanifest_content = tagmanifest_entries.join("\n") + "\n";
    fs::write(path.join("tagmanifest-sha256.txt"), tagmanifest_content)?;

    if let Some(ref tx) = progress_tx {
        let _ = tx.send(Progress::Done {
            path: path.to_path_buf(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_bag_directory() {
        // Create a temporary directory
        let temp_dir = std::env::temp_dir().join("bagit_test");
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).unwrap();
        }
        fs::create_dir(&temp_dir).unwrap();

        // Create test files
        fs::write(temp_dir.join("file1.txt"), "Hello, world!").unwrap();
        fs::write(temp_dir.join("file2.txt"), "Test content").unwrap();
        fs::create_dir(temp_dir.join("subdir")).unwrap();
        fs::write(temp_dir.join("subdir").join("nested.txt"), "Nested file").unwrap();

        // Bag the directory
        bag_directory(&temp_dir, None).unwrap();

        // Verify structure
        assert!(temp_dir.join("data").exists());
        assert!(temp_dir.join("data").join("file1.txt").exists());
        assert!(temp_dir.join("data").join("file2.txt").exists());
        assert!(temp_dir.join("data").join("subdir").join("nested.txt").exists());
        assert!(temp_dir.join("bagit.txt").exists());
        assert!(temp_dir.join("manifest-sha256.txt").exists());
        assert!(temp_dir.join("bag-info.txt").exists());
        assert!(temp_dir.join("tagmanifest-sha256.txt").exists());

        // Verify bagit.txt content
        let bagit_content = fs::read_to_string(temp_dir.join("bagit.txt")).unwrap();
        assert!(bagit_content.contains("BagIt-Version: 0.97"));
        assert!(bagit_content.contains("Tag-File-Character-Encoding: UTF-8"));

        // Verify manifest has correct format and file count
        let manifest = fs::read_to_string(temp_dir.join("manifest-sha256.txt")).unwrap();
        assert!(manifest.contains("data/file1.txt"));
        assert!(manifest.contains("data/file2.txt"));
        assert!(manifest.contains("data/subdir/nested.txt"));
        assert_eq!(manifest.lines().count(), 3);

        // Cleanup
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_already_a_bag() {
        let temp_dir = std::env::temp_dir().join("bagit_test_already_bag");
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).unwrap();
        }
        fs::create_dir(&temp_dir).unwrap();
        fs::write(temp_dir.join("bagit.txt"), "existing").unwrap();

        let result = bag_directory(&temp_dir, None);
        assert!(matches!(result, Err(BagError::AlreadyABag)));

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
