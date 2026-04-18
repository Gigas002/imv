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

#[cfg(feature = "jpeg")]
#[test]
fn load_jpeg_4x4() {
    let jpeg_bytes = include_bytes!("../../tests/fixtures/4x4.jpg");
    let mut tmp = Builder::new().suffix(".jpg").tempfile().unwrap();
    tmp.write_all(jpeg_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "webp")]
#[test]
fn load_webp_4x4() {
    let webp_bytes = include_bytes!("../../tests/fixtures/4x4.webp");
    let mut tmp = Builder::new().suffix(".webp").tempfile().unwrap();
    tmp.write_all(webp_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "avif")]
#[test]
fn load_avif_4x4() {
    let avif_bytes = include_bytes!("../../tests/fixtures/4x4.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(avif_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "jxl")]
#[test]
fn load_jxl_4x4() {
    let jxl_bytes = include_bytes!("../../tests/fixtures/4x4.jxl");
    let mut tmp = Builder::new().suffix(".jxl").tempfile().unwrap();
    tmp.write_all(jxl_bytes).unwrap();
    let img = load(tmp.path()).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[cfg(feature = "gif")]
#[test]
fn load_gif_animated_4x4() {
    let gif_bytes = include_bytes!("../../tests/fixtures/4x4_anim.gif");
    let mut tmp = Builder::new().suffix(".gif").tempfile().unwrap();
    tmp.write_all(gif_bytes).unwrap();
    let result = super::load_gif_frames(tmp.path()).unwrap();
    assert_eq!(result.frames.len(), 2);
    assert_eq!(result.frames[0].0.width(), 4);
    assert_eq!(result.frames[0].0.height(), 4);
}

#[cfg(feature = "jxl-anim")]
#[test]
fn load_jxl_anim_4x4() {
    let jxl_bytes = include_bytes!("../../tests/fixtures/4x4_anim.jxl");
    let mut tmp = Builder::new().suffix(".jxl").tempfile().unwrap();
    tmp.write_all(jxl_bytes).unwrap();
    let result = super::load_jxl_anim_frames(tmp.path()).unwrap();
    assert!(!result.frames.is_empty());
    assert_eq!(result.frames[0].0.width(), 4);
    assert_eq!(result.frames[0].0.height(), 4);
}

#[cfg(feature = "avif-anim")]
#[test]
fn load_avif_anim_4x4() {
    let avif_bytes = include_bytes!("../../tests/fixtures/4x4_anim.avif");
    let mut tmp = Builder::new().suffix(".avif").tempfile().unwrap();
    tmp.write_all(avif_bytes).unwrap();
    let result = super::load_avif_anim_frames(tmp.path()).unwrap();
    assert!(!result.frames.is_empty());
    assert_eq!(result.frames[0].0.width(), 4);
    assert_eq!(result.frames[0].0.height(), 4);
}

#[test]
fn load_unsupported_format() {
    let mut tmp = Builder::new().suffix(".xyz").tempfile().unwrap();
    tmp.write_all(b"not an image").unwrap();
    let result = load(tmp.path());
    assert!(matches!(result, Err(LoadError::UnsupportedFormat)));
}
