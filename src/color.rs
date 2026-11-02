//! Convert only display buffers. Stored color values and profiles remain lossless.
use crate::model::*;
use moxcms::{ColorProfile, DataColorSpace, Layout, ToneReprCurve, TransformOptions};
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
        .chunks_exact(3)
        .zip(pixels)
        .map(|(c, p)| [c[0], c[1], c[2], p[3]])
        .collect())
}
