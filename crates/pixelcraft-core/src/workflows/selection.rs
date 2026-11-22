use crate::model::{MAX_DIMENSION, MAX_PIXELS, Pixel};
use image::{ImageDecoder, Luma, Pixel as _};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskEffect {
    Grow,
    Shrink,
    InsideBorder,
    OutsideBorder,
}

pub fn change_mask(
    source: &[bool],
    width: usize,
    height: usize,
    radius: usize,
    effect: MaskEffect,
) -> Result<Vec<bool>, String> {
    if source.len() != mask_area(width, height)? || !(1..=256).contains(&radius) {
        return Err("Invalid mask or selection radius".into());
    }
    let grow = matches!(effect, MaskEffect::Grow | MaskEffect::OutsideBorder);
    let diameter = 2 * radius + 1;
    let keep = |count| if grow { count != 0 } else { count == diameter };
    let mut horizontal = vec![false; source.len()];
    for (row, out) in source
        .chunks_exact(width)
        .zip(horizontal.chunks_exact_mut(width))
    {
        let mut count = row
            .iter()
            .take(radius + 1)
            .filter(|&&selected| selected)
            .count();
        for (x, value) in out.iter_mut().enumerate() {
            *value = keep(count);
            if x >= radius {
                count -= usize::from(row[x - radius]);
            }
            if let Some(&selected) = row.get(x + radius + 1) {
                count += usize::from(selected);
            }
        }
    }
    let mut counts = vec![0; width];
    for row in horizontal.chunks_exact(width).take(radius + 1) {
        for (count, &selected) in counts.iter_mut().zip(row) {
            *count += usize::from(selected);
        }
    }
    let mut output = vec![false; source.len()];
    for (y, row) in output.chunks_exact_mut(width).enumerate() {
        let previous = y
            .checked_sub(radius)
            .map(|row| &horizontal[row * width..][..width]);
        let next = horizontal.get((y + radius + 1) * width..(y + radius + 2) * width);
        for (x, (mask, count)) in row.iter_mut().zip(&mut counts).enumerate() {
            let filtered = keep(*count);
            let selected = source[y * width + x];
            *mask = match effect {
                MaskEffect::InsideBorder => selected && !filtered,
                MaskEffect::OutsideBorder => filtered && !selected,
                _ => filtered,
            };
            if let Some(previous) = previous {
                *count -= usize::from(previous[x]);
            }
            if let Some(next) = next {
                *count += usize::from(next[x]);
            }
        }
    }
    Ok(output)
}

pub fn color_range(pixels: &[Pixel], color: Pixel, tolerance: u8) -> Vec<bool> {
    pixels
        .iter()
        .map(|pixel| {
            (pixel[3] == 0 && color[3] == 0)
                || pixel
                    .iter()
                    .zip(color)
                    .all(|(channel, target)| channel.abs_diff(target) <= tolerance)
        })
        .collect()
}

/// Fill a polygon using the same even-odd, pixel-center rule as drawing tools.
pub fn polygon_mask(
    width: usize,
    height: usize,
    vertices: &[(i32, i32)],
) -> Result<Vec<bool>, String> {
    let area = mask_area(width, height)?;
    if !(3..=4096).contains(&vertices.len()) {
        return Err("Polygon needs between 3 and 4096 points".into());
    }
    let top = vertices
        .iter()
        .map(|point| point.1)
        .min()
        .unwrap()
        .clamp(0, height as i32) as usize;
    let bottom = vertices
        .iter()
        .map(|point| point.1)
        .max()
        .unwrap()
        .clamp(0, height as i32) as usize;
    let edges = vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1));
    let mut crossings = Vec::<f64>::with_capacity(vertices.len());
    let mut mask = vec![false; area];
    for y in top..bottom {
        let py = y as f64 + 0.5;
        crossings.clear();
        for ((x0, y0), (x1, y1)) in edges.clone() {
            let (y0, y1) = (f64::from(y0), f64::from(y1));
            if (y0 > py) != (y1 > py) {
                let x0 = f64::from(x0);
                crossings.push(x0 + (f64::from(x1) - x0) * (py - y0) / (y1 - y0));
            }
        }
        crossings.sort_unstable_by(f64::total_cmp);
        for pair in crossings.chunks_exact(2) {
            let left = (pair[0] - 0.5).ceil().clamp(0., width as f64) as usize;
            let right = (pair[1] - 0.5).ceil().clamp(0., width as f64) as usize;
            mask[y * width + left..y * width + right].fill(true);
        }
    }
    Ok(mask)
}

fn mask_area(width: usize, height: usize) -> Result<usize, String> {
    let area = width.saturating_mul(height);
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || area > MAX_PIXELS
    {
        return Err("Invalid selection mask dimensions".into());
    }
    Ok(area)
}

pub fn write_mask_png(
    path: &Path,
    mask: &[bool],
    width: usize,
    height: usize,
) -> Result<(), String> {
    if mask.len() != mask_area(width, height)? {
        return Err("Selection mask dimensions do not match the sprite".into());
    }
    let bytes: Vec<_> = mask
        .iter()
        .map(|&selected| if selected { 255 } else { 0 })
        .collect();
    image::save_buffer_with_format(
        path,
        &bytes,
        width as u32,
        height as u32,
        image::ColorType::L8,
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())
}

pub fn read_mask_png(path: &Path, width: usize, height: usize) -> Result<Vec<bool>, String> {
    let area = mask_area(width, height)?;
    let mut reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
    reader.set_format(image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(width as u32);
    limits.max_image_height = Some(height as u32);
    limits.max_alloc = Some(area as u64 * 16);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    if decoder.dimensions() != (width as u32, height as u32) {
        return Err("PNG mask dimensions must match the sprite".into());
    }
    let decoded = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let use_alpha = decoded.pixels().any(|p| p[3] != 255);
    Ok(decoded
        .pixels()
        .map(|pixel| {
            let Luma([luma]) = pixel.to_luma();
            if use_alpha {
                pixel[3] >= 128
            } else {
                luma >= 128
            }
        })
        .collect())
}
