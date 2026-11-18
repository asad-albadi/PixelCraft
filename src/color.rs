//! ICC decoding, display transforms, and explicit document color conversion.
use crate::model::*;
use moxcms::{
    ColorProfile, DataColorSpace, Layout, ToneReprCurve, Transform8BitExecutor, TransformOptions,
};
use std::{collections::HashMap, io::Read, path::Path, sync::Arc};

pub fn rgb_to_hsv([r, g, b, _]: Pixel) -> [f32; 3] {
    let [r, g, b] = [r, g, b].map(|byte| f32::from(byte) / 255.);
    let high = r.max(g).max(b);
    let low = r.min(g).min(b);
    let delta = high - low;
    let hue = if delta == 0. {
        0.
    } else if high == r {
        ((g - b) / delta).rem_euclid(6.)
    } else if high == g {
        (b - r) / delta + 2.
    } else {
        (r - g) / delta + 4.
    };
    [hue / 6., if high == 0. { 0. } else { delta / high }, high]
}

pub fn hsv_to_rgb([hue, saturation, value]: [f32; 3], alpha: u8) -> Pixel {
    let hue = hue.rem_euclid(1.) * 6.;
    let chroma = value * saturation;
    let mid = chroma * (1. - (hue.rem_euclid(2.) - 1.).abs());
    let offset = value - chroma;
    let rgb = match hue as u8 {
        0 => [chroma, mid, 0.],
        1 => [mid, chroma, 0.],
        2 => [0., chroma, mid],
        3 => [0., mid, chroma],
        4 => [mid, 0., chroma],
        _ => [chroma, 0., mid],
    };
    let [r, g, b] = rgb.map(|value| ((value + offset) * 255.).round() as u8);
    [r, g, b, alpha]
}
pub fn display_pixels(d: &Document, f: usize) -> Result<Vec<Pixel>, String> {
    to_srgb(&d.color_profile, &d.composite(f))
}
pub fn to_srgb(profile: &[u8], pixels: &[Pixel]) -> Result<Vec<Pixel>, String> {
    transform_pixels(profile, &[], pixels)
}

pub fn transform_pixels(
    source: &[u8],
    destination: &[u8],
    pixels: &[Pixel],
) -> Result<Vec<Pixel>, String> {
    if source == destination {
        return Ok(pixels.to_vec());
    }
    ProfileTransform::new(source, destination)?.pixels(pixels)
}

const MAX_ICC_BYTES: usize = 16 * 1024 * 1024;

pub fn icc_profile(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_ICC_BYTES {
        return Err("ICC profile exceeds 16 MiB".into());
    }
    let mut payload = vec![0; 16];
    payload[0] = 2;
    payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    payload.extend_from_slice(bytes);
    decode_profile(&payload)?;
    Ok(payload)
}

