//! Convert only display buffers. Stored color values and profiles remain lossless.
use crate::model::*;
use moxcms::{ColorProfile, DataColorSpace, Layout, ToneReprCurve, TransformOptions};

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
    if profile.is_empty() {
        return Ok(pixels.to_vec());
    }
    if profile.len() < 16 {
        return Err("Truncated color profile".into());
    }
    let kind = u16::from_le_bytes([profile[0], profile[1]]);
    let source = match kind {
        0 => return Ok(pixels.to_vec()),
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
            } else {
                return Ok(pixels.to_vec());
            }
            source
        }
        2 => {
            let size = profile.get(16..20).ok_or("Truncated ICC length")?;
            let n = u32::from_le_bytes(size.try_into().unwrap()) as usize;
            let icc = profile
                .get(20..20usize.checked_add(n).ok_or("Invalid ICC length")?)
                .ok_or("Truncated ICC data")?;
            ColorProfile::new_from_slice(icc).map_err(|e| format!("Invalid ICC profile: {e}"))?
        }
        _ => return Err(format!("Unsupported color profile type {kind}")),
    };
    let gray = source.color_space == DataColorSpace::Gray;
    if !gray && source.color_space != DataColorSpace::Rgb {
        return Err("Sprite ICC profile must be RGB or grayscale".into());
    }
    let layout = if gray { Layout::Gray } else { Layout::Rgb };
    let transform = source
        .create_transform_8bit(
            layout,
            &ColorProfile::new_srgb(),
            Layout::Rgb,
            TransformOptions::default(),
        )
        .map_err(|e| format!("ICC transform: {e}"))?;
    let src: Vec<u8> = pixels
        .iter()
        .flat_map(|p| if gray { vec![p[0]] } else { p[..3].to_vec() })
        .collect();
    let mut dst = vec![0; pixels.len() * 3];
    transform
        .transform(&src, &mut dst)
        .map_err(|e| format!("ICC rendering: {e}"))?;
    Ok(dst
        .as_chunks::<3>()
        .0
        .iter()
        .zip(pixels)
        .map(|(c, p)| [c[0], c[1], c[2], p[3]])
        .collect())
}
