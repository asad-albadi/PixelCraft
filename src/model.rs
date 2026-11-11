use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};
pub type Pixel = [u8; 4];
pub const CLEAR: Pixel = [0; 4];
pub const DEFAULT_DURATION: u16 = 100;
pub const MIN_BRUSH_SIZE: i32 = 1;
pub const MAX_BRUSH_SIZE: i32 = 64;
pub const MAX_DIMENSION: usize = 16384;
pub const MAX_PIXELS: usize = 64_000_000;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pixels(pub Arc<Vec<Pixel>>);
impl Pixels {
    pub fn shares_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl PartialEq for Pixels {
    fn eq(&self, other: &Self) -> bool {
        self.shares_storage(other) || self.0 == other.0
    }
}
impl From<Vec<Pixel>> for Pixels {
    fn from(v: Vec<Pixel>) -> Self {
        Self(Arc::new(v))
    }
}
impl FromIterator<Pixel> for Pixels {
    fn from_iter<T: IntoIterator<Item = Pixel>>(v: T) -> Self {
        Vec::from_iter(v).into()
    }
}
impl Deref for Pixels {
    type Target = Vec<Pixel>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Pixels {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}
impl<'a> IntoIterator for &'a Pixels {
    type Item = &'a Pixel;
    type IntoIter = std::slice::Iter<'a, Pixel>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ColorMode {
    #[default]
    Rgba,
    Grayscale,
    Indexed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BrushShape {
    #[default]
    Round,
    Square,
    Diamond,
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum BlendMode {
    #[default]
    Normal = 0,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
    Addition,
    Subtract,
    Divide,
}
impl BlendMode {
    pub const ALL: [Self; 19] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::ColorDodge,
        Self::ColorBurn,
        Self::HardLight,
        Self::SoftLight,
        Self::Difference,
        Self::Exclusion,
        Self::Hue,
        Self::Saturation,
        Self::Color,
        Self::Luminosity,
        Self::Addition,
        Self::Subtract,
        Self::Divide,
    ];
    pub fn from_id(n: u16) -> Result<Self, String> {
        Self::ALL
            .get(n as usize)
            .copied()
            .ok_or_else(|| "Unknown blend mode".into())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutsidePixel {
    pub x: i32,
    pub y: i32,
    pub color: Pixel,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TilemapGeometry {
    pub x: i32,
    pub y: i32,
    pub columns: usize,
    pub rows: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CelMeta {
    pub opacity: u8,
    pub z_index: i16,
    pub outside: Vec<OutsidePixel>,
    #[serde(default)]
    pub indices: Option<Arc<Vec<u8>>>,
    #[serde(default)]
    pub tilemap: Option<TilemapGeometry>,
    #[serde(default)]
    pub bounds: Option<[i32; 4]>,
}
impl Default for CelMeta {
    fn default() -> Self {
        Self {
            opacity: 255,
            z_index: 0,
            outside: vec![],
            indices: None,
            tilemap: None,
            bounds: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameTag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    pub direction: u8,
    #[serde(default)]
    pub repeat: u16,
    pub color: [u8; 3],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tileset {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<Pixels>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadataChunk {
    pub frame: usize,
    pub kind: u16,
    pub data: Vec<u8>,
    pub owner: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    pub cels: Vec<Pixels>,
    pub group: bool,
    pub parent: Option<usize>,
    pub blend: BlendMode,
    pub cel_meta: Vec<CelMeta>,
    pub tileset: Option<usize>,
    pub tile_cels: Vec<Vec<u32>>,
    #[serde(default)]
    pub background: bool,
    #[serde(default)]
    pub ase_flags: u16,
    #[serde(default)]
    pub uuid: Vec<u8>,
}
impl Layer {
    pub fn new(name: String, w: usize, h: usize, frames: usize) -> Self {
        Self {
            name,
            visible: true,
            locked: false,
            opacity: 255,
            cels: vec![vec![CLEAR; w * h].into(); frames],
            group: false,
            parent: None,
            blend: BlendMode::Normal,
            cel_meta: vec![CelMeta::default(); frames],
            tileset: None,
            tile_cels: vec![vec![]; frames],
            background: false,
            ase_flags: 0,
            uuid: vec![],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AseTilesetInfo {
    pub index: usize,
    pub id: u32,
    pub flags: u32,
    pub base_index: i16,
    pub external: Option<[u32; 2]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "default_compose_groups")]
    pub compose_groups: bool,
    pub width: usize,
    pub height: usize,
    pub layers: Vec<Layer>,
    pub durations: Vec<u16>,
    pub color_mode: ColorMode,
    pub palette: Vec<Pixel>,
    #[serde(default)]
    pub palette_names: Vec<String>,
    #[serde(default)]
    pub frame_palettes: Vec<Vec<Pixel>>,
    #[serde(default)]
    pub transparent_index: usize,
    #[serde(default)]
    pub drawing_index: Option<usize>,
    pub tags: Vec<FrameTag>,
    pub tilesets: Vec<Tileset>,
    #[serde(default)]
    pub ase_tilesets: Vec<AseTilesetInfo>,
    pub metadata: Vec<MetadataChunk>,
    pub color_profile: Vec<u8>,
    pub brush_shape: BrushShape,
    #[serde(default)]
    pub dithering: Option<crate::contributions::DitheringMatrix>,
    #[serde(default)]
    pub wraparound: bool,
    #[serde(default)]
    pub palette_snap: bool,
    #[serde(default)]
    pub references: Vec<crate::productivity::ReferenceImage>,
}
pub fn default_palette() -> Vec<Pixel> {
    let mut p = vec![CLEAR];
    for r in [0, 51, 102, 153, 204, 255] {
        for g in [0, 51, 102, 153, 204, 255] {
            for b in [0, 51, 102, 153, 204, 255] {
                p.push([r, g, b, 255]);
            }
        }
    }
    for v in (0..=255).step_by(7) {
        p.push([v, v, v, 255]);
    }
    p.truncate(256);
    p
}
pub fn nearest(p: Pixel, palette: &[Pixel]) -> usize {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| {
            p.iter()
                .zip(c.iter())
                .map(|(a, b)| (*a as i32 - *b as i32).pow(2) as u32)
                .sum::<u32>()
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}
pub fn convert(p: Pixel, mode: ColorMode, palette: &[Pixel]) -> Pixel {
    match mode {
        ColorMode::Rgba => p,
        ColorMode::Indexed => palette.get(nearest(p, palette)).copied().unwrap_or(CLEAR),
        ColorMode::Grayscale => {
            // doc::rgba_to_graya_using_luma, 0x5f7170: integer division
            // truncates; there is no half-denominator rounding bias.
            let y = ((p[0] as u32 * 2126 + p[1] as u32 * 7152 + p[2] as u32 * 722) / 10000) as u8;
            [y, y, y, p[3]]
        }
    }
}
pub fn over(dst: Pixel, src: Pixel, opacity: u8) -> Pixel {
    // doc::rgba_blender_normal, 0x5e81e0. Keep the native rounding
    // order: round alpha products, then truncate signed color division.
    let mul_un8 = |a: i32, b: i32| {
        let t = a * b + 128;
        (t + (t >> 8)) >> 8
    };
    let a = mul_un8(src[3] as i32, opacity as i32);
    if dst[3] == 0 {
        return [src[0], src[1], src[2], a as u8];
    }
    if src[3] == 0 {
        return dst;
    }
    let b = dst[3] as i32;
    let out = a + b - mul_un8(a, b);
    let mut p = [0; 4];
    for i in 0..3 {
        p[i] = (dst[i] as i32 + (src[i] as i32 - dst[i] as i32) * a / out) as u8;
    }
    p[3] = out as u8;
    p
}
pub fn blend(dst: Pixel, src: Pixel, opacity: u8, mode: BlendMode) -> Pixel {
    if mode == BlendMode::Normal || dst[3] == 0 {
        return over(dst, src, opacity);
    }
    let d = [
        dst[0] as f64 / 255.,
        dst[1] as f64 / 255.,
        dst[2] as f64 / 255.,
    ];
    let s = [
        src[0] as f64 / 255.,
        src[1] as f64 / 255.,
        src[2] as f64 / 255.,
    ];
    let mut b = [0.; 3];
    for i in 0..3 {
        let (a, c) = (d[i], s[i]);
        b[i] = match mode {
            BlendMode::Multiply => a * c,
            BlendMode::Screen => a + c - a * c,
            BlendMode::Overlay => {
                if a <= 0.5 {
                    2. * a * c
                } else {
                    1. - 2. * (1. - a) * (1. - c)
                }
            }
            BlendMode::Darken => a.min(c),
            BlendMode::Lighten => a.max(c),
            BlendMode::ColorDodge => {
                if c >= 1. {
                    1.
                } else {
                    (a / (1. - c)).min(1.)
                }
            }
            BlendMode::ColorBurn => {
                if c <= 0. {
                    0.
                } else {
                    1. - ((1. - a) / c).min(1.)
                }
            }
            BlendMode::HardLight => {
                if c <= 0.5 {
                    2. * a * c
                } else {
                    1. - 2. * (1. - a) * (1. - c)
                }
            }
            BlendMode::SoftLight => {
                if c <= 0.5 {
                    a - (1. - 2. * c) * a * (1. - a)
                } else {
                    a + (2. * c - 1.)
                        * (if a <= 0.25 {
                            ((16. * a - 12.) * a + 4.) * a
                        } else {
                            a.sqrt()
                        } - a)
                }
            }
            BlendMode::Difference => (a - c).abs(),
            BlendMode::Exclusion => a + c - 2. * a * c,
            BlendMode::Addition => (a + c).min(1.),
            BlendMode::Subtract => (a - c).max(0.),
            BlendMode::Divide => {
                if c == 0. {
                    1.
                } else {
                    (a / c).min(1.)
                }
            }
            _ => c,
        };
    }
    fn lum(c: [f64; 3]) -> f64 {
        c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11
    }
    fn sat(c: [f64; 3]) -> f64 {
        c.into_iter().fold(0., f64::max) - c.into_iter().fold(1., f64::min)
    }
    fn set_lum(mut c: [f64; 3], l: f64) -> [f64; 3] {
        let diff = l - lum(c);
        for v in &mut c {
            *v += diff;
        }
        let lo = c.into_iter().fold(1., f64::min);
        let hi = c.into_iter().fold(0., f64::max);
        if lo < 0. {
            for v in &mut c {
                *v = l + (*v - l) * l / (l - lo)
            }
        }
        if hi > 1. {
            for v in &mut c {
                *v = l + (*v - l) * (1. - l) / (hi - l)
            }
        }
        c
    }
    fn set_sat(mut c: [f64; 3], s: f64) -> [f64; 3] {
        let lo = c.into_iter().fold(1., f64::min);
        let hi = c.into_iter().fold(0., f64::max);
        for v in &mut c {
            *v = if hi > lo {
                (*v - lo) * s / (hi - lo)
            } else {
                0.
            }
        }
        c
    }
    b = match mode {
        BlendMode::Hue => set_lum(set_sat(s, sat(d)), lum(d)),
        BlendMode::Saturation => set_lum(set_sat(d, sat(s)), lum(d)),
        BlendMode::Color => set_lum(s, lum(d)),
        BlendMode::Luminosity => set_lum(d, lum(s)),
        _ => b,
    };
    let sa = src[3] as f64 * opacity as f64 / 65025.;
    let da = dst[3] as f64 / 255.;
    let a = sa + da - sa * da;
    if a == 0. {
        return CLEAR;
    }
    let mut out = [0; 4];
    for i in 0..3 {
        out[i] = (((1. - sa) * da * d[i] + (1. - da) * sa * s[i] + sa * da * b[i]) / a * 255.)
            .round()
            .clamp(0., 255.) as u8;
    }
    out[3] = (a * 255.).round() as u8;
    out
}
fn default_compose_groups() -> bool {
    true
}
impl Document {
    pub fn new(w: usize, h: usize) -> Self {
        assert!(w > 0 && h > 0 && w <= MAX_DIMENSION && h <= MAX_DIMENSION && w * h <= MAX_PIXELS);
        Self {
            compose_groups: true,
            width: w,
            height: h,
            layers: vec![Layer::new("Layer 1".into(), w, h, 1)],
            durations: vec![DEFAULT_DURATION],
            color_mode: ColorMode::Rgba,
            palette: default_palette(),
            palette_names: vec![],
            frame_palettes: vec![],
            transparent_index: 0,
            drawing_index: None,
            tags: vec![],
            tilesets: vec![],
            ase_tilesets: vec![],
            metadata: vec![],
            color_profile: vec![],
            brush_shape: BrushShape::Round,
            dithering: None,
            wraparound: false,
            palette_snap: false,
            references: vec![],
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut reference_pixels = 0usize;
        for r in &self.references {
            reference_pixels = reference_pixels.saturating_add(r.width.saturating_mul(r.height));
            if r.width == 0
                || r.height == 0
                || r.width > MAX_DIMENSION
                || r.height > MAX_DIMENSION
                || r.pixels.len() != r.width.saturating_mul(r.height)
                || reference_pixels > MAX_PIXELS
                || !r.x.is_finite()
                || !r.y.is_finite()
                || !r.scale.is_finite()
                || r.scale <= 0.
                || r.scale > 1000.
            {
                return Err("Invalid reference image".into());
            }
        }
        if self.dithering.as_ref().is_some_and(|m| !m.validate()) {
            return Err("Invalid dithering matrix".into());
        }
        if self.width == 0
            || self.height == 0
            || self.width > MAX_DIMENSION
            || self.height > MAX_DIMENSION
            || self.durations.is_empty()
            || self.layers.is_empty()
            || self.durations.len() > 65535
            || self.layers.len() > 65535
            || self.stored_pixels() > MAX_PIXELS
        {
            return Err("Project dimensions or stored pixel count exceed limits".into());
        }
        if self
            .frame_palettes
            .iter()
            .any(|p| p.is_empty() || p.len() > 256)
        {
            return Err("Invalid frame palette".into());
        }
        if self.palette.is_empty() || self.palette.len() > 256 {
            return Err("Palette must contain 1–256 colors".into());
        }
        for (i, l) in self.layers.iter().enumerate() {
            for m in &l.cel_meta {
                if m.bounds.is_some_and(|b| {
                    b[0] < i16::MIN as i32
                        || b[0] > i16::MAX as i32
                        || b[1] < i16::MIN as i32
                        || b[1] > i16::MAX as i32
                        || b[2] < 0
                        || b[3] < 0
                        || b[2] > 65535
                        || b[3] > 65535
                        || (b[2] as usize).saturating_mul(b[3] as usize) > MAX_PIXELS
                }) {
                    return Err("Invalid cel bounds".into());
                }
                if m.tilemap.as_ref().is_some_and(|g| {
                    g.columns == 0
                        || g.rows == 0
                        || g.columns > 65535
                        || g.rows > 65535
                        || g.columns.saturating_mul(g.rows) > MAX_PIXELS
                        || g.x < i16::MIN as i32
                        || g.x > i16::MAX as i32
                        || g.y < i16::MIN as i32
                        || g.y > i16::MAX as i32
                }) {
                    return Err("Invalid tilemap geometry".into());
                }
            }
            if l.cel_meta.iter().any(|m| {
                m.indices
                    .as_ref()
                    .is_some_and(|a| a.len() != self.width * self.height)
            }) {
                return Err("Invalid indexed cel size".into());
            }
            if l.cels.len() != self.durations.len()
                || l.cel_meta.len() != self.durations.len()
                || l.tile_cels.len() != self.durations.len()
                || l.cels.iter().any(|c| c.len() != self.width * self.height)
            {
                return Err("Invalid cel dimensions".into());
            }
            let mut p = l.parent;
            let mut depth = 0;
            while let Some(j) = p {
                if j >= self.layers.len() || j == i || !self.layers[j].group || depth >= 128 {
                    return Err("Invalid layer group hierarchy".into());
                }
                p = self.layers[j].parent;
                depth += 1;
            }
            if let Some(t) = l.tileset {
                let t = self.tilesets.get(t).ok_or("Missing tileset")?;
                if t.width == 0 || t.height == 0 {
                    return Err("Invalid tileset size".into());
                }
                let count = self.width.div_ceil(t.width) * self.height.div_ceil(t.height);
                if l.tile_cels.iter().enumerate().any(|(f, c)| {
                    let count = l.cel_meta[f]
                        .tilemap
                        .as_ref()
                        .map_or(count, |g| g.columns.saturating_mul(g.rows));
                    count > MAX_PIXELS
                        || c.len() != count
                        || c.iter().any(|v| (v & 0x1fffffff) as usize > t.tiles.len())
                }) {
                    return Err("Invalid tile reference".into());
                }
            }
        }
        for t in &self.tilesets {
            if t.width == 0
                || t.height == 0
                || t.width > MAX_DIMENSION
                || t.height > MAX_DIMENSION
                || t.width
                    .saturating_mul(t.height)
                    .saturating_mul(t.tiles.len())
                    > MAX_PIXELS
                || t.tiles.iter().any(|p| p.len() != t.width * t.height)
            {
                return Err("Invalid tileset".into());
            }
        }
        if self
            .tags
            .iter()
            .any(|t| t.from > t.to || t.to >= self.durations.len() || t.direction > 3)
        {
            return Err("Invalid animation range".into());
        }
        Ok(())
    }
    pub fn stored_pixels(&self) -> usize {
        self.width
            .saturating_mul(self.height)
            .saturating_mul(self.layers.len())
            .saturating_mul(self.durations.len())
    }
    pub fn can_grow(&self, pixels: usize) -> bool {
        self.stored_pixels().saturating_add(pixels) <= MAX_PIXELS
    }
    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            Some(y as usize * self.width + x as usize)
        } else {
            None
        }
    }
    pub fn palette_for(&self, f: usize) -> &[Pixel] {
        self.frame_palettes
            .get(f)
            .filter(|p| !p.is_empty())
            .map(Vec::as_slice)
            .unwrap_or(&self.palette)
    }
    pub fn editing_palette(&self, l: usize, f: usize) -> Vec<Pixel> {
        let mut p = self.palette_for(f).to_vec();
        if !self.layers[l].background {
            if let Some(c) = p.get_mut(self.transparent_index) {
                c[3] = 0;
            }
        }
        p
    }
    pub fn snapped_color(&self, f: usize, c: Pixel) -> Pixel {
        if self.palette_snap && self.color_mode == ColorMode::Rgba && c[3] > 0 {
            let p = self.palette_for(f);
            p[nearest(c, p)]
        } else {
            c
        }
    }
    fn drawing_color(&self, l: usize, f: usize, c: Pixel) -> (Pixel, u8) {
        if self.color_mode != ColorMode::Indexed {
            return (convert(c, self.color_mode, self.palette_for(f)), 0);
        }
        let p = self.editing_palette(l, f);
        let index = self
            .drawing_index
            .filter(|&i| p.get(i) == Some(&c))
            .unwrap_or_else(|| {
                if c[3] == 0 && !self.layers[l].background {
                    self.transparent_index.min(p.len() - 1)
                } else {
                    nearest(c, &p)
                }
            });
        (p[index], index as u8)
    }
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        let palettes: Vec<_> = (0..self.durations.len())
            .map(|f| self.palette_for(f).to_vec())
            .collect();
        self.color_mode = mode;
        for l in &mut self.layers {
            for (f, c) in l.cels.iter_mut().enumerate() {
                let mut effective = palettes[f].clone();
                if !l.background {
                    if let Some(p) = effective.get_mut(self.transparent_index) {
                        p[3] = 0;
                    }
                }
                let p = &effective;
                let old = l.cel_meta[f].indices.clone();
                let mut indices = Vec::with_capacity(c.len());
                for (i, v) in c.iter_mut().enumerate() {
                    if mode == ColorMode::Indexed {
                        let n = old
                            .as_ref()
                            .and_then(|a| a.get(i))
                            .copied()
                            .filter(|n| p.get(*n as usize) == Some(v))
                            .map(|n| n as usize)
                            .unwrap_or_else(|| {
                                if v[3] == 0 && !l.background {
                                    return self.transparent_index.min(p.len() - 1);
                                }
                                self.dithering.as_ref().map_or_else(
                                    || nearest(*v, p),
                                    |m| m.index(*v, p, i % self.width, i / self.width),
                                )
                            });
                        // Transparency is a mask, not a candidate shade for opaque artwork.
                        let n = if v[3] > 0 && !l.background && p[n][3] == 0 {
                            p.iter()
                                .enumerate()
                                .filter(|(_, c)| c[3] > 0)
                                .min_by_key(|(_, c)| {
                                    v.iter()
                                        .zip(c.iter())
                                        .map(|(a, b)| (*a as i32 - *b as i32).pow(2) as u32)
                                        .sum::<u32>()
                                })
                                .map(|(i, _)| i)
                                .unwrap_or(n)
                        } else {
                            n
                        };
                        indices.push(n as u8);
                        *v = p[n];
                    } else {
                        *v = convert(*v, mode, p);
                    }
                }
                l.cel_meta[f].indices = if mode == ColorMode::Indexed {
                    Some(Arc::new(indices))
                } else {
                    None
                };
            }
            for (f, m) in l.cel_meta.iter_mut().enumerate() {
                for v in &mut m.outside {
                    v.color = convert(v.color, mode, &palettes[f]);
                }
            }
        }
        for t in &mut self.tilesets {
            for c in &mut t.tiles {
                for v in c.iter_mut() {
                    *v = convert(*v, mode, &self.palette);
                }
            }
        }
    }
    /// Import colors while reserving a distinct slot for transparency in indexed sprites.
    pub fn import_palette(&mut self, mut palette: Vec<Pixel>) -> Result<(), String> {
        if palette.is_empty() || palette.len() > 256 {
            return Err("Palette must contain 1–256 colors".into());
        }
        if self.color_mode == ColorMode::Indexed {
            let transparent = if let Some(i) = palette.iter().position(|p| p[3] == 0) {
                i
            } else if palette.len() < 256 {
                palette.push(CLEAR);
                palette.len() - 1
            } else {
                return Err(
                    "Indexed palettes need a transparent slot; choose at most 255 opaque colors"
                        .into(),
                );
            };
            self.transparent_index = transparent;
        }
        self.palette = palette;
        self.frame_palettes.clear();
        self.drawing_index = None;
        self.set_color_mode(self.color_mode);
        Ok(())
    }
    pub fn set_palette_color(&mut self, index: usize, color: Pixel) {
        if index >= self.palette.len() {
            return;
        }
        let palettes: Vec<_> = (0..self.durations.len())
            .map(|f| self.palette_for(f).to_vec())
            .collect();
        if self.color_mode == ColorMode::Indexed {
            for l in &mut self.layers {
                for (f, c) in l.cels.iter_mut().enumerate() {
                    let mut effective = palettes[f].clone();
                    if !l.background {
                        if let Some(p) = effective.get_mut(self.transparent_index) {
                            p[3] = 0;
                        }
                    }
                    let p = &effective;
                    let indices = l.cel_meta[f].indices.clone();
                    for (i, v) in c.iter_mut().enumerate() {
                        let n = indices
                            .as_ref()
                            .and_then(|a| a.get(i))
                            .copied()
                            .filter(|n| p.get(*n as usize) == Some(v))
                            .map(|n| n as usize)
                            .unwrap_or_else(|| {
                                self.dithering.as_ref().map_or_else(
                                    || nearest(*v, p),
                                    |m| m.index(*v, p, i % self.width, i / self.width),
                                )
                            });
                        if n == index {
                            let mut replacement = color;
                            if index == self.transparent_index && !l.background {
                                replacement[3] = 0;
                            }
                            *v = replacement
                        }
                    }
                }
                for (f, m) in l.cel_meta.iter_mut().enumerate() {
                    for v in &mut m.outside {
                        if palettes[f].get(index) == Some(&v.color) {
                            v.color = color;
                        }
                    }
                }
            }
            let old = self.palette[index];
            for t in &mut self.tilesets {
                for c in &mut t.tiles {
                    for v in c.iter_mut() {
                        if *v == old {
                            *v = color
                        }
                    }
                }
            }
        }
        self.palette[index] = color;
        for p in &mut self.frame_palettes {
            if let Some(v) = p.get_mut(index) {
                *v = color;
            }
        }
    }
    pub fn set_pixel(&mut self, l: usize, f: usize, i: usize, color: Pixel) {
        if !self.editable(l) || self.layers[l].tileset.is_some() {
            return;
        }
        let (color, n) = self.drawing_color(l, f, color);
        self.layers[l].cels[f][i] = color;
        if let Some(indices) = &mut self.layers[l].cel_meta[f].indices {
            Arc::make_mut(indices)[i] = n;
        }
    }
    pub fn editable(&self, l: usize) -> bool {
        let mut p = Some(l);
        let mut n = 0;
        while let Some(i) = p {
            let Some(layer) = self.layers.get(i) else {
                return false;
            };
            if layer.locked || n > self.layers.len() {
                return false;
            }
            p = layer.parent;
            n += 1;
        }
        !self.layers[l].group
    }
    pub fn stamp(&mut self, l: usize, f: usize, x: i32, y: i32, c: Pixel, size: i32) {
        if !self.editable(l) || self.layers[l].tileset.is_some() {
            return;
        }
        let (c, ci) = self.drawing_color(l, f, self.snapped_color(f, c));
        let size = size.clamp(1, MAX_BRUSH_SIZE);
        let lo = -(size / 2);
        for dy in lo..lo + size {
            for dx in lo..lo + size {
                let (a, b) = (
                    2 * dx + if size % 2 == 0 { 1 } else { 0 },
                    2 * dy + if size % 2 == 0 { 1 } else { 0 },
                );
                let accept = match self.brush_shape {
                    BrushShape::Round => a * a + b * b <= size * size,
                    BrushShape::Square => true,
                    BrushShape::Diamond => a.abs() + b.abs() <= size,
                    BrushShape::Horizontal => dy == 0,
                    BrushShape::Vertical => dx == 0,
                };
                if accept {
                    if let Some(i) = self.drawing_position(x + dx, y + dy) {
                        self.layers[l].cels[f][i] = c;
                        if let Some(indices) = &mut self.layers[l].cel_meta[f].indices {
                            Arc::make_mut(indices)[i] = ci;
                        }
                    }
                }
            }
        }
    }
    pub fn line(&mut self, l: usize, f: usize, a: (i32, i32), b: (i32, i32), c: Pixel, size: i32) {
        let (mut x, mut y) = a;
        let dx = (b.0 - x).abs();
        let sx = if x < b.0 { 1 } else { -1 };
        let dy = -(b.1 - y).abs();
        let sy = if y < b.1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.stamp(l, f, x, y, c, size);
            if (x, y) == b {
                break;
            }
            let e = err * 2;
            if e >= dy {
                err += dy;
                x += sx;
            }
            if e <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    pub fn ellipse(
        &mut self,
        l: usize,
        f: usize,
        a: (i32, i32),
        b: (i32, i32),
        c: Pixel,
        filled: bool,
        size: i32,
    ) {
        let (mut x0, mut y0, mut x1, mut y1) = (
            a.0.min(b.0) as i64,
            a.1.min(b.1) as i64,
            a.0.max(b.0) as i64,
            a.1.max(b.1) as i64,
        );
        let a = x1 - x0;
        let b = y1 - y0;
        let mut b1 = b & 1;
        let mut dx = 4 * (1 - a) * b * b;
        let mut dy = 4 * (b1 + 1) * a * a;
        let mut err = dx + dy + b1 * a * a;
        y0 += (b + 1) / 2;
        y1 = y0 - b1;
        let aa = 8 * a * a;
        b1 = 8 * b * b;
        let mut draw = |lx: i64, rx: i64, y: i64| {
            if filled {
                self.line(l, f, (lx as i32, y as i32), (rx as i32, y as i32), c, 1)
            } else {
                self.stamp(l, f, lx as i32, y as i32, c, size);
                self.stamp(l, f, rx as i32, y as i32, c, size);
            }
        };
        loop {
            draw(x0, x1, y0);
            draw(x0, x1, y1);
            let e2 = 2 * err;
            if e2 <= dy {
                y0 += 1;
                y1 -= 1;
                dy += aa;
                err += dy;
            }
            if e2 >= dx || 2 * err > dy {
                x0 += 1;
                x1 -= 1;
                dx += b1;
                err += dx;
            }
            if x0 > x1 {
                break;
            }
        }
        while y0 - y1 < b {
            draw(x0 - 1, x1 + 1, y0);
            y0 += 1;
            draw(x0 - 1, x1 + 1, y1);
            y1 -= 1;
        }
    }
    pub fn drawing_position(&self, x: i32, y: i32) -> Option<usize> {
        if self.wraparound {
            self.index(
                x.rem_euclid(self.width as i32),
                y.rem_euclid(self.height as i32),
            )
        } else {
            self.index(x, y)
        }
    }
    pub fn fill(&mut self, l: usize, f: usize, x: i32, y: i32, c: Pixel) {
        if !self.editable(l) || self.layers[l].tileset.is_some() {
            return;
        }
        let Some(i) = self.drawing_position(x, y) else {
            return;
        };
        let (x, y) = ((i % self.width) as i32, (i / self.width) as i32);
        let (c, ci) = self.drawing_color(l, f, self.snapped_color(f, c));
        let old = self.layers[l].cels[f][i];
        if old == c {
            return;
        }
        let mut stack = vec![(x, y)];
        self.layers[l].cels[f][i] = c;
        if let Some(indices) = &mut self.layers[l].cel_meta[f].indices {
            Arc::make_mut(indices)[i] = ci;
        }
        while let Some((x, y)) = stack.pop() {
            for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if let Some(n) = self.drawing_position(nx, ny) {
                    if self.layers[l].cels[f][n] == old {
                        self.layers[l].cels[f][n] = c;
                        if let Some(indices) = &mut self.layers[l].cel_meta[f].indices {
                            Arc::make_mut(indices)[n] = ci;
                        }
                        stack.push(((n % self.width) as i32, (n / self.width) as i32));
                    }
                }
            }
        }
    }
    pub fn layer_pixels(&self, l: usize, f: usize) -> Vec<Pixel> {
        let layer = &self.layers[l];
        let Some(ti) = layer.tileset else {
            return layer.cels[f].to_vec();
        };
        let Some(t) = self.tilesets.get(ti) else {
            return vec![CLEAR; self.width * self.height];
        };
        let geometry = layer.cel_meta[f].tilemap.as_ref();
        let cols = geometry.map_or(self.width.div_ceil(t.width), |g| g.columns);
        let (ox, oy) = geometry.map_or((0, 0), |g| (g.x, g.y));
        let mut out = vec![CLEAR; self.width * self.height];
        for (i, &id) in layer.tile_cels[f].iter().enumerate() {
            let n = (id & 0x1fffffff) as usize;
            if n == 0 {
                continue;
            }
            let Some(tile) = t.tiles.get(n - 1) else {
                continue;
            };
            for y in 0..t.height {
                for x in 0..t.width {
                    let (mut sx, mut sy) = (x, y);
                    if id & 0x20000000 != 0 {
                        std::mem::swap(&mut sx, &mut sy);
                    }
                    if sx >= t.width || sy >= t.height {
                        continue;
                    }
                    if id & 0x80000000 != 0 {
                        sx = t.width - 1 - sx
                    }
                    if id & 0x40000000 != 0 {
                        sy = t.height - 1 - sy
                    }
                    if let Some(p) = self.index(
                        ox + (i % cols * t.width + x) as i32,
                        oy + (i / cols * t.height + y) as i32,
                    ) {
                        out[p] = tile[sy * t.width + sx];
                    }
                }
            }
        }
        out
    }
    pub fn composite(&self, f: usize) -> Vec<Pixel> {
        self.composite_group(f, None, 0)
    }
    fn composite_group(&self, f: usize, parent: Option<usize>, depth: usize) -> Vec<Pixel> {
        let mut out = vec![CLEAR; self.width * self.height];
        self.composite_into(f, parent, depth, &mut out);
        out
    }
    fn composite_into(&self, f: usize, parent: Option<usize>, depth: usize, out: &mut Vec<Pixel>) {
        if depth > self.layers.len() {
            return;
        }
        let mut order: Vec<_> = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.parent == parent && l.visible)
            .map(|(i, l)| (i as i32 + l.cel_meta[f].z_index as i32, i))
            .collect();
        order.sort_by_key(|p| (p.0, self.layers[p.1].cel_meta[f].z_index));
        for (_, i) in order {
            let l = &self.layers[i];
            if l.group && !self.compose_groups {
                self.composite_into(f, Some(i), depth + 1, out);
                continue;
            }
            let pixels = if l.group {
                self.composite_group(f, Some(i), depth + 1)
            } else {
                self.layer_pixels(i, f)
            };
            let opacity = ((l.opacity as u16 * l.cel_meta[f].opacity as u16 + 127) / 255) as u8;
            for (d, s) in out.iter_mut().zip(pixels) {
                *d = blend(*d, s, opacity, l.blend);
            }
        }
    }
    pub fn add_frame(&mut self, f: usize, duplicate: bool) {
        if !self.can_grow(self.width * self.height * self.layers.len()) {
            return;
        }
        for l in &mut self.layers {
            l.cels.insert(
                f + 1,
                if duplicate {
                    l.cels[f].clone()
                } else {
                    vec![CLEAR; self.width * self.height].into()
                },
            );
            l.cel_meta.insert(
                f + 1,
                if duplicate {
                    l.cel_meta[f].clone()
                } else {
                    CelMeta::default()
                },
            );
            l.tile_cels.insert(
                f + 1,
                if duplicate {
                    l.tile_cels[f].clone()
                } else {
                    vec![0; l.tile_cels[f].len()]
                },
            );
        }
        self.durations.insert(f + 1, self.durations[f]);
        if !self.frame_palettes.is_empty() {
            let p = self
                .frame_palettes
                .get(f)
                .cloned()
                .unwrap_or_else(|| self.palette.clone());
            self.frame_palettes.insert(f + 1, p);
        }
        for t in &mut self.tags {
            if t.from > f {
                t.from += 1
            }
            if t.to >= f {
                t.to += 1
            }
        }
        for m in &mut self.metadata {
            if m.frame > f {
                m.frame += 1
            }
        }
    }
    pub fn remove_frame(&mut self, f: usize) {
        if self.durations.len() > 1 {
            self.durations.remove(f);
            if f < self.frame_palettes.len() {
                self.frame_palettes.remove(f);
            }
            for l in &mut self.layers {
                l.cels.remove(f);
                l.cel_meta.remove(f);
                l.tile_cels.remove(f);
            }
            self.tags.retain(|t| !(t.from == f && t.to == f));
            for t in &mut self.tags {
                if t.from > f {
                    t.from -= 1
                }
                if t.to >= f {
                    t.to = t.to.saturating_sub(1)
                }
            }
            self.metadata.retain(|m| m.frame != f);
            for m in &mut self.metadata {
                if m.frame > f {
                    m.frame -= 1
                }
            }
        }
    }
    pub fn add_layer(&mut self) {
        if self.can_grow(self.width * self.height * self.durations.len()) {
            self.layers.push(Layer::new(
                format!("Layer {}", self.layers.len() + 1),
                self.width,
                self.height,
                self.durations.len(),
            ));
        }
    }
    pub fn add_group(&mut self, name: &str) -> Result<usize, String> {
        if !self.can_grow(self.width * self.height * self.durations.len()) {
            return Err("Group limit reached for this canvas size".into());
        }
        self.add_layer();
        let i = self.layers.len() - 1;
        self.layers[i].group = true;
        self.layers[i].name = name.into();
        Ok(i)
    }
    pub fn add_tile_layer(&mut self, tileset: usize) -> Result<(), String> {
        let t = self.tilesets.get(tileset).ok_or("Missing tileset")?;
        let count = self.width.div_ceil(t.width) * self.height.div_ceil(t.height);
        if !self.can_grow(self.width * self.height * self.durations.len()) {
            return Err("Layer pixel limit reached".into());
        }
        self.add_layer();
        let l = self.layers.last_mut().unwrap();
        l.tileset = Some(tileset);
        l.name = "Tile layer".into();
        l.tile_cels = vec![vec![0; count]; self.durations.len()];
        Ok(())
    }
    pub fn paint_tile(
        &mut self,
        l: usize,
        f: usize,
        x: usize,
        y: usize,
        tile: u32,
    ) -> Result<(), String> {
        if !self.editable(l) {
            return Err("Layer is locked".into());
        }
        let t = self.layers[l]
            .tileset
            .and_then(|i| self.tilesets.get(i))
            .ok_or("Select a tile layer")?;
        let geometry = self.layers[l].cel_meta[f].tilemap.as_ref();
        let cols = geometry.map_or(self.width.div_ceil(t.width), |g| g.columns);
        let rows = geometry.map_or(self.height.div_ceil(t.height), |g| g.rows);
        if x >= cols || y >= rows || (tile & 0x1fffffff) as usize > t.tiles.len() {
            return Err("Tile is outside the map or tileset".into());
        }
        self.layers[l].tile_cels[f][y * cols + x] = tile;
        Ok(())
    }
    pub fn remove_layer(&mut self, index: usize) {
        if self.layers.len() <= 1 {
            return;
        }
        let parent = self.layers[index].parent;
        self.layers.remove(index);
        for l in &mut self.layers {
            if l.parent == Some(index) {
                l.parent = parent;
            }
            if let Some(p) = l.parent {
                if p > index {
                    l.parent = Some(p - 1);
                }
            }
        }
        self.metadata
            .retain(|m| m.owner != format!("layer:{index}") && m.owner != format!("cel:{index}"));
        for m in &mut self.metadata {
            for prefix in ["layer:", "cel:"] {
                if let Some(i) = m
                    .owner
                    .strip_prefix(prefix)
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    if i > index {
                        m.owner = format!("{prefix}{}", i - 1);
                    }
                }
            }
        }
    }
    pub fn swap_layers(&mut self, a: usize, b: usize) {
        self.layers.swap(a, b);
        for l in &mut self.layers {
            l.parent = l.parent.map(|p| {
                if p == a {
                    b
                } else if p == b {
                    a
                } else {
                    p
                }
            });
        }
        for m in &mut self.metadata {
            for prefix in ["layer:", "cel:"] {
                if let Some(i) = m
                    .owner
                    .strip_prefix(prefix)
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    m.owner = format!(
                        "{prefix}{}",
                        if i == a {
                            b
                        } else if i == b {
                            a
                        } else {
                            i
                        }
                    );
                }
            }
        }
    }
    pub fn flip(&mut self, l: usize, f: usize, horizontal: bool) {
        if !self.editable(l) {
            return;
        }
        let old = self.layers[l].cels[f].clone();
        let old_indices = self.layers[l].cel_meta[f].indices.clone();
        for y in 0..self.height {
            for x in 0..self.width {
                let (sx, sy) = if horizontal {
                    (self.width - 1 - x, y)
                } else {
                    (x, self.height - 1 - y)
                };
                self.layers[l].cels[f][y * self.width + x] = old[sy * self.width + sx];
                if let (Some(target), Some(source)) =
                    (&mut self.layers[l].cel_meta[f].indices, &old_indices)
                {
                    Arc::make_mut(target)[y * self.width + x] = source[sy * self.width + sx];
                }
            }
        }
    }
    pub fn demo() -> Self {
        let mut d = Self::new(48, 48);
        d.layers[0].name = "Floating island".into();
        let edge = [55, 49, 67, 255];
        let soil = [125, 82, 79, 255];
        let shade = [88, 61, 73, 255];
        let green = [112, 174, 106, 255];
        let light = [178, 208, 118, 255];
        for y in 28..44 {
            for x in 6..43 {
                let dx = (x as i32 - 24).abs();
                let inside = if y < 33 {
                    dx < 18 - (y as i32 - 30).abs()
                } else {
                    dx < 18 - (y as i32 - 32) * 2
                };
                if inside {
                    let c = if y < 31 {
                        green
                    } else if y == 31 {
                        edge
                    } else if y > 39 || x % 9 == 0 {
                        shade
                    } else {
                        soil
                    };
                    d.layers[0].cels[0][y * 48 + x] = c;
                }
            }
        }
        for x in 11..38 {
            if x % 3 != 0 {
                d.layers[0].cels[0][29 * 48 + x] = light;
            }
        }
        for (x, y) in [(12, 34), (18, 37), (28, 34), (31, 38), (24, 41)] {
            d.layers[0].cels[0][y * 48 + x] = [180, 125, 98, 255];
        }
        d.add_layer();
        d.layers[1].name = "Garden".into();
        // Original hand-authored pixel motif: a small red mushroom and two flowers.
        for y in 15..30 {
            for x in 15..33 {
                let dx = (x as i32 - 24).abs();
                let c = if (23..30).contains(&y) && (22..27).contains(&x) {
                    Some(if x == 22 {
                        [177, 152, 139, 255]
                    } else {
                        [248, 229, 178, 255]
                    })
                } else if (15..23).contains(&y) && dx < 9 - (18 - y as i32).max(0) {
                    Some(if y == 22 || dx == 8 {
                        edge
                    } else if x < 21 {
                        [176, 65, 83, 255]
                    } else {
                        [231, 104, 104, 255]
                    })
                } else {
                    None
                };
                if let Some(c) = c {
                    d.layers[1].cels[0][y * 48 + x] = c;
                }
            }
        }
        for (x, y) in [(21, 17), (27, 19), (18, 20)] {
            for dy in 0..2 {
                for dx in 0..2 {
                    d.layers[1].cels[0][(y + dy) * 48 + x + dx] = [255, 226, 179, 255];
                }
            }
        }
        for (x, y) in [(11, 25), (36, 24)] {
            d.line(1, 0, (x, y), (x, y + 5), [76, 114, 92, 255], 1);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                d.stamp(1, 0, x + dx, y + dy, [233, 174, 210, 255], 1);
            }
            d.stamp(1, 0, x, y, [255, 226, 138, 255], 1);
        }
        d.add_layer();
        d.layers[2].name = "Fireflies".into();
        for f in 0..4 {
            if f > 0 {
                d.add_frame(f - 1, true)
            }
            for (x, y) in [(9 + f as i32, 14), (36, 11 + f as i32), (31 - f as i32, 6)] {
                d.stamp(2, f, x, y, [254, 218, 136, 255], 1);
            }
        }
        d
    }
}