pub fn read_icc_profile(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    file.take(MAX_ICC_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    icc_profile(&bytes)
}

fn decode_profile(profile: &[u8]) -> Result<ColorProfile, String> {
    if profile.is_empty() {
        return Ok(ColorProfile::new_srgb());
    }
    if profile.len() < 16 {
        return Err("Truncated color profile".into());
    }
    let kind = u16::from_le_bytes([profile[0], profile[1]]);
    let decoded = match kind {
        0 => ColorProfile::new_srgb(),
        1 => {
            let mut source = ColorProfile::new_srgb();
            if profile[2] & 1 != 0 {
                let gamma = u32::from_le_bytes(profile[4..8].try_into().unwrap()) as f32 / 65536.;
                if gamma <= 0. {
                    return Err("Invalid fixed gamma".into());
                }
                source.cicp = None;
                source.red_trc = Some(ToneReprCurve::Parametric(vec![gamma]));
                source.green_trc = source.red_trc.clone();
                source.blue_trc = source.red_trc.clone();
            }
            source
        }
        2 => {
            let size = profile.get(16..20).ok_or("Truncated ICC length")?;
            let length = u32::from_le_bytes(size.try_into().unwrap()) as usize;
            if length > MAX_ICC_BYTES {
                return Err("ICC profile exceeds 16 MiB".into());
            }
            let icc = profile.get(20..20 + length).ok_or("Truncated ICC data")?;
            ColorProfile::new_from_slice(icc).map_err(|e| format!("Invalid ICC profile: {e}"))?
        }
        _ => return Err(format!("Unsupported color profile type {kind}")),
    };
    if !matches!(
        decoded.color_space,
        DataColorSpace::Rgb | DataColorSpace::Gray
    ) {
        return Err("ICC profile must use RGB or grayscale".into());
    }
    Ok(decoded)
}

pub fn validate_display_profile(profile: &[u8]) -> Result<(), String> {
    if decode_profile(profile)?.color_space != DataColorSpace::Rgb {
        return Err("The display profile must use RGB".into());
    }
    ProfileTransform::new(&[], profile)?;
    Ok(())
}

struct ProfileTransform {
    executor: Arc<Transform8BitExecutor>,
    from_gray: bool,
    to_gray: bool,
}

impl ProfileTransform {
    fn new(source: &[u8], destination: &[u8]) -> Result<Self, String> {
        let source = decode_profile(source)?;
        let destination = decode_profile(destination)?;
        let from_gray = source.color_space == DataColorSpace::Gray;
        let to_gray = destination.color_space == DataColorSpace::Gray;
        let layout = |gray| if gray { Layout::Gray } else { Layout::Rgb };
        let executor = source
            .create_transform_8bit(
                layout(from_gray),
                &destination,
                layout(to_gray),
                TransformOptions::default(),
            )
            .map_err(|error| format!("ICC transform: {error}"))?;
        Ok(Self {
            executor,
            from_gray,
            to_gray,
        })
    }

    fn pixels(&self, pixels: &[Pixel]) -> Result<Vec<Pixel>, String> {
        let input: Vec<u8> = pixels
            .iter()
            .flat_map(|p| if self.from_gray { &p[..1] } else { &p[..3] })
            .copied()
            .collect();
        let channels = if self.to_gray { 1 } else { 3 };
        let mut output = vec![0; pixels.len() * channels];
        self.executor
            .transform(&input, &mut output)
            .map_err(|error| format!("ICC rendering: {error}"))?;
        Ok(output
            .chunks_exact(channels)
            .zip(pixels)
            .map(|(color, source)| {
                if self.to_gray {
                    [color[0], color[0], color[0], source[3]]
                } else {
                    [color[0], color[1], color[2], source[3]]
                }
            })
            .collect())
    }

    fn stored_pixels(
        &self,
        source: &Pixels,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Pixels, String> {
        let mut output = if source.is_dense() {
            vec![CLEAR; source.len()].into()
        } else {
            Pixels::empty(source.len())
        };
        let mut slots = Vec::with_capacity(4096);
        let mut colors = Vec::with_capacity(4096);
        let mut result = Ok(());
        let mut flush = |slots: &mut Vec<usize>, colors: &mut Vec<Pixel>| -> Result<(), String> {
            if cancelled() {
                return Err("Profile conversion cancelled".into());
            }
            let data = self.pixels(colors)?;
            for (slot, color) in slots.drain(..).zip(data) {
                output.set(slot, color);
            }
            colors.clear();
            Ok(())
        };
        source.visit_nonclear(|index, pixel| {
            if result.is_ok() {
                slots.push(index);
                colors.push(pixel);
                if colors.len() == 4096 {
                    result = flush(&mut slots, &mut colors);
                }
            }
        });
        result?;
        if !slots.is_empty() {
            flush(&mut slots, &mut colors)?;
        }
        Ok(output)
    }
}

pub fn assign_document_profile(source: &Document, profile: &[u8]) -> Result<Document, String> {
    let decoded = decode_profile(profile)?;
    if decoded.color_space == DataColorSpace::Gray && source.color_mode != ColorMode::Grayscale {
        return Err("Assigning a grayscale ICC profile requires a grayscale sprite. Convert the profile to change color values.".into());
    }
    let mut next = source.clone();
    next.color_profile = profile.to_vec();
    Ok(next)
}

pub fn convert_document_profile(source: &Document, profile: &[u8]) -> Result<Document, String> {
    convert_document_profile_until(source, profile, || false)
}

pub fn convert_document_profile_until(
    source: &Document,
    profile: &[u8],
    cancelled: impl Fn() -> bool,
) -> Result<Document, String> {
    if source.color_profile == profile {
        decode_profile(profile)?;
        return Ok(source.clone());
    }
    let transform = ProfileTransform::new(&source.color_profile, profile)?;
    let mut next = source.clone();
    next.color_profile = profile.to_vec();
    if next.color_mode == ColorMode::Grayscale && !transform.to_gray {
        next.color_mode = ColorMode::Rgba;
    } else if next.color_mode == ColorMode::Rgba && transform.to_gray {
        next.color_mode = ColorMode::Grayscale;
    }
    next.palette = transform.pixels(&source.palette)?;
    for palette in &mut next.frame_palettes {
        if !palette.is_empty() {
            *palette = transform.pixels(palette)?;
        }
    }
    let mut shared = HashMap::new();
    let mut convert = |pixels: &mut Pixels| -> Result<(), String> {
        if cancelled() {
            return Err("Profile conversion cancelled".into());
        }
        if let Some(id) = pixels.backing_id() {
            if let Some(cached) = shared.get(&id) {
                *pixels = Pixels::clone(cached);
            } else {
                *pixels = transform.stored_pixels(pixels, &cancelled)?;
                shared.insert(id, pixels.clone());
            }
        }
        Ok(())
    };
    for layer in &mut next.layers {
        for pixels in &mut layer.cels {
            convert(pixels)?;
        }
        for meta in &mut layer.cel_meta {
            for chunk in meta.outside.chunks_mut(4096) {
                if cancelled() {
                    return Err("Profile conversion cancelled".into());
                }
                let colors: Vec<_> = chunk.iter().map(|pixel| pixel.color).collect();
                for (pixel, color) in chunk.iter_mut().zip(transform.pixels(&colors)?) {
                    pixel.color = color;
                }
            }
        }
    }
    for set in &mut next.tilesets {
        for pixels in &mut set.tiles {
            convert(pixels)?;
        }
    }
    next.validate()?;
    Ok(next)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImportProfile {
    #[default]
    Preserve,
    ConvertToSrgb,
    AssignSrgb,
}

pub fn apply_import_profile(document: Document, policy: ImportProfile) -> Result<Document, String> {
    match policy {
        ImportProfile::Preserve => Ok(document),
        ImportProfile::ConvertToSrgb => convert_document_profile(&document, &[]),
        ImportProfile::AssignSrgb => assign_document_profile(&document, &[]),
    }
}
