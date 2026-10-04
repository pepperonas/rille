//! Find audio files below a folder.

use std::path::{Path, PathBuf};

/// Extensions rille can decode (ALAC lives in `.m4a`).
const AUDIO_EXTENSIONS: [&str; 7] = ["mp3", "m4a", "aac", "flac", "wav", "aif", "aiff"];
/// Deeper than any sane music folder; guards against pathological trees.
const MAX_DEPTH: usize = 32;

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// All audio files below `root`, sorted. Hidden files and folders (and macOS `._` stubs) are
/// skipped, symbolic links to folders are not followed (no loops), unreadable folders are
/// passed over silently.
pub fn scan(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, 0, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = entry
            .file_name()
            .to_str()
            .is_none_or(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        // `file_type` does not follow symlinks: a linked folder is neither dir nor file here.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            walk(&path, depth + 1, found);
        } else if (kind.is_file() || kind.is_symlink() && path.is_file()) && is_audio_file(&path) {
            found.push(path);
        }
    }
}
