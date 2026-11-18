use crate::model::{CLEAR, Document, MAX_PIXELS, Pixel};
use std::{collections::BTreeMap, ops::RangeInclusive};

#[derive(Default)]
struct Bin {
    weight: u64,
    sums: [u64; 4],
}

impl Bin {
    fn add(&mut self, pixel: Pixel, count: u64) {
        self.weight += count;
        for (sum, channel) in self.sums.iter_mut().zip(pixel) {
            *sum += u64::from(channel) * count;
        }
    }

    fn color(&self) -> Pixel {
        self.sums
            .map(|sum| ((sum + self.weight / 2) / self.weight) as u8)
    }
}

struct ColorBox {
    colors: Vec<(Pixel, u64)>,
    channel: usize,
    priority: u64,
    weight: u64,
}

impl ColorBox {
    fn new(colors: Vec<(Pixel, u64)>) -> Self {
        let mut lows = [255; 4];
        let mut highs = [0; 4];
        let mut weight = 0;
        for &(color, count) in &colors {
            weight += count;
            for channel in 0..4 {
                lows[channel] = lows[channel].min(color[channel]);
                highs[channel] = highs[channel].max(color[channel]);
            }
        }
        let channel = (0..4).max_by_key(|&c| highs[c] - lows[c]).unwrap();
        let priority = u64::from(highs[channel] - lows[channel]) * weight;
        Self {
            colors,
            channel,
            priority,
            weight,
        }
    }

    fn split(mut self) -> [Self; 2] {
        self.colors
            .sort_unstable_by_key(|&(color, _)| (color[self.channel], color));
        let half = self.weight.div_ceil(2);
        let mut sum = 0;
        let split = self
            .colors
            .iter()
            .position(|&(_, weight)| {
                sum += weight;
                sum >= half
            })
            .map_or(1, |index| (index + 1).min(self.colors.len() - 1));
        let rest = self.colors.split_off(split);
        [Self::new(self.colors), Self::new(rest)]
    }

    fn color(self) -> Pixel {
        let mut bin = Bin::default();
        for (pixel, count) in self.colors {
            bin.add(pixel, count);
        }
        bin.color()
    }
}

/// Generate from visible, composited artwork, reserving the first slot for transparency.
pub fn generate_artwork_palette(
    doc: &Document,
    frames: RangeInclusive<usize>,
    limit: usize,
) -> Result<Vec<Pixel>, String> {
    generate_artwork_palette_until(doc, frames, limit, || false)
}

pub fn generate_artwork_palette_until(
    doc: &Document,
    frames: RangeInclusive<usize>,
    limit: usize,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<Pixel>, String> {
    if !(2..=256).contains(&limit) || frames.is_empty() || *frames.end() >= doc.durations.len() {
        return Err("Choose a valid frame range and 2–256 colors".into());
    }
    let area = doc.width * doc.height;
    if frames.clone().count().saturating_mul(area) > MAX_PIXELS * 2 {
        return Err("Palette source exceeds the work limit. Select fewer frames.".into());
    }
    let mut exact = BTreeMap::<Pixel, u64>::new();
    let mut reduced = false;
    let mut bins = BTreeMap::<u32, Bin>::new();
    let add = |bins: &mut BTreeMap<u32, Bin>, rgba: Pixel, count| {
        let [r, g, b, a] = rgba.map(u32::from);
        let key = ((r >> 3) << 13) | ((g >> 3) << 8) | ((b >> 3) << 3) | (a >> 5);
        bins.entry(key).or_default().add(rgba, count);
    };
    for frame in frames {
        if cancelled() {
            return Err("Palette generation cancelled".into());
        }
        let pixels = doc.composite(frame);
        for chunk in pixels.chunks(8192) {
            if cancelled() {
                return Err("Palette generation cancelled".into());
            }
            for &pixel in chunk.iter().filter(|p| p[3] != 0) {
                if reduced {
                    add(&mut bins, pixel, 1);
                } else {
                    *exact.entry(pixel).or_default() += 1;
                    if exact.len() >= limit {
                        for (color, count) in std::mem::take(&mut exact) {
                            add(&mut bins, color, count);
                        }
                        reduced = true;
                    }
                }
            }
        }
    }
    let mut output = vec![CLEAR];
    if !reduced {
        output.extend(exact.into_keys());
        return Ok(output);
    }
    let samples = bins
        .into_values()
        .map(|bin| (bin.color(), bin.weight))
        .collect();
    let mut boxes = vec![ColorBox::new(samples)];
    while boxes.len() < limit - 1 {
        if cancelled() {
            return Err("Palette generation cancelled".into());
        }
        let Some(index) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.colors.len() > 1)
            .max_by_key(|(_, b)| b.priority)
            .map(|(index, _)| index)
        else {
            break;
        };
        let split = boxes.swap_remove(index).split();
        boxes.extend(split);
    }
    output.extend(boxes.into_iter().map(ColorBox::color));
    output.sort_unstable();
    output.dedup();
    Ok(output)
}
