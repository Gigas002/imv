//! Image loading via `image-rs`.
//!
//! Wraps [`image::open`] with a typed error that distinguishes I/O failures,
//! decode failures, and formats not compiled in via Cargo features.

#[cfg(test)]
mod tests;

use std::path::Path;

use image::{DynamicImage, ImageError};

/// Errors that can occur when loading an image.
#[derive(Debug)]
pub enum LoadError {
    /// An OS-level I/O error (file not found, permission denied, etc.).
    Io(std::io::Error),
    /// The file was found but could not be decoded.
    Decode(ImageError),
    /// The format is not supported by the currently enabled Cargo features.
    UnsupportedFormat,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "IO error: {e}"),
            LoadError::Decode(e) => write!(f, "Decode error: {e}"),
            LoadError::UnsupportedFormat => write!(f, "unsupported image format"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Load an image from `path`, returning a [`DynamicImage`] on success.
///
/// Blocking, single-threaded. Only the first frame is loaded for formats that
/// support animation; subsequent frames are ignored.
pub fn load(path: &Path) -> Result<DynamicImage, LoadError> {
    image::open(path).map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        ImageError::Unsupported(_) => LoadError::UnsupportedFormat,
        other => LoadError::Decode(other),
    })
}
