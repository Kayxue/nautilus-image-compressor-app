use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageFormat};

use crate::compress_mode::CompressMode;

/// Compresses `img` into the encoded byte representation of `format` using `mode`.
pub fn compress_image(
    img: &DynamicImage,
    format: ImageFormat,
    mode: CompressMode,
) -> Result<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => compress_jpeg(img, mode),
        ImageFormat::Png => compress_png(img, mode),
        ImageFormat::WebP => compress_webp(img, mode),
        ImageFormat::Avif => compress_avif(img, mode),
        other => bail!("Unsupported image format for compression: {:?}", other),
    }
}

fn compress_jpeg(img: &DynamicImage, mode: CompressMode) -> Result<Vec<u8>> {
    let preset = match mode {
        CompressMode::Balanced => mozjpeg_rs::Preset::ProgressiveBalanced,
        CompressMode::Ultra => mozjpeg_rs::Preset::ProgressiveSmallest,
    };
    let rgb = img.to_rgb8();
    mozjpeg_rs::Encoder::new(preset)
        .quality(mode.jpeg_quality())
        .encode_rgb(rgb.as_raw(), rgb.width(), rgb.height())
        .context("Failed to compress JPEG with mozjpeg-rs")
}

fn compress_png(img: &DynamicImage, mode: CompressMode) -> Result<Vec<u8>> {
    use rgb::FromSlice;

    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let pixels: &[rgb::RGBA8] = rgba.as_raw().as_rgba();

    let (min_quality, max_quality) = mode.png_quality_bounds();
    let mut liq = imagequant::new();
    liq.set_quality(min_quality, max_quality)
        .context("Failed to configure imagequant quality")?;

    let mut iq_image = liq
        .new_image_borrowed(pixels, width as usize, height as usize, 0.0)
        .context("Failed to create imagequant image")?;

    let raw_image = match liq.quantize(&mut iq_image) {
        Ok(mut quantized) => {
            quantized
                .set_dithering_level(1.0)
                .context("Failed to set imagequant dithering level")?;
            let (palette, indexed_pixels) = quantized
                .remapped(&mut iq_image)
                .context("Failed to remap quantized PNG pixels")?;
            oxipng::RawImage::new(
                width,
                height,
                oxipng::ColorType::Indexed { palette },
                oxipng::BitDepth::Eight,
                indexed_pixels,
            )
            .context("Failed to create indexed RawImage for oxipng")?
        }
        Err(imagequant::Error::QualityTooLow) => {
            drop(iq_image);
            oxipng::RawImage::new(
                width,
                height,
                oxipng::ColorType::RGBA,
                oxipng::BitDepth::Eight,
                rgba.into_raw(),
            )
            .context("Failed to create RGBA RawImage for oxipng")?
        }
        Err(e) => return Err(e).context("Failed to quantize PNG with imagequant"),
    };

    let opts = oxipng::Options::from_preset(mode.oxipng_preset());
    raw_image
        .create_optimized_png(&opts)
        .context("Failed to optimize PNG with oxipng")
}

fn compress_webp(img: &DynamicImage, mode: CompressMode) -> Result<Vec<u8>> {
    let quality = mode.webp_quality();
    let webp_memory = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height())
            .encode_simple(false, quality)
            .map_err(|e| anyhow::anyhow!("Failed to encode RGBA WebP: {:?}", e))?
    } else {
        let rgb = img.to_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height())
            .encode_simple(false, quality)
            .map_err(|e| anyhow::anyhow!("Failed to encode RGB WebP: {:?}", e))?
    };
    Ok(webp_memory.to_vec())
}

fn compress_avif(img: &DynamicImage, mode: CompressMode) -> Result<Vec<u8>> {
    use rgb::FromSlice;

    let (quality, speed) = mode.avif_params();
    let encoder = ravif::Encoder::new()
        .with_quality(quality)
        .with_speed(speed);

    let encoded = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        let pixels: &[rgb::RGBA8] = rgba.as_raw().as_rgba();
        let img_ref = ravif::Img::new(pixels, rgba.width() as usize, rgba.height() as usize);
        encoder
            .encode_rgba(img_ref)
            .context("Failed to compress RGBA AVIF with ravif")?
    } else {
        let rgb = img.to_rgb8();
        let pixels: &[rgb::RGB8] = rgb.as_raw().as_rgb();
        let img_ref = ravif::Img::new(pixels, rgb.width() as usize, rgb.height() as usize);
        encoder
            .encode_rgb(img_ref)
            .context("Failed to compress RGB AVIF with ravif")?
    };

    Ok(encoded.avif_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn sample_image() -> DynamicImage {
        let buf = ImageBuffer::from_fn(32, 32, |x, y| {
            Rgba([(x * 8) as u8, (y * 8) as u8, ((x + y) * 4) as u8, 255])
        });
        DynamicImage::ImageRgba8(buf)
    }

    #[test]
    fn test_compress_all_formats() {
        let img = sample_image();
        for mode in [CompressMode::Balanced, CompressMode::Ultra] {
            for format in [
                ImageFormat::Jpeg,
                ImageFormat::Png,
                ImageFormat::WebP,
                ImageFormat::Avif,
            ] {
                let bytes = compress_image(&img, format, mode)
                    .unwrap_or_else(|e| panic!("Failed {:?} in {:?}: {:?}", format, mode, e));
                assert!(!bytes.is_empty());
                let decoded = image::load_from_memory_with_format(&bytes, format)
                    .unwrap_or_else(|e| panic!("Failed to decode {:?}: {:?}", format, e));
                assert_eq!(decoded.width(), 32);
                assert_eq!(decoded.height(), 32);
            }
        }
    }
}
