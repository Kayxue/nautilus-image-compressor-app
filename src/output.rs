//! Output handling: where a processed image is written, how it is encoded,
//! and what happens to the original file afterwards.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat};

use crate::{compress_mode::CompressMode, compressor::compress_image};

/// Describes where a processed image is written and what happens to the
/// original file.
///
/// | Mode             | Resize / Rotate                    | Convert                                   |
/// |------------------|------------------------------------|-------------------------------------------|
/// | `InPlace`        | overwrite `photo.jpg`              | write `photo.png`, remove `photo.jpg`     |
/// | `NewFile(sfx)`   | write `photo{sfx}.jpg`, keep orig. | write `photo{sfx}.png`, keep `photo.jpg`  |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputMode {
    /// Replace the original file. When the extension changes (format
    /// conversion), the new file is written next to the original and the
    /// original is removed after a successful save.
    InPlace,
    /// Keep the original file and write a new one next to it, with the given
    /// suffix appended to the file stem (the suffix may be empty).
    NewFile(String),
}

impl OutputMode {
    /// Computes the destination path for `original`.
    ///
    /// `target_ext` replaces the file extension when given (format
    /// conversion); otherwise the original extension is kept.
    pub fn destination(&self, original: &Path, target_ext: Option<&str>) -> PathBuf {
        match self {
            OutputMode::InPlace => match target_ext {
                Some(ext) => original.with_extension(ext),
                None => original.to_path_buf(),
            },
            OutputMode::NewFile(suffix) => {
                let parent = original.parent().unwrap_or_else(|| Path::new(""));
                let file_stem = original
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("image");
                let extension =
                    target_ext.or_else(|| original.extension().and_then(|e| e.to_str()));
                let new_filename = match extension {
                    Some(ext) => format!("{}{}.{}", file_stem, suffix, ext),
                    None => format!("{}{}", file_stem, suffix),
                };
                parent.join(new_filename)
            }
        }
    }

    /// Whether the original file must be removed once `dest` was written.
    pub fn removes_original(&self, original: &Path, dest: &Path) -> bool {
        matches!(self, OutputMode::InPlace) && original != dest
    }
}

/// Compresses `img` according to `mode` and writes it to `dest`, choosing the
/// encoder from the file extension (or `fallback_format` if provided).
pub fn save_image(
    img: &DynamicImage,
    dest: &Path,
    mode: CompressMode,
    fallback_format: Option<ImageFormat>,
) -> Result<()> {
    let format = ImageFormat::from_path(dest)
        .ok()
        .or(fallback_format)
        .with_context(|| format!("Unsupported output format: {:?}", dest))?;

    let bytes = compress_image(img, format, mode)
        .with_context(|| format!("Failed to compress image for {:?}", dest))?;

    std::fs::write(dest, bytes)
        .with_context(|| format!("Failed to write compressed image to {:?}", dest))
}
