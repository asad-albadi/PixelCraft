use crate::model::*;
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    path::Path,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum AtlasLayout {
    Rows,
    #[default]
    MaxRects,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AtlasOptions {
    pub layout: AtlasLayout,
    pub width: usize,
    pub padding: usize,
    pub trim: bool,
    pub deduplicate: bool,
    pub power_of_two: bool,
}
impl Default for AtlasOptions {
    fn default() -> Self {
        Self {
            layout: AtlasLayout::MaxRects,
            width: 1024,
            padding: 1,
            trim: true,
            deduplicate: true,
            power_of_two: false,
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}
impl Rect {
    fn contains(self, b: Self) -> bool {
        b.x >= self.x
            && b.y >= self.y
            && b.x + b.w <= self.x + self.w
            && b.y + b.h <= self.y + self.h
    }
    fn intersects(self, b: Self) -> bool {
        self.x < b.x + b.w && self.x + self.w > b.x && self.y < b.y + b.h && self.y + self.h > b.y
    }
}
fn maxrects(
    sizes: &[(usize, usize)],
    width: usize,
    height: usize,
    pad: usize,
) -> Option<Vec<Rect>> {
    let mut free = vec![Rect {
        x: pad,
        y: pad,
        w: width.checked_sub(pad)?,
        h: height.checked_sub(pad)?,
    }];
    let mut order: Vec<_> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(sizes[i].0.max(sizes[i].1)));
    let mut placed = vec![
        Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0
        };
        sizes.len()
    ];
    for i in order {
        let (w, h) = (sizes[i].0 + pad, sizes[i].1 + pad);
        let r = free
            .iter()
            .filter(|r| w <= r.w && h <= r.h)
            .min_by_key(|r| ((r.w - w).min(r.h - h), (r.w - w).max(r.h - h)))?;
        let used = Rect {
            x: r.x,
            y: r.y,
            w,
            h,
        };
        placed[i] = used;
        let mut split = vec![];
        for r in free {
            if !r.intersects(used) {
                split.push(r);
                continue;
            }
            if used.x > r.x {
                split.push(Rect {
                    w: used.x - r.x,
                    ..r
                });
            }
            if used.x + used.w < r.x + r.w {
                split.push(Rect {
                    x: used.x + used.w,
                    w: r.x + r.w - used.x - used.w,
                    ..r
                });
            }
            if used.y > r.y {
                split.push(Rect {
                    h: used.y - r.y,
                    ..r
                });
            }
            if used.y + used.h < r.y + r.h {
                split.push(Rect {
                    y: used.y + used.h,
                    h: r.y + r.h - used.y - used.h,
                    ..r
                });
            }
        }
        free = split
            .iter()
            .enumerate()
            .filter(|(i, r)| {
                !split
                    .iter()
                    .enumerate()
                    .any(|(j, s)| i != &j && s.contains(**r) && (s.w * s.h > r.w * r.h || j < *i))
            })
            .map(|(_, r)| *r)
            .collect();
    }
    Some(placed)
}
pub fn pack_atlas(
    sources: &[(String, Document)],
    o: &AtlasOptions,
) -> Result<(RgbaImage, Value), String> {
    if sources.is_empty() || o.width == 0 || o.width > MAX_DIMENSION || o.padding > 256 {
        return Err("Choose input sprites, a width from 1–16384, and padding from 0–256".into());
    }
    let mut entries = vec![];
    let mut unique: Vec<(usize, usize, Vec<Pixel>)> = vec![];
    let mut hashes: HashMap<u64, Vec<usize>> = HashMap::new();
    let mut total = 0usize;
    for (si, (name, d)) in sources.iter().enumerate() {
        d.validate()?;
        for f in 0..d.durations.len() {
            let pixels = d.composite(f);
            let (mut x0, mut y0, mut x1, mut y1) = (d.width, d.height, 0, 0);
            if o.trim {
                for (i, c) in pixels.iter().enumerate() {
                    if c[3] > 0 {
                        x0 = x0.min(i % d.width);
                        y0 = y0.min(i / d.width);
                        x1 = x1.max(i % d.width + 1);
                        y1 = y1.max(i / d.width + 1);
                    }
                }
                if x1 == 0 {
                    (x0, y0, x1, y1) = (0, 0, 1, 1);
                }
            } else {
                (x0, y0, x1, y1) = (0, 0, d.width, d.height);
            }
            let (w, h) = (x1 - x0, y1 - y0);
            let mut p = vec![];
            for y in y0..y1 {
                p.extend_from_slice(&pixels[y * d.width + x0..y * d.width + x1]);
            }
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (w, h, &p).hash(&mut hash);
            let hash = hash.finish();
            let id = if o.deduplicate {
                hashes
                    .get(&hash)
                    .and_then(|ids| {
                        ids.iter()
                            .find(|&&i| unique[i].0 == w && unique[i].1 == h && unique[i].2 == p)
                    })
                    .copied()
            } else {
                None
            };
            let id = id.unwrap_or_else(|| {
                let id = unique.len();
                total = total.saturating_add(w * h);
                unique.push((w, h, p));
                hashes.entry(hash).or_default().push(id);
                id
            });
            if total > MAX_PIXELS || entries.len() >= 65535 {
                return Err("Atlas inputs exceed pixel or frame budget".into());
            }
            entries.push((
                format!("{si}/{name}/{f}"),
                si,
                f,
                id,
                [x0, y0, w, h],
                d.width,
                d.height,
                d.durations[f],
            ));
        }
    }
    let pad = o.padding;
    let min_width = unique.iter().map(|s| s.0 + 2 * pad).max().unwrap_or(1);
    let width = if o.power_of_two {
        1usize << o.width.ilog2()
    } else {
        o.width
    };
    if min_width > width {
        return Err("A frame exceeds the maximum atlas width, including padding".into());
    }
    if width > MAX_DIMENSION {
        return Err("Frames exceed atlas width limit".into());
    }
    let sizes: Vec<_> = unique.iter().map(|(w, h, _)| (*w, *h)).collect();
    let placements = if o.layout == AtlasLayout::Rows {
        let (mut x, mut y, mut rh) = (pad, pad, 0);
        let mut out = vec![];
        for &(w, h) in &sizes {
            if x + w + pad > width {
                x = pad;
                y += rh + pad;
                rh = 0;
            }
            out.push(Rect {
                x,
                y,
                w: w + pad,
                h: h + pad,
            });
            x += w + pad;
            rh = rh.max(h);
        }
        out
    } else {
        let area: usize = sizes.iter().map(|(w, h)| (w + pad) * (h + pad)).sum();
        let mut height = sizes
            .iter()
            .map(|s| s.1 + 2 * pad)
            .max()
            .unwrap_or(1)
            .max(area.div_ceil(width));
        loop {
            // The free search rectangle is not the output image. A narrow packed
            // result can fit the pixel budget even with a wide maximum width.
            if height > MAX_DIMENSION {
                return Err("Atlas exceeds image limits; increase width or reduce inputs".into());
            }
            if let Some(out) = maxrects(&sizes, width, height, pad) {
                break out;
            }
            let limit = MAX_DIMENSION;
            if height >= limit {
                return Err("Atlas does not fit within image limits".into());
            }
            height = (height + 1).max(height * 2).min(limit);
        }
    };
    let mut w = placements.iter().map(|r| r.x + r.w).max().unwrap_or(1);
    let mut h = placements.iter().map(|r| r.y + r.h).max().unwrap_or(1);
    if o.power_of_two {
        w = w.next_power_of_two();
        h = h.next_power_of_two();
    }
    if w > MAX_DIMENSION || h > MAX_DIMENSION || w.saturating_mul(h) > MAX_PIXELS {
        return Err("Atlas exceeds image limits".into());
    }
    let mut im = RgbaImage::new(w as u32, h as u32);
    for ((sw, sh, p), r) in unique.iter().zip(&placements) {
        for y in 0..*sh {
            for x in 0..*sw {
                im.put_pixel((r.x + x) as u32, (r.y + y) as u32, Rgba(p[y * sw + x]));
            }
        }
    }
    let frames:Vec<_>=entries.iter().map(|(name,si,f,id,b,sw,sh,duration)|{let r=placements[*id];json!({"filename":name,"source":si,"sourceFrame":f,"frame":{"x":r.x,"y":r.y,"w":b[2],"h":b[3]},"rotated":false,"trimmed":o.trim,"spriteSourceSize":{"x":b[0],"y":b[1],"w":b[2],"h":b[3]},"sourceSize":{"w":sw,"h":sh},"duration":duration})}).collect();
    let sources: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(i, (name, d))| json!({"id":i,"name":name,"tags":d.tags}))
        .collect();
    Ok((
        im,
        json!({"frames":frames,"meta":{"app":"PixelCraft","image":"atlas.png","size":{"w":w,"h":h},"padding":pad,"layout":o.layout,"sources":sources}}),
    ))
}
pub fn export_atlas(
    path: &Path,
    sources: &[(String, Document)],
    o: &AtlasOptions,
) -> Result<(), String> {
    let (im, mut meta) = pack_atlas(sources, o)?;
    meta["meta"]["image"] = json!(path.file_name().unwrap_or_default().to_string_lossy());
    im.save(path).map_err(|e| e.to_string())?;
    crate::interchange::write_json(&path.with_extension("json"), &meta)
}
