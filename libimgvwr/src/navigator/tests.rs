use std::fs::File;

use tempfile::tempdir;

use super::*;

#[cfg(feature = "png")]
fn make_png_files(dir: &std::path::Path, names: &[&str]) {
    for name in names {
        File::create(dir.join(name)).unwrap();
    }
}

#[cfg(feature = "png")]
#[test]
fn scan_dir_sorts_by_filename() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["c.png", "a.png", "b.png"]);
    let nav = Navigator::from_path(dir.path()).unwrap();
    assert_eq!(nav.current().file_name().unwrap(), "a.png");
}

#[cfg(feature = "png")]
#[test]
fn scan_file_starts_at_that_file() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png", "c.png"]);
    let nav = Navigator::from_path(&dir.path().join("b.png")).unwrap();
    assert_eq!(nav.current().file_name().unwrap(), "b.png");
}

#[cfg(feature = "png")]
#[test]
fn next_wraps_around() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png"]);
    let mut nav = Navigator::from_path(dir.path()).unwrap();
    nav.next();
    assert_eq!(nav.current().file_name().unwrap(), "b.png");
    nav.next();
    assert_eq!(nav.current().file_name().unwrap(), "a.png");
}

#[cfg(feature = "png")]
#[test]
fn prev_wraps_around() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png"]);
    let mut nav = Navigator::from_path(dir.path()).unwrap();
    nav.prev();
    assert_eq!(nav.current().file_name().unwrap(), "b.png");
    nav.prev();
    assert_eq!(nav.current().file_name().unwrap(), "a.png");
}

#[test]
fn empty_dir_returns_error() {
    let dir = tempdir().unwrap();
    let result = Navigator::from_path(dir.path());
    assert!(result.is_err());
}

#[cfg(feature = "png")]
#[test]
fn non_image_files_are_excluded() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("readme.txt")).unwrap();
    File::create(dir.path().join("image.png")).unwrap();
    let nav = Navigator::from_path(dir.path()).unwrap();
    assert_eq!(nav.paths.len(), 1);
    assert_eq!(nav.current().file_name().unwrap(), "image.png");
}
