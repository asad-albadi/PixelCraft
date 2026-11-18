use super::{pcx, vector};
use crate::{
    automation::mcp_io,
    model::{Document, MAX_DIMENSION, MAX_PIXELS},
};
use image::{DynamicImage, ImageDecoder, ImageFormat, RgbaImage};
use std::{
    io::{BufReader, BufWriter, Write},
    path::Path,
};

pub(super) fn load_static(path: &Path, extension: &str) -> Result<Document, String> {
    if extension == "pcx" {
        return pcx::load(path);
    }
    if extension == "webp" {
        let file = mcp_io::open(path).map_err(|e| e.to_string())?;
        let decoder = image::codecs::webp::WebPDecoder::new(BufReader::new(file))
            .map_err(|e| e.to_string())?;
        if decoder.has_animation() {
            return crate::interchange::load_webp(path);
        }
        return decode(decoder);
    }

    let decoder = mcp_io::image_reader(path)
        .map_err(|e| e.to_string())?
        .into_decoder()
        .map_err(|e| e.to_string())?;
    decode(decoder)
}

fn decode(decoder: impl ImageDecoder) -> Result<Document, String> {
    let (w, h) = decoder.dimensions();
    let (w, h) = (w as usize, h as usize);
    if w == 0
        || h == 0
        || w > MAX_DIMENSION
        || h > MAX_DIMENSION
        || w.saturating_mul(h) > MAX_PIXELS
        || decoder.total_bytes() > MAX_PIXELS as u64 * 8
    {
        return Err("Image exceeds canvas or memory limits".into());
    }
    let image = DynamicImage::from_decoder(decoder)
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let mut doc = Document::new(w, h);
    doc.layers[0].cels[0] = image.pixels().map(|pixel| pixel.0).collect();
    Ok(doc)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RasterFormat {
    Standard(ImageFormat),
    Pcx,
    Svg,
    Css,
}

pub(crate) fn raster_format(extension: &str) -> Option<RasterFormat> {
    if extension.eq_ignore_ascii_case("pcx") {
        return Some(RasterFormat::Pcx);
    }
    if extension.eq_ignore_ascii_case("svg") {
        return Some(RasterFormat::Svg);
    }
    if extension.eq_ignore_ascii_case("css") {
        return Some(RasterFormat::Css);
    }
    let format = ImageFormat::from_extension(extension)?;
    matches!(
        format,
        ImageFormat::Png
            | ImageFormat::Bmp
            | ImageFormat::Ico
            | ImageFormat::Qoi
            | ImageFormat::Tga
            | ImageFormat::Jpeg
            | ImageFormat::WebP
    )
    .then_some(RasterFormat::Standard(format))
}

pub(crate) fn validate_raster_size(
    format: RasterFormat,
    width: usize,
    height: usize,
    scale: usize,
) -> Result<(), String> {
    let (w, h) = (width.saturating_mul(scale), height.saturating_mul(scale));
    if w == 0
        || h == 0
        || w > MAX_DIMENSION
        || h > MAX_DIMENSION
        || w.saturating_mul(h) > MAX_PIXELS
    {
        return Err("Scaled export exceeds image limits".into());
    }
    if format == RasterFormat::Standard(ImageFormat::Ico) && (w > 256 || h > 256) {
        return Err("ICO export supports images up to 256 × 256 pixels".into());
    }
    Ok(())
}

pub(crate) fn write_raster_image(
    path: &Path,
    image: &RgbaImage,
    format: RasterFormat,
    scale: usize,
) -> Result<(), String> {
    validate_raster_size(
        format,
        image.width() as usize,
        image.height() as usize,
        scale,
    )?;
    match format {
        RasterFormat::Svg => return vector::write(path, image, scale, vector::VectorFormat::Svg),
        RasterFormat::Css => return vector::write(path, image, scale, vector::VectorFormat::Css),
        _ => {}
    }
    let image = crate::workflows::scale_image(image, scale)?;
    let RasterFormat::Standard(format) = format else {
        return pcx::write_rgb(path, &image);
    };
    if format != ImageFormat::Jpeg {
        return mcp_io::write_image_as(path, &image, format).map_err(|e| e.to_string());
    }
    let matte = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b, a] = image.get_pixel(x, y).0;
        image::Rgb([r, g, b].map(|channel| on_white(channel, a)))
    });
    let mut writer = BufWriter::new(mcp_io::create(path).map_err(|e| e.to_string())?);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 95)
        .encode_image(&matte)
        .map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())
}

pub(super) fn on_white(channel: u8, alpha: u8) -> u8 {
    let alpha = u16::from(alpha);
    ((u16::from(channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
}
