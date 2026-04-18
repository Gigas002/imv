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
    #[cfg(feature = "jxl")]
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        == Some("jxl")
    {
        return load_jxl(path);
    }

    image::open(path).map_err(|e| match e {
        ImageError::IoError(io_err) => LoadError::Io(io_err),
        ImageError::Unsupported(_) => LoadError::UnsupportedFormat,
        other => LoadError::Decode(other),
    })
}

#[cfg(feature = "jxl")]
fn load_jxl(path: &Path) -> Result<DynamicImage, LoadError> {
    use image::{ImageBuffer, Rgba};
    use jxl::api::states::Initialized;
    use jxl::api::{
        JxlDecoder, JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat, ProcessingResult,
    };

    let file_bytes = std::fs::read(path).map_err(LoadError::Io)?;
    let options = JxlDecoderOptions::default();

    // Phase 1 — parse image header → get dimensions
    let mut decoder = JxlDecoder::<Initialized>::new(options);
    let mut input = file_bytes.as_slice();
    let mut decoder_info = loop {
        match decoder.process(&mut input).map_err(jxl_err)? {
            ProcessingResult::Complete { result } => break result,
            ProcessingResult::NeedsMoreInput { fallback, .. } => decoder = fallback,
        }
    };

    let (width, height) = decoder_info.basic_info().size;
    decoder_info.set_pixel_format(JxlPixelFormat::rgba8(0));

    // Phase 2 — parse frame header
    let mut decoder_frame = loop {
        match decoder_info.process(&mut input).map_err(jxl_err)? {
            ProcessingResult::Complete { result } => break result,
            ProcessingResult::NeedsMoreInput { fallback, .. } => decoder_info = fallback,
        }
    };

    // Phase 3 — decode first frame into an RGBA u8 buffer
    let stride = width * 4;
    let mut pixel_buf = vec![0u8; height * stride];
    loop {
        let out = JxlOutputBuffer::new(&mut pixel_buf, height, stride);
        match decoder_frame
            .process(&mut input, &mut [out])
            .map_err(jxl_err)?
        {
            ProcessingResult::Complete { .. } => break,
            ProcessingResult::NeedsMoreInput { fallback, .. } => decoder_frame = fallback,
        }
    }

    ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, pixel_buf)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| {
            use image::error::{DecodingError, ImageFormatHint};
            LoadError::Decode(ImageError::Decoding(DecodingError::new(
                ImageFormatHint::Name("JXL".to_owned()),
                "buffer size mismatch",
            )))
        })
}

#[cfg(feature = "jxl")]
fn jxl_err(e: jxl::error::Error) -> LoadError {
    use image::error::{DecodingError, ImageFormatHint};
    LoadError::Decode(ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("JXL".to_owned()),
        e.to_string(),
    )))
}
