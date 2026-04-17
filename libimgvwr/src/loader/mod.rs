#[cfg(test)]
mod tests;

use std::path::Path;

use image::{DynamicImage, ImageError};

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Decode(ImageError),
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

pub fn load(path: &Path) -> Result<DynamicImage, LoadError> {
    image::open(path).map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        ImageError::Unsupported(_) => LoadError::UnsupportedFormat,
        other => LoadError::Decode(other),
    })
}
