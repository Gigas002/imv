use std::io::Write;

use tempfile::Builder;

use super::*;

#[cfg(feature = "png")]
#[test]
fn load_png_4x4() {
    let png_bytes = include_bytes!("../../tests/fixtures/4x4.png");
    let mut tmp = Builder::new().suffix(".png").tempfile().unwrap();
    tmp.write_all(png_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[test]
fn load_unsupported_format() {
    let mut tmp = Builder::new().suffix(".xyz").tempfile().unwrap();
    tmp.write_all(b"not an image").unwrap();
    let result = load(tmp.path());
    assert!(matches!(result, Err(LoadError::UnsupportedFormat)));
}
