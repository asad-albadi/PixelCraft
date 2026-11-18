use crate::model::{ColorMode, Document, MAX_PIXELS, Pixel};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    BrightnessContrast {
        brightness: i16,
        contrast: i16,
    },
    Curves(Vec<(u8, u8)>),
    Convolution {
        weights: [i16; 9],
        divisor: i16,
        bias: i16,
    },
    Despeckle {
        threshold: u8,
    },
}

impl Filter {
    fn channel_map(&self) -> Result<Option<[u8; 256]>, String> {
        match self {
            Self::BrightnessContrast {
                brightness,
                contrast,
            } => {
                let brightness = f32::from(*brightness) * 2.55;
                let contrast = 1. + f32::from(*contrast) / if *contrast > 0 { 50. } else { 100. };
                Ok(Some(std::array::from_fn(|index| {
                    ((index as f32 - 127.5) * contrast + 127.5 + brightness)
                        .round()
                        .clamp(0., 255.) as u8
                })))
            }
            Self::Curves(points) => {
                if points.first().is_none_or(|p| p.0 != 0)
                    || points.last().is_none_or(|p| p.0 != 255)
                    || points.windows(2).any(|pair| pair[0].0 >= pair[1].0)
                {
                    return Err("Curve inputs must increase from 0 to 255".into());
                }
                let mut map = [0; 256];
                for pair in points.windows(2) {
                    let [(x0, y0), (x1, y1)] = [pair[0], pair[1]];
                    let span = u32::from(x1 - x0);
                    for x in x0..=x1 {
                        let t = u32::from(x - x0);
                        map[x as usize] =
                            ((u32::from(y0) * (span - t) + u32::from(y1) * t + span / 2) / span)
                                as u8;
                    }
                }
                Ok(Some(map))
            }
            Self::Convolution { divisor: 0, .. } => {
                Err("Convolution divisor cannot be zero".into())
            }
            _ => Ok(None),
        }
    }

    fn neighborhood(&self, pixels: [Pixel; 9], channel: usize) -> u8 {
        match self {
            Self::Convolution {
                weights,
                divisor,
                bias,
            } => {
                let sum: i32 = pixels
                    .iter()
                    .zip(weights)
                    .map(|(p, &weight)| i32::from(p[channel]) * i32::from(weight))
                    .sum();
                (sum / i32::from(*divisor) + i32::from(*bias)).clamp(0, 255) as u8
            }
            Self::Despeckle { threshold } => {
                let center = pixels[4][channel];
                let mut neighborhood = pixels.map(|p| p[channel]);
                let (_, median, _) = neighborhood.select_nth_unstable(4);
                if center.abs_diff(*median) >= *threshold {
                    *median
                } else {
                    center
                }
            }
            _ => unreachable!("Point filters use a channel map"),
        }
    }
}

/// Filter the selected raster area from a stable snapshot. Linked artwork is
/// filtered once and writes follow the document's native linked-cel behavior.
pub fn apply_filter(
    source: &Document,
    targets: &[(usize, usize)],
    bounds: [usize; 4],
    mask: Option<&[bool]>,
    filter: &Filter,
    channels: [bool; 4],
) -> Result<(Document, usize), String> {
    apply_filter_until(source, targets, bounds, mask, filter, channels, || false)
}

pub fn apply_filter_until(
    source: &Document,
    targets: &[(usize, usize)],
    [left, top, right, bottom]: [usize; 4],
    mask: Option<&[bool]>,
    filter: &Filter,
    channels: [bool; 4],
    cancelled: impl Fn() -> bool,
) -> Result<(Document, usize), String> {
    if targets.is_empty() || !channels.iter().any(|&enabled| enabled) {
        return Err("Choose a cel and at least one channel".into());
    }
    if left > right
        || top > bottom
        || right > source.width
        || bottom > source.height
        || mask.is_some_and(|mask| mask.len() != source.width * source.height)
    {
        return Err("Invalid filter selection".into());
    }
    let map = filter.channel_map()?;
    let area = (right - left) * (bottom - top);
    let mut linked = HashSet::new();
    let mut unique = HashSet::new();
    let mut planned = Vec::new();
    let mut work = 0usize;
    for &(layer, frame) in targets {
        if cancelled() {
            return Err("Filter cancelled".into());
        }
        if !source.editable(layer)
            || frame >= source.durations.len()
            || source.layers[layer].tileset.is_some()
            || source.layers[layer].adjustment.is_some()
        {
            return Err("Choose unlocked raster cels".into());
        }
        let meta = &source.layers[layer].cel_meta[frame];
        if !unique.insert((layer, frame))
            || meta
                .artwork_link
                .is_some_and(|key| !linked.insert((layer, key)))
        {
            continue;
        }
        let writing = if meta.artwork_link.is_some() {
            source.durations.len()
        } else {
            1
        };
        let palette = if source.color_mode == ColorMode::Indexed {
            source.palette_for(frame).len().div_ceil(8)
        } else {
            1
        };
        let sampling = if map.is_some() { 1 } else { 9 };
        work = work.saturating_add(area.saturating_mul(sampling + writing.saturating_mul(palette)));
        if work > MAX_PIXELS * 2 {
            return Err(
                "Filter scope exceeds the work limit. Choose fewer cels or a smaller selection."
                    .into(),
            );
        }
        planned.push((layer, frame));
    }
    let mut next = source.clone();
    let mut changed = 0;
    for (layer, frame) in planned {
        let artwork = &source.layers[layer].cels[frame];
        let change_alpha = channels[3] && !source.layers[layer].background;
        for y in top..bottom {
            if cancelled() {
                return Err("Filter cancelled".into());
            }
            for x in left..right {
                let index = y * source.width + x;
                let original = artwork[index];
                if mask.is_some_and(|mask| !mask[index]) || (original[3] == 0 && !change_alpha) {
                    continue;
                }
                let neighbors = map.is_none().then(|| {
                    std::array::from_fn(|i| {
                        let neighbor_x = (x + i % 3).saturating_sub(1).min(source.width - 1);
                        let neighbor_y = (y + i / 3).saturating_sub(1).min(source.height - 1);
                        let p = artwork[neighbor_y * source.width + neighbor_x];
                        if p[3] == 0 && !change_alpha {
                            original
                        } else {
                            p
                        }
                    })
                });
                let mut color = original;
                for channel in 0..4 {
                    if channels[channel] && (channel != 3 || change_alpha) {
                        color[channel] = if let Some(map) = &map {
                            map[original[channel] as usize]
                        } else {
                            filter.neighborhood(
                                neighbors.expect("Neighborhood filters have samples"),
                                channel,
                            )
                        };
                    }
                }
                if color != original {
                    next.set_pixel(layer, frame, index, color);
                    changed += usize::from(next.layers[layer].cels[frame][index] != original);
                }
            }
        }
    }
    if changed != 0 {
        next.validate()?;
    }
    Ok((next, changed))
}
