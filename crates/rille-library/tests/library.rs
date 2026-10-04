#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code

use std::path::PathBuf;

use rille_library::scan::{is_audio_file, scan};
use rille_library::tags::read_info;
use tempfile::TempDir;

mod common;
use common::{fixture, sine, write_wav, write_wav_tagged};

#[test]
fn scan_finds_audio_recursively_and_skips_the_rest() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::create_dir_all(root.join(".hidden")).unwrap();
    for f in [
        "one.mp3",
        "a/two.FLAC",
        "a/b/three.m4a",
        "a/b/four.aiff",
        "a/notes.txt",
        "cover.jpg",
        ".hidden/five.mp3",
        "a/._six.wav", // macOS resource fork stub
    ] {
        std::fs::write(root.join(f), b"x").unwrap();
    }
    let found: Vec<PathBuf> = scan(root)
        .into_iter()
        .map(|p| p.strip_prefix(root).unwrap().to_path_buf())
        .collect();
    assert_eq!(
        found,
        ["a/b/four.aiff", "a/b/three.m4a", "a/two.FLAC", "one.mp3"].map(PathBuf::from)
    );
}

#[test]
fn scan_does_not_follow_directory_links_in_circles() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("x")).unwrap();
    std::fs::write(dir.path().join("x/song.wav"), b"x").unwrap();
    std::os::unix::fs::symlink(dir.path(), dir.path().join("x/loop")).unwrap();
    assert_eq!(scan(dir.path()).len(), 1);
}

#[test]
fn scan_of_a_missing_folder_is_empty() {
    assert!(scan(std::path::Path::new("/nonexistent/rille")).is_empty());
}

#[test]
fn audio_extensions() {
    for ok in [
        "a.mp3", "a.M4A", "a.aac", "a.flac", "a.wav", "a.aif", "a.aiff",
    ] {
        assert!(is_audio_file(std::path::Path::new(ok)), "{ok}");
    }
    for no in ["a.ogg", "a.txt", "mp3", "a."] {
        assert!(!is_audio_file(std::path::Path::new(no)), "{no}");
    }
}

#[test]
fn tags_are_read_from_the_file() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "whatever.wav");
    write_wav_tagged(&path, 44_100, &sine(44_100, 440.0, 2.0, 2), "Opal", "Bicep");
    let info = read_info(&path).unwrap();
    assert_eq!(info.title, "Opal");
    assert_eq!(info.artist.as_deref(), Some("Bicep"));
    assert!((info.duration_secs.unwrap() - 2.0).abs() < 0.01);
}

#[test]
fn file_name_stands_in_for_missing_tags() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "Fleetwood Mac - Dreams (2004 Remaster).wav");
    write_wav(&path, 48_000, 2, &sine(48_000, 440.0, 0.5, 2), None);
    let info = read_info(&path).unwrap();
    assert_eq!(info.title, "Dreams (2004 Remaster)");
    assert_eq!(info.artist.as_deref(), Some("Fleetwood Mac"));

    let path = fixture(&dir, "untitled take 3.wav");
    write_wav(&path, 48_000, 2, &sine(48_000, 440.0, 0.5, 2), None);
    let info = read_info(&path).unwrap();
    assert_eq!(info.title, "untitled take 3");
    assert_eq!(info.artist, None);
}

#[test]
fn unreadable_file_reports_an_error() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "fake.mp3");
    std::fs::write(&path, b"definitely not audio").unwrap();
    assert!(read_info(&path).is_err());
}
