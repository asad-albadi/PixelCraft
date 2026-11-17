use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod cel_selection;
mod frame_order;
mod layer_conversion;
mod layer_operations;
mod layer_order;
mod tile_operations;
pub use cel_selection::CelSelection;
pub use layer_conversion::LayerConversion;
pub use layer_order::LayerDrop;
pub use tile_operations::{TileDefinitionEdit, TilePixelEdit, TileTransform};
pub type Pixel = [u8; 4];
pub const TILE_ID_MASK: u32 = 0x1fff_ffff;
pub const CLEAR: Pixel = [0; 4];
pub const DEFAULT_DURATION: u16 = 100;
pub const MAX_FRAME_NAME_CHARS: usize = 256;
pub const MIN_BRUSH_SIZE: i32 = 1;
pub const MAX_BRUSH_SIZE: i32 = 64;
pub const MAX_DIMENSION: usize = 16384;
pub const MAX_PIXELS: usize = 64_000_000;
pub const MAX_DOCUMENT_PIXELS: usize = 128_000_000;
pub(crate) const MAX_CEL_SLOTS: usize = 1_000_000;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DrawingGrid {
    pub origin: [i32; 2],
    pub size: [u32; 2],
}

impl Default for DrawingGrid {
    fn default() -> Self {
        Self {
            origin: [0, 0],
            size: [16, 16],
        }
    }
}

impl DrawingGrid {
    pub fn valid(self) -> bool {
        self.origin
            .iter()
            .all(|&value| i16::try_from(value).is_ok())
            && self
                .size
                .iter()
                .all(|&value| value != 0 && u16::try_from(value).is_ok())
    }

    pub fn snap(self, point: (i32, i32), bounds: (i32, i32, i32, i32)) -> (i32, i32) {
        let axis = |point: i32, origin: i32, step: u32, low: i32, high: i32| {
            let (p, o, s) = (i64::from(point), i64::from(origin), i64::from(step));
            if s == 0 {
                return point;
            }
            let first = (i64::from(low) - o + s - 1).div_euclid(s);
            let last = (i64::from(high) - o).div_euclid(s);
            if first > last {
                return point;
            }
            let nearest = (p - o + s / 2).div_euclid(s).clamp(first, last);
            (o + nearest * s) as i32
        };
        (
            axis(point.0, self.origin[0], self.size[0], bounds.0, bounds.1),
            axis(point.1, self.origin[1], self.size[1], bounds.2, bounds.3),
        )
    }

    pub(crate) fn from_ase_header(header: &[u8]) -> Option<Self> {
        let bytes: &[u8; 8] = header.get(36..44)?.try_into().ok()?;
        let grid = Self {
            origin: [
                i16::from_le_bytes([bytes[0], bytes[1]]) as i32,
                i16::from_le_bytes([bytes[2], bytes[3]]) as i32,
            ],
            size: [
                u16::from_le_bytes([bytes[4], bytes[5]]) as u32,
                u16::from_le_bytes([bytes[6], bytes[7]]) as u32,
            ],
        };
        grid.valid().then_some(grid)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WrapAxes {
    pub horizontal: bool,
    pub vertical: bool,
}

impl WrapAxes {
    pub const NONE: Self = Self {
        horizontal: false,
        vertical: false,
    };
    pub const HORIZONTAL: Self = Self {
        horizontal: true,
        vertical: false,
    };
    pub const VERTICAL: Self = Self {
        horizontal: false,
        vertical: true,
    };
    pub const BOTH: Self = Self {
        horizontal: true,
        vertical: true,
    };

    pub fn position(self, size: (usize, usize), (mut x, mut y): (i32, i32)) -> Option<(i32, i32)> {
        let (width, height) = (i32::try_from(size.0).ok()?, i32::try_from(size.1).ok()?);
        if width == 0 || height == 0 {
            return None;
        }
        if self.horizontal {
            x = x.rem_euclid(width);
        }
        if self.vertical {
            y = y.rem_euclid(height);
        }
        ((0..width).contains(&x) && (0..height).contains(&y)).then_some((x, y))
    }
}

impl From<bool> for WrapAxes {
    fn from(value: bool) -> Self {
        if value { Self::BOTH } else { Self::NONE }
    }
}

pub(crate) fn line_pixels<E>(
    a: (i32, i32),
    b: (i32, i32),
    mut visit: impl FnMut((i32, i32)) -> Result<(), E>,
) -> Result<(), E> {
    let (mut x, mut y) = a;
    let dx = (b.0 - x).abs();
    let sx = if x < b.0 { 1 } else { -1 };
    let dy = -(b.1 - y).abs();
    let sy = if y < b.1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        visit((x, y))?;
        if (x, y) == b {
            return Ok(());
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
thread_local! {
    static PROJECT_DECODE_BUDGET: std::cell::Cell<Option<ProjectDecodeBudget>> =
        const { std::cell::Cell::new(None) };
}

#[derive(Clone, Copy)]
struct ProjectDecodeBudget {
    bytes: usize,
    cel_slots: [usize; 3],
}

/// Keep the native wire representation while accounting for allocations across
/// the entire document, before any shared raster references are restored.
pub(crate) fn with_project_decode_budget<T>(bytes: usize, decode: impl FnOnce() -> T) -> T {
    struct ResetBudget;
    impl Drop for ResetBudget {
        fn drop(&mut self) {
            PROJECT_DECODE_BUDGET.set(None);
        }
    }
    if PROJECT_DECODE_BUDGET.get().is_some() {
        return decode();
    }
    PROJECT_DECODE_BUDGET.set(Some(ProjectDecodeBudget {
        bytes,
        cel_slots: [MAX_CEL_SLOTS; 3],
    }));
    let _reset = ResetBudget;
    decode()
}

fn charge_project_bytes<E: serde::de::Error>(bytes: usize) -> Result<(), E> {
    if let Some(mut limit) = PROJECT_DECODE_BUDGET.get() {
        limit.bytes = limit
            .bytes
            .checked_sub(bytes)
            .ok_or_else(|| E::custom("Project exceeds its raster allocation budget"))?;
        PROJECT_DECODE_BUDGET.set(Some(limit));
    }
    Ok(())
}

fn charge_cel_slot<E: serde::de::Error, const KIND: usize>() -> Result<(), E> {
    if let Some(mut limit) = PROJECT_DECODE_BUDGET.get() {
        limit.cel_slots[KIND] = limit.cel_slots[KIND]
            .checked_sub(1)
            .ok_or_else(|| E::custom("Project exceeds its cel slot limit"))?;
        PROJECT_DECODE_BUDGET.set(Some(limit));
    }
    Ok(())
}

// Use incremental visitors for the arrays that can dominate a project allocation.
// A separate slot counter covers vectors of empty cels, which have no raster backing.
struct BudgetedVec<T, const KIND: usize>(Vec<T>);

impl<'de, T: Deserialize<'de>, const KIND: usize> Deserialize<'de> for BudgetedVec<T, KIND> {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Entries<T, const KIND: usize>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const KIND: usize> serde::de::Visitor<'de> for Entries<T, KIND> {
            type Value = BudgetedVec<T, KIND>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a project array")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element()? {
                    if KIND < 3 {
                        if values.len() == u16::MAX as usize {
                            return Err(serde::de::Error::custom("Layer exceeds its frame limit"));
                        }
                        charge_cel_slot::<A::Error, KIND>()?;
                    } else {
                        charge_project_bytes(std::mem::size_of::<T>())?;
                    }
                    values.push(value);
                }
                Ok(BudgetedVec(values))
            }
        }
        decoder.deserialize_seq(Entries::<T, KIND>(std::marker::PhantomData))
    }
}

fn decode_raster_slots<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Vec<Pixels>, D::Error> {
    BudgetedVec::<Pixels, 0>::deserialize(decoder).map(|values| values.0)
}
fn decode_metadata_slots<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Vec<CelMeta>, D::Error> {
    BudgetedVec::<CelMeta, 1>::deserialize(decoder).map(|values| values.0)
}
fn decode_tile_slots<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Vec<Vec<u32>>, D::Error> {
    BudgetedVec::<BudgetedVec<u32, 3>, 2>::deserialize(decoder)
        .map(|slots| slots.0.into_iter().map(|values| values.0).collect())
}
fn decode_outside_pixels<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Vec<OutsidePixel>, D::Error> {
    BudgetedVec::<OutsidePixel, 3>::deserialize(decoder).map(|values| values.0)
}
fn decode_indices<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<Arc<Vec<u8>>>, D::Error> {
    Option::<BudgetedVec<u8, 3>>::deserialize(decoder)
        .map(|value| value.map(|values| Arc::new(values.0)))
}

const PIXEL_BLOCK: usize = 16;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SparsePixels {
    len: usize,
    blocks: std::collections::BTreeMap<usize, [Pixel; PIXEL_BLOCK]>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Pixels {
    Empty { clear_pixels: usize },
    Sparse(Arc<SparsePixels>),
    Dense(Arc<Vec<Pixel>>),
}
impl<'de> Deserialize<'de> for Pixels {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        use serde::de::{Error, MapAccess, SeqAccess, Visitor, value::MapAccessDeserializer};
        struct PixelVisitor;
        impl<'de> Visitor<'de> for PixelVisitor {
            type Value = Pixels;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a pixel array or compact raster")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Pixels, A::Error> {
                let mut pixels = Vec::new();
                while let Some(pixel) = seq.next_element()? {
                    if pixels.len() == MAX_PIXELS {
                        return Err(A::Error::custom("Raster exceeds pixel limit"));
                    }
                    charge_project_bytes(std::mem::size_of::<Pixel>())?;
                    pixels.push(pixel);
                }
                Ok(pixels.into())
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Pixels, A::Error> {
                #[derive(Deserialize)]
                struct Compact {
                    clear_pixels: Option<usize>,
                    len: Option<usize>,
                    #[serde(default, deserialize_with = "decode_sparse_blocks")]
                    blocks: Option<std::collections::BTreeMap<usize, [Pixel; PIXEL_BLOCK]>>,
                }
                let compact = Compact::deserialize(MapAccessDeserializer::new(map))?;
                match (compact.clear_pixels, compact.len, compact.blocks) {
                    (Some(len), None, None) if len <= MAX_PIXELS => Ok(Pixels::empty(len)),
                    (None, Some(len), Some(blocks))
                        if len <= MAX_PIXELS
                            && blocks.keys().all(|key| *key < len.div_ceil(PIXEL_BLOCK))
                            && blocks.len() <= len.div_ceil(PIXEL_BLOCK) =>
                    {
                        Ok(Pixels::Sparse(Arc::new(SparsePixels { len, blocks })))
                    }
                    _ => Err(A::Error::custom("Invalid compact raster dimensions")),
                }
            }
        }
        decoder.deserialize_any(PixelVisitor)
    }
}
fn decode_sparse_blocks<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<std::collections::BTreeMap<usize, [Pixel; PIXEL_BLOCK]>>, D::Error> {
    struct Blocks;
    impl<'de> serde::de::Visitor<'de> for Blocks {
        type Value = Option<std::collections::BTreeMap<usize, [Pixel; PIXEL_BLOCK]>>;

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: serde::Deserializer<'de>>(
            self,
            decoder: D,
        ) -> Result<Self::Value, D::Error> {
            decoder.deserialize_map(self)
        }

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a sparse raster block map")
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            use serde::de::Error;
            let mut blocks = std::collections::BTreeMap::new();
            while let Some((key, pixels)) = map.next_entry::<usize, [Pixel; PIXEL_BLOCK]>()? {
                if key >= MAX_PIXELS.div_ceil(PIXEL_BLOCK) {
                    return Err(A::Error::custom("Invalid sparse raster block index"));
                }
                if let std::collections::btree_map::Entry::Vacant(slot) = blocks.entry(key) {
                    charge_project_bytes((PIXEL_BLOCK + 8) * std::mem::size_of::<Pixel>())?;
                    slot.insert(pixels);
                } else {
                    return Err(A::Error::custom("Duplicate sparse raster block"));
                }
            }
            Ok(Some(blocks))
        }
    }
    decoder.deserialize_option(Blocks)
}

impl Pixels {
    pub fn empty(len: usize) -> Self {
        Self::Empty { clear_pixels: len }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty { clear_pixels } => *clear_pixels,
            Self::Sparse(raster) => raster.len,
            Self::Dense(pixels) => pixels.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn backing_id(&self) -> Option<usize> {
        match self {
            Self::Empty { .. } => None,
            Self::Sparse(raster) => Some(Arc::as_ptr(raster) as usize),
            Self::Dense(pixels) => Some(Arc::as_ptr(pixels) as usize),
        }
    }

    pub fn allocation_pixels(&self) -> usize {
        match self {
            Self::Empty { .. } => 0,
            Self::Sparse(raster) => raster.blocks.len() * (PIXEL_BLOCK + 8),
            Self::Dense(pixels) => pixels.len(),
        }
    }

    pub fn shares_storage(&self, other: &Self) -> bool {
        self.len() == other.len() && self.backing_id() == other.backing_id()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Pixel> {
        (0..self.len()).map(|index| &self[index])
    }

    pub fn occupied(&self) -> bool {
        match self {
            Self::Empty { .. } => false,
            Self::Sparse(raster) => raster.blocks.values().flatten().any(|p| p[3] != 0),
            Self::Dense(pixels) => pixels.iter().any(|p| p[3] != 0),
        }
    }

    pub fn visit_nonclear(&self, mut visit: impl FnMut(usize, Pixel)) {
        match self {
            Self::Empty { .. } => {}
            Self::Sparse(raster) => {
                for (&block, pixels) in &raster.blocks {
                    for (offset, &pixel) in pixels.iter().enumerate() {
                        let index = block * PIXEL_BLOCK + offset;
                        if index < raster.len && pixel != CLEAR {
                            visit(index, pixel);
                        }
                    }
                }
            }
            Self::Dense(pixels) => {
                for (index, &pixel) in pixels.iter().enumerate().filter(|(_, p)| **p != CLEAR) {
                    visit(index, pixel);
                }
            }
        }
    }

    pub fn to_vec(&self) -> Vec<Pixel> {
        if let Self::Dense(pixels) = self {
            return pixels.as_ref().clone();
        }
        let mut out = vec![CLEAR; self.len()];
        self.visit_nonclear(|index, pixel| out[index] = pixel);
        out
    }

    pub fn contains(&self, color: &Pixel) -> bool {
        self.iter().any(|pixel| pixel == color)
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Pixel> {
        self.make_dense().iter_mut()
    }

    pub fn fill(&mut self, color: Pixel) {
        if color == CLEAR {
            *self = Self::empty(self.len());
        } else {
            self.make_dense().fill(color);
        }
    }

    fn make_dense(&mut self) -> &mut Vec<Pixel> {
        if !matches!(self, Self::Dense(_)) {
            *self = self.to_vec().into();
        }
        let Self::Dense(pixels) = self else {
            unreachable!()
        };
        Arc::make_mut(pixels)
    }

    pub fn set(&mut self, index: usize, color: Pixel) {
        if self[index] == color {
            return;
        }
        self[index] = color;
        if color == CLEAR
            && let Self::Sparse(raster) = self
        {
            let raster = Arc::make_mut(raster);
            let block = index / PIXEL_BLOCK;
            if raster.blocks[&block].iter().all(|p| *p == CLEAR) {
                raster.blocks.remove(&block);
            }
            if raster.blocks.is_empty() {
                *self = Self::empty(raster.len);
            }
        }
    }
}
impl PartialEq for Pixels {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        if self.shares_storage(other) {
            return true;
        }
        if let (Self::Dense(a), Self::Dense(b)) = (self, other) {
            return a == b;
        }
        let mut equal = true;
        self.visit_nonclear(|i, pixel| equal &= other[i] == pixel);
        if equal {
            other.visit_nonclear(|i, pixel| equal &= self[i] == pixel);
        }
        equal
    }
}
impl From<Vec<Pixel>> for Pixels {
    fn from(v: Vec<Pixel>) -> Self {
        Self::Dense(Arc::new(v))
    }
}
impl FromIterator<Pixel> for Pixels {
    fn from_iter<T: IntoIterator<Item = Pixel>>(v: T) -> Self {
        Vec::from_iter(v).into()
    }
}
impl std::ops::Index<usize> for Pixels {
    type Output = Pixel;
    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len(), "Pixel index out of bounds");
        match self {
            Self::Empty { .. } => &CLEAR,
            Self::Sparse(raster) => raster
                .blocks
                .get(&(index / PIXEL_BLOCK))
                .map_or(&CLEAR, |block| &block[index % PIXEL_BLOCK]),
            Self::Dense(pixels) => &pixels[index],
        }
    }
}
impl std::ops::IndexMut<usize> for Pixels {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let len = self.len();
        assert!(index < len, "Pixel index out of bounds");
        let block = index / PIXEL_BLOCK;
        let new_block = match self {
            Self::Empty { .. } => true,
            Self::Sparse(raster) => !raster.blocks.contains_key(&block),
            Self::Dense(_) => false,
        };
        if new_block && self.allocation_pixels() + PIXEL_BLOCK + 8 >= len {
            return &mut self.make_dense()[index];
        }
        if let Self::Empty { .. } = self {
            *self = Self::Sparse(Arc::new(SparsePixels {
                len,
                blocks: std::collections::BTreeMap::new(),
            }));
        }
        match self {
            Self::Sparse(raster) => &mut Arc::make_mut(raster)
                .blocks
                .entry(block)
                .or_insert([CLEAR; PIXEL_BLOCK])[index % PIXEL_BLOCK],
            Self::Dense(pixels) => &mut Arc::make_mut(pixels)[index],
            Self::Empty { .. } => unreachable!(),
        }
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
/// Runtime targeting identity. Snapshots retain it, while reopened files
/// get a fresh identity so a client cannot target a different open document.
#[derive(Clone, Copy, Debug)]
pub struct EditorId(u64);

impl EditorId {
    /// Identifies a runtime target for in-process API bridges.
    pub fn runtime_key(self) -> u64 {
        self.0
    }
}

impl Default for EditorId {
    fn default() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl PartialEq for EditorId {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Display for EditorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CelKeyframe {
    pub x: i32,
    pub y: i32,
    pub opacity: u8,
    #[serde(default)]
    pub hold: bool,
}

impl Default for CelKeyframe {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            opacity: 255,
            hold: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CelMeta {
    #[serde(skip)]
    pub editor_id: EditorId,
    pub opacity: u8,
    pub z_index: i16,
    #[serde(deserialize_with = "decode_outside_pixels")]
    pub outside: Vec<OutsidePixel>,
    #[serde(default, deserialize_with = "decode_indices")]
    pub indices: Option<Arc<Vec<u8>>>,
    #[serde(default)]
    pub tilemap: Option<TilemapGeometry>,
    #[serde(default)]
    pub bounds: Option<[i32; 4]>,
    /// ASE reference placement in signed 16.16 sprite coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_bounds: Option<[i32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_link: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyframe: Option<CelKeyframe>,
}
impl Default for CelMeta {
    fn default() -> Self {
        Self {
            editor_id: EditorId::default(),
            opacity: 255,
            z_index: 0,
            outside: vec![],
            indices: None,
            tilemap: None,
            bounds: None,
            reference_bounds: None,
            artwork_link: None,
            keyframe: None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CelRange {
    pub first_layer: usize,
    pub last_layer: usize,
    pub first_frame: usize,
    pub last_frame: usize,
}

impl CelRange {
    pub fn between(a: (usize, usize), b: (usize, usize)) -> Self {
        Self {
            first_layer: a.0.min(b.0),
            last_layer: a.0.max(b.0),
            first_frame: a.1.min(b.1),
            last_frame: a.1.max(b.1),
        }
    }

    pub fn contains(self, layer: usize, frame: usize) -> bool {
        (self.first_layer..=self.last_layer).contains(&layer)
            && (self.first_frame..=self.last_frame).contains(&frame)
    }

    fn fits(self, document: &Document) -> bool {
        self.first_layer <= self.last_layer
            && self.first_frame <= self.last_frame
            && self.last_layer < document.layers.len()
            && self.last_frame < document.durations.len()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SliceKey {
    pub frame: usize,
    pub bounds: [i32; 4],
    pub center: Option<[i32; 4]>,
    pub pivot: Option<[i32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteSlice {
    pub name: String,
    pub keys: Vec<SliceKey>,
}

impl SpriteSlice {
    pub fn key_at(&self, frame: usize) -> Option<&SliceKey> {
        self.keys.iter().rev().find(|key| key.frame <= frame)
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
impl MetadataChunk {
    pub(crate) fn is_tag_user_data(&self) -> bool {
        self.frame == 0 && self.kind == 0x2020 && self.owner == "tags"
    }

    fn is_frame_independent(&self) -> bool {
        self.frame == 0
            && (self.owner.starts_with("layer:")
                || self.owner.starts_with("slice:")
                || self.owner.starts_with("tileset:")
                || self.owner == "tags"
                || self.owner == "ase:header"
                || (self.owner == "palette" && self.kind == 0x2020)
                || (self.owner.is_empty() && self.kind == 0x2008))
    }

    fn cel_user_data_layer<'a>(&self, layers: &'a [Layer]) -> Option<&'a Layer> {
        if self.kind != 0x2020 {
            return None;
        }
        let layer = self.owner.strip_prefix("cel:")?.parse::<usize>().ok()?;
        layers.get(layer)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Adjustment {
    Palette { colors: Vec<(Pixel, Pixel)> },
    Levels { black: u8, white: u8 },
    HueSaturation { hue: i16, saturation: i16 },
}

impl Adjustment {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Palette { colors } if colors.len() > 256 => {
                Err("Use up to 256 palette mappings".into())
            }
            Self::Levels { black, white } if black >= white => {
                Err("Black level must be below white level".into())
            }
            Self::HueSaturation { hue, saturation }
                if !(-180..=180).contains(hue) || !(-100..=100).contains(saturation) =>
            {
                Err("Hue and saturation are outside their supported ranges".into())
            }
            _ => Ok(()),
        }
    }

    pub fn apply(&self, pixel: Pixel) -> Pixel {
        let mut result = pixel;
        match self {
            Self::Palette { colors } => {
                if let Some((_, target)) = colors.iter().find(|(source, _)| *source == pixel) {
                    result[..3].copy_from_slice(&target[..3]);
                }
            }
            Self::Levels { black, white } => {
                let span = u32::from(white.saturating_sub(*black).max(1));
                for channel in &mut result[..3] {
                    *channel = ((u32::from(channel.saturating_sub(*black)) * 255 + span / 2) / span)
                        .min(255) as u8;
                }
            }
            Self::HueSaturation { hue, saturation } => {
                let rgb = [pixel[0], pixel[1], pixel[2]].map(|c| f64::from(c) / 255.);
                let max = rgb[0].max(rgb[1]).max(rgb[2]);
                let min = rgb[0].min(rgb[1]).min(rgb[2]);
                let delta = max - min;
                if delta == 0. {
                    return result;
                }
                let sector = if max == rgb[0] {
                    (rgb[1] - rgb[2]) / delta
                } else if max == rgb[1] {
                    2. + (rgb[2] - rgb[0]) / delta
                } else {
                    4. + (rgb[0] - rgb[1]) / delta
                };
                let h = (sector + f64::from(*hue) / 60.).rem_euclid(6.);
                let chroma = (delta * (1. + f64::from(*saturation) / 100.)).min(max);
                let x = chroma * (1. - (h.rem_euclid(2.) - 1.).abs());
                let shifted = match h as u8 {
                    0 => [chroma, x, 0.],
                    1 => [x, chroma, 0.],
                    2 => [0., chroma, x],
                    3 => [0., x, chroma],
                    4 => [x, 0., chroma],
                    _ => [chroma, 0., x],
                };
                for (channel, value) in result[..3].iter_mut().zip(shifted) {
                    *channel = ((value + max - chroma) * 255.).round() as u8;
                }
            }
        }
        result
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    #[serde(skip)]
    pub editor_id: EditorId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    #[serde(deserialize_with = "decode_raster_slots")]
    pub cels: Vec<Pixels>,
    pub group: bool,
    pub parent: Option<usize>,
    pub blend: BlendMode,
    #[serde(deserialize_with = "decode_metadata_slots")]
    pub cel_meta: Vec<CelMeta>,
    pub tileset: Option<usize>,
    #[serde(deserialize_with = "decode_tile_slots")]
    pub tile_cels: Vec<Vec<u32>>,
    #[serde(default)]
    pub background: bool,
    #[serde(default)]
    pub ase_flags: u16,
    #[serde(default)]
    pub uuid: Vec<u8>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub link_duplicates: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exclude_from_onion: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Pixels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adjustment: Option<Adjustment>,
}
impl Layer {
    pub fn is_ase_reference(&self) -> bool {
        self.ase_flags & 64 != 0 && !self.group && self.tileset.is_none()
    }

    fn prune_cel_links(&mut self) {
        let mut counts = std::collections::HashMap::new();
        for link in self.cel_meta.iter().filter_map(|cel| cel.artwork_link) {
            *counts.entry(link).or_insert(0usize) += 1;
        }
        for cel in &mut self.cel_meta {
            if cel.artwork_link.is_some_and(|link| counts[&link] == 1) {
                cel.artwork_link = None;
            }
        }
    }

    pub fn new(name: String, w: usize, h: usize, frames: usize) -> Self {
        Self {
            editor_id: EditorId::default(),
            name,
            visible: true,
            locked: false,
            opacity: 255,
            cels: vec![Pixels::empty(w * h); frames],
            group: false,
            parent: None,
            blend: BlendMode::Normal,
            cel_meta: (0..frames).map(|_| CelMeta::default()).collect(),
            tileset: None,
            tile_cels: vec![vec![]; frames],
            background: false,
            ase_flags: 0,
            uuid: vec![],
            link_duplicates: false,
            exclude_from_onion: false,
            mask: None,
            adjustment: None,
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
// Older projects use their frame indices until a structural edit would shift them.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct FrameExportIds(Option<FrameExportIdState>);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FrameExportIdState {
    #[serde(deserialize_with = "decode_frame_export_ids")]
    frames: Vec<u32>,
    next: u64,
}

impl FrameExportIds {
    fn is_none(&self) -> bool {
        self.0.is_none()
    }

    fn nonsequential_ids(&self) -> Option<&[u32]> {
        self.0.as_ref().and_then(|state| {
            (!state
                .frames
                .iter()
                .copied()
                .eq(0..state.frames.len() as u32))
            .then_some(state.frames.as_slice())
        })
    }
}

impl PartialEq for FrameExportIds {
    fn eq(&self, other: &Self) -> bool {
        // An allocator advance alone does not change which sprites the document contains.
        self.nonsequential_ids() == other.nonsequential_ids()
    }
}

fn decode_frame_export_ids<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Vec<u32>, D::Error> {
    struct FrameIds;
    impl<'de> serde::de::Visitor<'de> for FrameIds {
        type Value = Vec<u32>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an array of frame export IDs")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<u32>, A::Error> {
            let mut ids = Vec::new();
            while let Some(id) = seq.next_element()? {
                if ids.len() == u16::MAX as usize {
                    return Err(serde::de::Error::custom("Too many frame export IDs"));
                }
                charge_project_bytes(std::mem::size_of::<u32>())?;
                ids.push(id);
            }
            Ok(ids)
        }
    }
    decoder.deserialize_seq(FrameIds)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(skip)]
    pub editor_id: EditorId,
    #[serde(default = "default_compose_groups")]
    pub compose_groups: bool,
    pub width: usize,
    pub height: usize,
    pub layers: Vec<Layer>,
    pub durations: Vec<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) frame_names: Vec<String>,
    #[serde(default, skip_serializing_if = "FrameExportIds::is_none")]
    pub(crate) frame_export_ids: FrameExportIds,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slices: Vec<SpriteSlice>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing_grid: Option<DrawingGrid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixel_aspect: Option<[u8; 2]>,
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
            editor_id: EditorId::default(),
            compose_groups: true,
            width: w,
            height: h,
            layers: vec![Layer::new("Layer 1".into(), w, h, 1)],
            durations: vec![DEFAULT_DURATION],
            frame_names: vec![],
            frame_export_ids: FrameExportIds::default(),
            color_mode: ColorMode::Rgba,
            palette: default_palette(),
            palette_names: vec![],
            frame_palettes: vec![],
            transparent_index: 0,
            drawing_index: None,
            tags: vec![],
            slices: vec![],
            tilesets: vec![],
            ase_tilesets: vec![],
            metadata: vec![],
            color_profile: vec![],
            brush_shape: BrushShape::Round,
            dithering: None,
            wraparound: false,
            palette_snap: false,
            drawing_grid: None,
            pixel_aspect: None,
            references: vec![],
        }
    }
    pub fn drawing_grid(&self) -> DrawingGrid {
        self.stored_drawing_grid().unwrap_or_default()
    }

    pub(crate) fn stored_drawing_grid(&self) -> Option<DrawingGrid> {
        self.drawing_grid
            .or_else(|| DrawingGrid::from_ase_header(self.ase_header()?))
    }

    fn ase_header(&self) -> Option<&[u8]> {
        self.metadata
            .iter()
            .find(|chunk| chunk.owner == "ase:header")
            .map(|chunk| chunk.data.as_slice())
    }

    pub(crate) fn pixel_aspect_from_ase_header(header: &[u8]) -> Option<[u8; 2]> {
        let ratio: [u8; 2] = header.get(34..36)?.try_into().ok()?;
        (!ratio.contains(&0) && ratio != [1, 1]).then_some(ratio)
    }

    pub fn pixel_aspect(&self) -> [u8; 2] {
        self.pixel_aspect
            .or_else(|| Self::pixel_aspect_from_ase_header(self.ase_header()?))
            .unwrap_or([1, 1])
    }

    pub fn frame_name(&self, frame: usize) -> Option<&str> {
        self.frame_names
            .get(frame)
            .filter(|name| !name.is_empty())
            .map(String::as_str)
    }

    pub fn has_frame_names(&self) -> bool {
        self.frame_names.iter().any(|name| !name.is_empty())
    }

    /// A blank name restores the default numbered label.
    pub fn set_frame_name(&mut self, frame: usize, name: &str) -> Result<(), String> {
        if frame >= self.durations.len() {
            return Err("Invalid frame".into());
        }
        let name = name.trim();
        if name.chars().count() > MAX_FRAME_NAME_CHARS || name.chars().any(char::is_control) {
            return Err(format!(
                "Use a single-line frame name of up to {MAX_FRAME_NAME_CHARS} characters"
            ));
        }
        if !name.is_empty() || frame < self.frame_names.len() {
            if frame >= self.frame_names.len() {
                self.frame_names.resize_with(frame + 1, String::new);
            }
            self.frame_names[frame] = name.to_owned();
            self.prune_frame_names();
        }
        Ok(())
    }

    pub(crate) fn prune_frame_names(&mut self) {
        while self.frame_names.last().is_some_and(String::is_empty) {
            self.frame_names.pop();
        }
    }

    pub fn frame_export_id(&self, frame: usize) -> u32 {
        self.frame_export_ids
            .0
            .as_ref()
            .map_or(frame as u32, |ids| ids.frames[frame])
    }

    pub(crate) fn next_frame_export_id(&self) -> u64 {
        self.frame_export_ids
            .0
            .as_ref()
            .map_or(self.durations.len() as u64, |ids| ids.next)
    }

    /// A persistent allocation reservation can change without changing the artwork.
    pub fn differs_from_saved(&self, saved: &Self) -> bool {
        self != saved || self.next_frame_export_id() != saved.next_frame_export_id()
    }

    pub(crate) fn set_sheet_frame_export_ids(&mut self, frames: Vec<u32>, next: u64) {
        self.frame_export_ids = FrameExportIds(Some(FrameExportIdState { frames, next }));
    }

    /// History restores the frames but keeps IDs allocated by abandoned edits reserved.
    pub fn retain_frame_export_id_counter(&mut self, previous: &Self) {
        if self.editor_id.0 == previous.editor_id.0
            && previous.next_frame_export_id() > self.next_frame_export_id()
        {
            self.explicit_frame_export_ids().next = previous.next_frame_export_id();
        }
    }

    fn explicit_frame_export_ids(&mut self) -> &mut FrameExportIdState {
        self.frame_export_ids
            .0
            .get_or_insert_with(|| FrameExportIdState {
                frames: (0..self.durations.len() as u32).collect(),
                next: self.durations.len() as u64,
            })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.pixel_aspect.is_some_and(|ratio| ratio.contains(&0)) {
            return Err("Invalid pixel aspect ratio".into());
        }
        if self.drawing_grid.is_some_and(|grid| !grid.valid()) {
            return Err("Invalid drawing grid".into());
        }
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
            || self.width.saturating_mul(self.height) > MAX_PIXELS
            || self.durations.is_empty()
            || self.layers.is_empty()
            || self.durations.len() > 65535
            || self.layers.len() > 65535
            || self.layers.len().saturating_mul(self.durations.len()) > MAX_CEL_SLOTS
            || self.stored_pixels() > MAX_DOCUMENT_PIXELS
        {
            return Err("Project dimensions or stored pixel count exceed limits".into());
        }
        if self.frame_names.len() > self.durations.len()
            || self.frame_names.iter().any(|name| {
                name.chars().count() > MAX_FRAME_NAME_CHARS || name.chars().any(char::is_control)
            })
        {
            return Err("Invalid frame names".into());
        }
        if let Some(ids) = &self.frame_export_ids.0 {
            let mut seen = std::collections::HashSet::with_capacity(ids.frames.len());
            if ids.frames.len() != self.durations.len()
                || ids.next > u64::from(u32::MAX) + 1
                || ids
                    .frames
                    .iter()
                    .any(|&id| u64::from(id) >= ids.next || !seen.insert(id))
            {
                return Err("Invalid frame export identities".into());
            }
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
        let group_buffers: Vec<_> = self
            .layers
            .iter()
            .map(|layer| {
                layer.group
                    && (self.compose_groups
                        || layer.mask.is_some()
                        || layer.cel_meta.iter().any(|cel| {
                            cel.keyframe
                                .is_some_and(|key| key != CelKeyframe::default())
                        }))
            })
            .collect();
        for (i, l) in self.layers.iter().enumerate() {
            if l.mask
                .as_ref()
                .is_some_and(|mask| mask.len() != self.width * self.height)
            {
                return Err("Layer mask dimensions must match the canvas".into());
            }
            if let Some(adjustment) = &l.adjustment {
                if l.group || l.tileset.is_some() {
                    return Err("Adjustments require their own image layer".into());
                }
                adjustment.validate()?;
            }
            for m in &l.cel_meta {
                if m.keyframe.is_some_and(|key| {
                    key.x.unsigned_abs() > MAX_DIMENSION as u32
                        || key.y.unsigned_abs() > MAX_DIMENSION as u32
                }) {
                    return Err("Position keyframe exceeds the canvas limits".into());
                }
                if m.outside.iter().any(|pixel| {
                    !(-131_072..=131_072).contains(&pixel.x)
                        || !(-131_072..=131_072).contains(&pixel.y)
                }) {
                    return Err("Off-canvas pixels exceed supported coordinates".into());
                }
                if m.artwork_link == Some(usize::MAX) {
                    return Err("Invalid cel link".into());
                }
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
            // Keep scratch allocations bounded independently of sparse backing.
            let mut buffers = 1 + usize::from(group_buffers[i] || l.tileset.is_some());
            let mut p = l.parent;
            let mut depth = 0;
            while let Some(j) = p {
                if j >= self.layers.len() || j == i || !self.layers[j].group || depth >= 128 {
                    return Err("Invalid layer group hierarchy".into());
                }
                buffers += usize::from(group_buffers[j]);
                p = self.layers[j].parent;
                depth += 1;
            }
            if self
                .width
                .saturating_mul(self.height)
                .saturating_mul(buffers)
                > MAX_DOCUMENT_PIXELS
            {
                return Err("Layer hierarchy exceeds the compositing workspace budget".into());
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
        for slice in &self.slices {
            if slice.name.len() > u16::MAX as usize
                || slice
                    .keys
                    .windows(2)
                    .any(|pair| pair[0].frame >= pair[1].frame)
                || slice.keys.iter().any(|key| {
                    key.frame >= self.durations.len()
                        || key.bounds[2] < 0
                        || key.bounds[3] < 0
                        || key.center.is_some_and(|rect| rect[2] < 0 || rect[3] < 0)
                })
            {
                return Err("Invalid slice bounds or animation keys".into());
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
        let mut seen = std::collections::HashSet::new();
        let raster_pixels = self
            .layers
            .iter()
            .flat_map(|layer| layer.cels.iter().chain(layer.mask.iter()))
            .chain(self.tilesets.iter().flat_map(|tileset| &tileset.tiles))
            .chain(self.references.iter().map(|reference| &reference.pixels))
            .filter(|pixels| pixels.backing_id().is_some_and(|id| seen.insert(id)))
            .fold(0usize, |total, pixels| {
                total.saturating_add(pixels.allocation_pixels())
            });
        let mut seen_indices = std::collections::HashSet::new();
        self.layers.iter().fold(raster_pixels, |total, layer| {
            let placement_pixels = layer.tile_cels.iter().map(Vec::len).sum::<usize>();
            layer
                .cel_meta
                .iter()
                .fold(total.saturating_add(placement_pixels), |total, cel| {
                    let index_bytes = cel
                        .indices
                        .as_ref()
                        .filter(|indices| seen_indices.insert(Arc::as_ptr(indices)))
                        .map_or(0, |indices| indices.len());
                    total
                        .saturating_add(index_bytes.div_ceil(4))
                        .saturating_add(cel.outside.len().saturating_mul(3))
                })
        })
    }

    pub fn same_editor_layer_targets(&self, other: &Self) -> bool {
        self.layers
            .iter()
            .map(|layer| layer.editor_id.0)
            .eq(other.layers.iter().map(|layer| layer.editor_id.0))
    }

    pub fn same_editor_targets(&self, other: &Self) -> bool {
        self.editor_id.0 == other.editor_id.0
            && self.layers.len() == other.layers.len()
            && self.layers.iter().zip(&other.layers).all(|(a, b)| {
                a.editor_id.0 == b.editor_id.0
                    && a.cel_meta.len() == b.cel_meta.len()
                    && a.cel_meta
                        .iter()
                        .zip(&b.cel_meta)
                        .all(|(a, b)| a.editor_id.0 == b.editor_id.0)
            })
    }

    pub fn logical_pixels(&self) -> usize {
        self.width
            .saturating_mul(self.height)
            .saturating_mul(self.layers.len())
            .saturating_mul(self.durations.len())
    }

    pub(crate) fn share_linked_rasters(&mut self) {
        for layer in &mut self.layers {
            let mut shared = std::collections::HashMap::new();
            for (meta, raster) in layer.cel_meta.iter().zip(&mut layer.cels) {
                let Some(link) = meta.artwork_link else {
                    continue;
                };
                let origin = meta.bounds.map_or((0, 0), |b| (b[0], b[1]));
                let candidate = shared
                    .entry((link, origin))
                    .or_insert_with(|| raster.clone());
                if candidate == raster {
                    *raster = candidate.clone();
                }
            }
        }
    }

    pub fn can_grow(&self, pixels: usize) -> bool {
        self.stored_pixels().saturating_add(pixels) <= MAX_DOCUMENT_PIXELS
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
        if !self.layers[l].background
            && let Some(c) = p.get_mut(self.transparent_index)
        {
            c[3] = 0;
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
                if !l.background
                    && let Some(p) = effective.get_mut(self.transparent_index)
                {
                    p[3] = 0;
                }
                let p = &effective;
                let old = l.cel_meta[f].indices.clone();
                if matches!(c, Pixels::Empty { .. }) && old.is_none() && !l.background {
                    continue;
                }
                let mut indices = if mode == ColorMode::Indexed {
                    Vec::with_capacity(c.len())
                } else {
                    Vec::new()
                };
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
    /// Move a swatch, remapping indices rather than recoloring existing artwork.
    pub fn reorder_palette(&mut self, from: usize, to: usize) -> Result<(), String> {
        let n = self.palette.len();
        if from >= n || to >= n {
            return Err("Invalid palette position".into());
        }
        if self
            .frame_palettes
            .iter()
            .any(|p| !p.is_empty() && p.len() != n)
        {
            return Err("Frame palettes must have matching sizes to reorder".into());
        }
        let mut order: Vec<usize> = (0..n).collect();
        let item = order.remove(from);
        order.insert(to, item);
        let mut remap = vec![0; n];
        for (new, old) in order.iter().enumerate() {
            remap[*old] = new;
        }
        let reorder = |p: &mut Vec<Pixel>| {
            if !p.is_empty() {
                let old = p.clone();
                *p = order.iter().map(|i| old[*i]).collect();
            }
        };
        reorder(&mut self.palette);
        for p in &mut self.frame_palettes {
            reorder(p);
        }
        self.palette_names.resize(n, String::new());
        let name = self.palette_names.remove(from);
        self.palette_names.insert(to, name);
        self.transparent_index = remap[self.transparent_index.min(n - 1)];
        self.drawing_index = self.drawing_index.map(|i| remap[i.min(n - 1)]);
        for l in &mut self.layers {
            for m in &mut l.cel_meta {
                if let Some(indices) = &mut m.indices {
                    for i in Arc::make_mut(indices) {
                        *i = remap.get(*i as usize).copied().unwrap_or(0) as u8;
                    }
                }
            }
        }
        Ok(())
    }
    /// Append intermediate shades without changing any existing palette slots.
    pub fn palette_ramp(&mut self, from: usize, to: usize, steps: usize) -> Result<(), String> {
        if from >= self.palette.len()
            || to >= self.palette.len()
            || !(3..=256).contains(&steps)
            || self.palette.len() + steps - 2 > 256
        {
            return Err("Ramp needs valid endpoints and room in the 256-color palette".into());
        }
        if self
            .frame_palettes
            .iter()
            .any(|p| p.len() != self.palette.len())
        {
            return Err("Frame palettes must have matching sizes to add a shared ramp".into());
        }
        let colors = crate::workflows::ramp(self.palette[from], self.palette[to], steps);
        self.palette.extend_from_slice(&colors[1..steps - 1]);
        for p in &mut self.frame_palettes {
            if !p.is_empty() {
                let ramp = crate::workflows::ramp(p[from], p[to], steps);
                p.extend_from_slice(&ramp[1..steps - 1]);
            }
        }
        self.palette_names.resize(self.palette.len(), String::new());
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
                    if !l.background
                        && let Some(p) = effective.get_mut(self.transparent_index)
                    {
                        p[3] = 0;
                    }
                    let p = &effective;
                    let indices = l.cel_meta[f].indices.clone();
                    let slot_at = |i: usize, v: Pixel| {
                        indices
                            .as_ref()
                            .and_then(|a| a.get(i))
                            .copied()
                            .filter(|n| p.get(*n as usize) == Some(&v))
                            .map(|n| n as usize)
                            .unwrap_or_else(|| {
                                self.dithering.as_ref().map_or_else(
                                    || nearest(v, p),
                                    |m| m.index(v, p, i % self.width, i / self.width),
                                )
                            })
                    };
                    let mut replacement = color;
                    if index == self.transparent_index && !l.background {
                        replacement[3] = 0;
                    }
                    if l.background || matches!(c, Pixels::Dense(_)) {
                        for (i, v) in c.iter_mut().enumerate() {
                            if slot_at(i, *v) == index {
                                *v = replacement;
                            }
                        }
                    } else {
                        let before = c.clone();
                        before.visit_nonclear(|i, pixel| {
                            if slot_at(i, pixel) == index {
                                c.set(i, replacement);
                            }
                        });
                        // Unstored pixels are transparent. A stored nontransparent slot
                        // can deliberately point at an RGBA value that is currently clear.
                        if index != self.transparent_index
                            && p.get(index) == Some(&CLEAR)
                            && let Some(indices) = &indices
                        {
                            for (i, &slot) in indices.iter().enumerate() {
                                if slot as usize == index && before[i] == CLEAR {
                                    c.set(i, replacement);
                                }
                            }
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
    pub fn link_cel(&mut self, layer: usize, frame: usize, source: usize) -> Result<(), String> {
        if !self.editable(layer)
            || frame == source
            || frame >= self.durations.len()
            || source >= self.durations.len()
        {
            return Err("Choose two different cels in an unlocked layer".into());
        }
        let palette =
            (self.color_mode == ColorMode::Indexed).then(|| self.editing_palette(layer, frame));
        self.set_cel_link(layer, frame, source);
        let layer = &mut self.layers[layer];
        let target = &layer.cel_meta[frame];
        let source_meta = &layer.cel_meta[source];
        let origin = |meta: &CelMeta| {
            meta.tilemap
                .as_ref()
                .map(|g| (g.x, g.y))
                .or_else(|| meta.bounds.map(|b| (b[0], b[1])))
                .unwrap_or((0, 0))
        };
        let (sx, sy) = origin(source_meta);
        let (tx, ty) = origin(target);
        let mut meta = source_meta.clone();
        meta.editor_id = target.editor_id;
        meta.opacity = target.opacity;
        meta.z_index = target.z_index;
        meta.keyframe = target.keyframe;
        let source_pixels = layer.cels[source].clone();
        let mut raster = source_pixels.clone();
        if let Some(tileset) = layer.tileset {
            let set = &self.tilesets[tileset];
            if meta.tilemap.is_some() || (tx, ty) != (0, 0) {
                let mut geometry = meta.tilemap.take().unwrap_or_else(|| TilemapGeometry {
                    x: 0,
                    y: 0,
                    columns: self.width.div_ceil(set.width),
                    rows: self.height.div_ceil(set.height),
                });
                geometry.x = tx;
                geometry.y = ty;
                meta.tilemap = Some(geometry);
            }
        } else if (sx, sy) != (tx, ty) || palette.is_some() {
            raster = Pixels::empty(self.width * self.height);
            let mut outside = Vec::new();
            let mut indexes = meta
                .indices
                .as_ref()
                .map(|_| vec![self.transparent_index as u8; raster.len()]);
            let mut place = |x: i32, y: i32, pixel: Pixel, palette_index: Option<u8>| {
                let color = palette_index
                    .and_then(|slot| palette.as_ref()?.get(slot as usize).copied())
                    .unwrap_or(pixel);
                if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
                    let index = y as usize * self.width + x as usize;
                    raster.set(index, color);
                    if let (Some(indexes), Some(slot)) = (&mut indexes, palette_index) {
                        indexes[index] = slot;
                    }
                } else if color != CLEAR {
                    outside.push(OutsidePixel { x, y, color });
                }
            };
            if let Some(indices) = &meta.indices {
                for (i, &slot) in indices.iter().enumerate() {
                    place(
                        (i % self.width) as i32 + tx - sx,
                        (i / self.width) as i32 + ty - sy,
                        source_pixels[i],
                        Some(slot),
                    );
                }
            } else {
                source_pixels.visit_nonclear(|i, pixel| {
                    place(
                        (i % self.width) as i32 + tx - sx,
                        (i / self.width) as i32 + ty - sy,
                        pixel,
                        None,
                    );
                });
            }
            for pixel in &meta.outside {
                place(pixel.x + tx - sx, pixel.y + ty - sy, pixel.color, None);
            }
            meta.outside = outside;
            meta.indices = indexes.map(Arc::new);
            if (sx, sy) != (tx, ty) || meta.bounds.is_some() {
                let bounds = meta
                    .bounds
                    .unwrap_or([0, 0, self.width as i32, self.height as i32]);
                meta.bounds = Some([tx, ty, bounds[2], bounds[3]]);
            }
        }
        layer.cels[frame] = raster;
        layer.tile_cels[frame] = layer.tile_cels[source].clone();
        layer.cel_meta[frame] = meta;
        layer.prune_cel_links();
        Ok(())
    }

    pub(crate) fn set_cel_link(&mut self, layer: usize, frame: usize, source: usize) {
        let meta = &mut self.layers[layer].cel_meta;
        let id = meta[source].artwork_link.unwrap_or_else(|| {
            meta.iter()
                .filter_map(|cel| cel.artwork_link)
                .max()
                .unwrap_or(0)
                + 1
        });
        meta[source].artwork_link = Some(id);
        meta[frame].artwork_link = Some(id);
    }

    pub fn unlink_cel(&mut self, layer: usize, frame: usize) -> Result<(), String> {
        if !self.editable(layer) || frame >= self.durations.len() {
            return Err("Choose an unlocked cel".into());
        }
        self.layers[layer].cel_meta[frame].artwork_link = None;
        self.layers[layer].prune_cel_links();
        Ok(())
    }

    pub(crate) fn draw_pixel(&mut self, l: usize, f: usize, i: usize, color: Pixel, index: u8) {
        let indexed = self.color_mode == ColorMode::Indexed;
        let creates_index = indexed && index as usize != self.transparent_index;
        let initial_indices = || vec![self.transparent_index as u8; self.width * self.height];
        let has_link = self.layers[l].cel_meta[f].artwork_link.is_some();
        if !has_link {
            let layer = &mut self.layers[l];
            layer.cels[f].set(i, color);
            if creates_index && layer.cel_meta[f].indices.is_none() {
                layer.cel_meta[f].indices = Some(Arc::new(initial_indices()));
            }
            if let Some(indices) = &mut layer.cel_meta[f].indices {
                Arc::make_mut(indices)[i] = index;
            }
            return;
        }
        let origin = |meta: &CelMeta| meta.bounds.map_or((0, 0), |b| (b[0], b[1]));
        let source_meta = &self.layers[l].cel_meta[f];
        let (sx, sy) = origin(source_meta);
        let link = source_meta.artwork_link;
        let x = (i % self.width) as i32 - sx;
        let y = (i / self.width) as i32 - sy;
        let min_x = x.min(0);
        let min_y = y.min(0);
        let same_grid = self.color_mode != ColorMode::Indexed
            && self.layers[l]
                .cel_meta
                .iter()
                .filter(|meta| meta.artwork_link == link)
                .all(|meta| origin(meta) == (sx, sy));
        let shared = if same_grid {
            let layer = &mut self.layers[l];
            if layer.cels[f][i] != color {
                let backing = layer.cels[f].backing_id();
                if backing.is_some() {
                    // Release the linked placements before the write, so only
                    // snapshots or independent cels trigger copy-on-write.
                    for (frame, (pixels, meta)) in
                        layer.cels.iter_mut().zip(&layer.cel_meta).enumerate()
                    {
                        if frame != f && meta.artwork_link == link && pixels.backing_id() == backing
                        {
                            *pixels = Pixels::empty(pixels.len());
                        }
                    }
                }
                layer.cels[f].set(i, color);
            }
            Some(layer.cels[f].clone())
        } else {
            None
        };
        let mut new_indices = std::collections::HashMap::new();
        for target in 0..self.durations.len() {
            let meta = &self.layers[l].cel_meta[target];
            if meta.artwork_link != link {
                continue;
            }
            let (tx, ty) = origin(meta);
            let (dest_x, dest_y) = (tx + x, ty + y);
            let color = if indexed {
                self.editing_palette(l, target)
                    .get(index as usize)
                    .copied()
                    .unwrap_or(color)
            } else {
                color
            };
            let pos = self.index(dest_x, dest_y);
            let layer = &mut self.layers[l];
            let meta = &mut layer.cel_meta[target];
            if let Some(bounds) = &mut meta.bounds {
                bounds[0] += min_x;
                bounds[1] += min_y;
                bounds[2] = bounds[2].max(x + 1) - min_x;
                bounds[3] = bounds[3].max(y + 1) - min_y;
            }
            if let Some(pixel) = pos {
                if let Some(pixels) = &shared {
                    layer.cels[target] = pixels.clone();
                } else {
                    layer.cels[target].set(pixel, color);
                }
                if let Some(indices) = &mut meta.indices {
                    if indices[pixel] != index {
                        Arc::make_mut(indices)[pixel] = index;
                    }
                } else if creates_index {
                    let indices = new_indices.entry(pixel).or_insert_with(|| {
                        let mut indices = initial_indices();
                        indices[pixel] = index;
                        Arc::new(indices)
                    });
                    meta.indices = Some(indices.clone());
                }
            } else if let Some(position) = meta
                .outside
                .iter()
                .position(|pixel| pixel.x == dest_x && pixel.y == dest_y)
            {
                if color == CLEAR {
                    meta.outside.swap_remove(position);
                } else {
                    meta.outside[position].color = color;
                }
            } else if color != CLEAR {
                meta.outside.push(OutsidePixel {
                    x: dest_x,
                    y: dest_y,
                    color,
                });
            }
        }
    }

    pub fn restore_pixel(
        &mut self,
        layer: usize,
        frame: usize,
        index: usize,
        pixel: Pixel,
        palette_slot: Option<u8>,
    ) {
        let palette_slot = palette_slot.unwrap_or_else(|| {
            if self.color_mode != ColorMode::Indexed {
                0
            } else if pixel[3] == 0 && !self.layers[layer].background {
                self.transparent_index as u8
            } else {
                nearest(pixel, &self.editing_palette(layer, frame)) as u8
            }
        });
        self.draw_pixel(layer, frame, index, pixel, palette_slot);
    }

    pub fn set_pixel(&mut self, l: usize, f: usize, i: usize, color: Pixel) {
        if !self.editable(l) || self.layers[l].tileset.is_some() {
            return;
        }
        let (color, n) = self.drawing_color(l, f, color);
        self.draw_pixel(l, f, i, color, n);
    }
    /// Estimate pixels inspected and backing allocated by a scoped color replacement.
    pub fn color_replacement_cost(
        &self,
        layer: usize,
        frame: usize,
        source: Pixel,
        replacement: Pixel,
        area: usize,
    ) -> (usize, usize) {
        let layer_data = &self.layers[layer];
        let cel = &layer_data.cels[frame];
        let (color, slot) = self.drawing_color(layer, frame, replacement);
        if area == 0 || color == source {
            return (1, 0);
        }
        let meta = &layer_data.cel_meta[frame];
        let existing = cel.allocation_pixels();
        let (scan, raster) = if source == CLEAR {
            // Each changed pixel could require a separate sparse block.
            let raster = (existing + area * (PIXEL_BLOCK + 8)).min(cel.len());
            (area, raster)
        } else {
            (area.min(existing), existing)
        };
        if scan == 0 {
            return (1, 0);
        }
        let indices = if self.color_mode == ColorMode::Indexed
            && (meta.indices.is_some() || slot as usize != self.transparent_index)
        {
            cel.len().div_ceil(std::mem::size_of::<Pixel>())
        } else {
            0
        };
        let link_work = if meta.artwork_link.is_some() {
            self.durations.len()
        } else {
            0
        };
        (scan + link_work, raster + indices)
    }

    /// Replace an exact RGBA match in one editable raster cel.
    /// The region contains the left, top, right, and bottom edges, with exclusive final edges.
    pub fn replace_color_in_cel(
        &mut self,
        layer: usize,
        frame: usize,
        source: Pixel,
        replacement: Pixel,
        region: [usize; 4],
        selected: impl Fn(usize) -> bool,
    ) -> usize {
        let [left, top, right, bottom] = region;
        let width = self.width;
        if source == replacement
            || frame >= self.durations.len()
            || !self.editable(layer)
            || self.layers[layer].tileset.is_some()
            || left >= right
            || top >= bottom
            || right > width
            || bottom > self.height
        {
            return 0;
        }
        let (color, slot) = self.drawing_color(layer, frame, replacement);
        if color == source {
            return 0;
        }
        let cel = self.layers[layer].cels[frame].clone();
        let mut changed = 0;
        let mut replace = |i, pixel| {
            if pixel == source
                && (left..right).contains(&(i % width))
                && (top..bottom).contains(&(i / width))
                && selected(i)
            {
                if changed == 0 && self.layers[layer].cel_meta[frame].artwork_link.is_some() {
                    // An edit with explicit frame or selection limits must not propagate to linked cels.
                    self.layers[layer].cel_meta[frame].artwork_link = None;
                    self.layers[layer].prune_cel_links();
                }
                self.draw_pixel(layer, frame, i, color, slot);
                changed += 1;
            }
        };
        let area = (right - left) * (bottom - top);
        if source == CLEAR || area < cel.allocation_pixels() {
            for y in top..bottom {
                for i in y * width + left..y * width + right {
                    replace(i, cel[i]);
                }
            }
        } else {
            cel.visit_nonclear(replace);
        }
        changed
    }

    pub fn editable(&self, l: usize) -> bool {
        self.layer_unlocked(l) && !self.layers[l].group
    }

    fn layer_unlocked(&self, l: usize) -> bool {
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
        true
    }
    pub fn stamp(&mut self, l: usize, f: usize, x: i32, y: i32, c: Pixel, size: i32) {
        self.stamp_on_axes((l, f), (x, y), c, size, self.wraparound.into());
    }

    fn stamp_on_axes(
        &mut self,
        (l, f): (usize, usize),
        (x, y): (i32, i32),
        c: Pixel,
        size: i32,
        axes: WrapAxes,
    ) {
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
                if accept && let Some(i) = self.position_on_axes(x + dx, y + dy, axes) {
                    self.draw_pixel(l, f, i, c, ci);
                }
            }
        }
    }
    pub fn line(&mut self, l: usize, f: usize, a: (i32, i32), b: (i32, i32), c: Pixel, size: i32) {
        self.line_on_axes((l, f), a, b, c, size, self.wraparound.into());
    }

    pub fn line_on_axes(
        &mut self,
        cel: (usize, usize),
        a: (i32, i32),
        b: (i32, i32),
        c: Pixel,
        size: i32,
        axes: WrapAxes,
    ) {
        let _: Result<(), std::convert::Infallible> = line_pixels(a, b, |(x, y)| {
            self.stamp_on_axes(cel, (x, y), c, size, axes);
            Ok(())
        });
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "Preserve the established public API"
    )]
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
        self.ellipse_on_axes((l, f), (a, b), c, filled, size, self.wraparound.into());
    }

    pub fn ellipse_on_axes(
        &mut self,
        cel: (usize, usize),
        (a, b): ((i32, i32), (i32, i32)),
        c: Pixel,
        filled: bool,
        size: i32,
        axes: WrapAxes,
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
                self.line_on_axes(
                    cel,
                    (lx as i32, y as i32),
                    (rx as i32, y as i32),
                    c,
                    1,
                    axes,
                )
            } else {
                self.stamp_on_axes(cel, (lx as i32, y as i32), c, size, axes);
                self.stamp_on_axes(cel, (rx as i32, y as i32), c, size, axes);
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
        self.position_on_axes(x, y, self.wraparound.into())
    }

    pub fn position_on_axes(&self, x: i32, y: i32, axes: WrapAxes) -> Option<usize> {
        let (x, y) = axes.position((self.width, self.height), (x, y))?;
        Some(y as usize * self.width + x as usize)
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
        self.draw_pixel(l, f, i, c, ci);
        while let Some((x, y)) = stack.pop() {
            for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if let Some(n) = self.drawing_position(nx, ny)
                    && self.layers[l].cels[f][n] == old
                {
                    self.draw_pixel(l, f, n, c, ci);
                    stack.push(((n % self.width) as i32, (n / self.width) as i32));
                }
            }
        }
    }
    /// Read one authored layer pixel without allocating a full layer raster.
    pub fn sample_layer(&self, layer: usize, frame: usize, x: i32, y: i32) -> Option<Pixel> {
        let index = self.index(x, y)?;
        let layer_data = self.layers.get(layer)?;
        if let Some(set) = layer_data
            .tileset
            .and_then(|index| self.tilesets.get(index))
        {
            let Some((geometry, col, row)) = self.tile_at_point(layer, frame, (x, y)) else {
                return Some(CLEAR);
            };
            let placement = layer_data.tile_cels[frame][row * geometry.columns + col];
            let px = (i64::from(x) - i64::from(geometry.x)) as usize % set.width;
            let py = (i64::from(y) - i64::from(geometry.y)) as usize % set.height;
            return Some(set.placement_pixel(placement, px, py));
        }
        Some(layer_data.cels.get(frame)?[index])
    }

    pub fn layer_pixels(&self, l: usize, f: usize) -> Vec<Pixel> {
        if self.layers[l].tileset.is_none() {
            return self.layers[l].cels[f].to_vec();
        }
        let mut out = vec![CLEAR; self.width * self.height];
        self.visit_tile_pixels(l, f, |x, y, color| {
            if let Some(index) = self.index(x, y) {
                out[index] = color;
            }
        });
        out
    }

    fn visit_tile_pixels(&self, l: usize, f: usize, mut visit: impl FnMut(i32, i32, Pixel)) {
        let layer = &self.layers[l];
        let Some(t) = layer.tileset.and_then(|index| self.tilesets.get(index)) else {
            return;
        };
        let geometry = layer.cel_meta[f].tilemap.as_ref();
        let cols = geometry.map_or(self.width.div_ceil(t.width), |g| g.columns);
        let (ox, oy) = geometry.map_or((0, 0), |g| (g.x, g.y));
        for (i, &id) in layer.tile_cels[f].iter().enumerate() {
            let n = (id & 0x1fff_ffff) as usize;
            if n == 0 {
                continue;
            }
            let Some(tile) = t.tiles.get(n - 1) else {
                continue;
            };
            for y in 0..t.height {
                for x in 0..t.width {
                    let Some(offset) = t.source_offset(id, x, y) else {
                        continue;
                    };
                    visit(
                        ox + (i % cols * t.width + x) as i32,
                        oy + (i / cols * t.height + y) as i32,
                        tile[offset],
                    );
                }
            }
        }
    }
    /// Compare only the inputs used to composite this frame. Shared raster
    /// buffers and layer metadata make the common unchanged check cheap.
    pub fn set_cel_keyframe(
        &mut self,
        layer: usize,
        frame: usize,
        key: Option<CelKeyframe>,
    ) -> Result<(), String> {
        if !self.layer_unlocked(layer) || frame >= self.durations.len() {
            return Err("Choose an unlocked layer and valid frame".into());
        }
        if key.is_some_and(|key| {
            key.x.unsigned_abs() > MAX_DIMENSION as u32
                || key.y.unsigned_abs() > MAX_DIMENSION as u32
        }) {
            return Err("Position keyframe exceeds the canvas limits".into());
        }
        self.layers[layer].cel_meta[frame].keyframe = key;
        Ok(())
    }

    pub fn sampled_cel_key(&self, layer: usize, frame: usize) -> CelKeyframe {
        let cels = &self.layers[layer].cel_meta;
        let read_key = |(index, cel): (usize, &CelMeta)| cel.keyframe.map(|key| (index, key));
        let previous = cels[..=frame].iter().enumerate().rev().find_map(read_key);
        let Some((left, key)) =
            previous.or_else(|| cels.iter().enumerate().rev().find_map(read_key))
        else {
            return CelKeyframe::default();
        };
        if left == frame || key.hold {
            return key;
        }
        let (right, next) = cels
            .iter()
            .enumerate()
            .skip(frame + 1)
            .find_map(read_key)
            .or_else(|| cels[..=left].iter().enumerate().find_map(read_key))
            .unwrap();
        if left == right {
            return key;
        }
        let duration = |start: usize, end: usize| -> u64 {
            let sum = |frames: &[u16]| frames.iter().map(|&value| u64::from(value)).sum::<u64>();
            if start <= end {
                sum(&self.durations[start..end])
            } else {
                sum(&self.durations[start..]) + sum(&self.durations[..end])
            }
        };
        let offset = duration(left, frame);
        let span = duration(left, right);
        let t = offset as f64 / span.max(1) as f64;
        let sample = |a: i32, b: i32| (f64::from(a) + f64::from(b - a) * t).round() as i32;
        CelKeyframe {
            x: sample(key.x, next.x),
            y: sample(key.y, next.y),
            opacity: sample(i32::from(key.opacity), i32::from(next.opacity)) as u8,
            hold: false,
        }
    }

    pub fn set_layer_mask(&mut self, layer: usize, mask: Option<Pixels>) -> Result<(), String> {
        if !self.layer_unlocked(layer)
            || mask
                .as_ref()
                .is_some_and(|p| p.len() != self.width * self.height)
        {
            return Err("Choose an unlocked layer and canvas-sized mask".into());
        }
        self.layers[layer].mask = mask;
        Ok(())
    }

    pub fn set_adjustment(
        &mut self,
        layer: usize,
        adjustment: Option<Adjustment>,
    ) -> Result<(), String> {
        if !self.editable(layer) || self.layers[layer].tileset.is_some() {
            return Err("Choose an unlocked image layer for the adjustment".into());
        }
        if let Some(adjustment) = &adjustment {
            adjustment.validate()?;
        }
        self.layers[layer].adjustment = adjustment;
        Ok(())
    }

    pub fn needs_baked_ase(&self) -> bool {
        self.layers.iter().any(|layer| {
            layer.adjustment.is_some()
                || layer.mask.is_some()
                || layer.cel_meta.iter().any(|cel| cel.keyframe.is_some())
        })
    }

    pub fn same_frame_pixels(&self, other: &Self, frame: usize) -> bool {
        if frame >= self.durations.len() || frame >= other.durations.len() {
            return false;
        }
        self.width == other.width
            && self.height == other.height
            && self.compose_groups == other.compose_groups
            && self.color_profile == other.color_profile
            && self.layers.len() == other.layers.len()
            && self
                .layers
                .iter()
                .zip(&other.layers)
                .enumerate()
                .all(|(layer, (a, b))| {
                    let (Some(left), Some(right)) = (a.cel_meta.get(frame), b.cel_meta.get(frame))
                    else {
                        return false;
                    };
                    let key = self.sampled_cel_key(layer, frame);
                    a.visible == b.visible
                        && a.group == b.group
                        && a.parent == b.parent
                        && a.opacity == b.opacity
                        && a.blend == b.blend
                        && a.mask == b.mask
                        && a.adjustment == b.adjustment
                        && key == other.sampled_cel_key(layer, frame)
                        && left.opacity == right.opacity
                        && left.z_index == right.z_index
                        && a.is_ase_reference() == b.is_ase_reference()
                        && (!a.is_ase_reference()
                            || (left.reference_bounds == right.reference_bounds
                                && left.bounds == right.bounds
                                && left.outside == right.outside))
                        && match (a.tileset, b.tileset) {
                            (None, None) => {
                                a.cels[frame] == b.cels[frame]
                                    && ((key.x == 0 && key.y == 0) || left.outside == right.outside)
                            }
                            (Some(a_set), Some(b_set)) => {
                                left.tilemap == right.tilemap
                                    && a.tile_cels[frame] == b.tile_cels[frame]
                                    && self.tilesets.get(a_set) == other.tilesets.get(b_set)
                            }
                            _ => false,
                        }
                })
    }

    pub fn composite(&self, f: usize) -> Vec<Pixel> {
        self.composite_group(f, None, 0, None, false)
    }
    pub fn composite_view(&self, f: usize) -> Vec<Pixel> {
        self.composite_group(f, None, 0, None, true)
    }
    fn visit_scaled_reference(
        &self,
        layer: usize,
        frame: usize,
        [dx, dy]: [i32; 2],
        mut visit: impl FnMut(Option<usize>, usize, Pixel),
    ) {
        const FIXED: i64 = 65_536;
        const HALF: i64 = FIXED / 2;
        let layer = &self.layers[layer];
        let meta = &layer.cel_meta[frame];
        let Some([x, y, w, h]) = meta.reference_bounds.map(|b| b.map(i64::from)) else {
            return;
        };
        let [sx, sy, sw, sh] = meta
            .bounds
            .unwrap_or([0, 0, self.width as i32, self.height as i32]);
        if w <= 0 || h <= 0 || sw <= 0 || sh <= 0 {
            return;
        }
        let outside: std::collections::HashMap<_, _> = meta
            .outside
            .iter()
            .map(|pixel| ((pixel.x, pixel.y), pixel.color))
            .collect();
        let x = x + i64::from(dx) * FIXED;
        let y = y + i64::from(dy) * FIXED;
        let edge = |value: i64, limit: usize| {
            (value - HALF + FIXED - 1)
                .div_euclid(FIXED)
                .clamp(0, limit as i64) as usize
        };
        let left = edge(x, self.width);
        let right = edge(x + w, self.width);
        let top = edge(y, self.height);
        let bottom = edge(y + h, self.height);
        for row in top..bottom {
            let py = sy + ((row as i64 * FIXED + HALF - y) * i64::from(sh) / h) as i32;
            for col in left..right {
                let px = sx + ((col as i64 * FIXED + HALF - x) * i64::from(sw) / w) as i32;
                let source = self.index(px, py);
                let pixel = source.map_or_else(
                    || outside.get(&(px, py)).copied().unwrap_or(CLEAR),
                    |index| layer.cels[frame][index],
                );
                visit(source, row * self.width + col, pixel);
            }
        }
    }
    pub(crate) fn composite_layer_for_export(&self, frame: usize, layer: usize) -> Vec<Pixel> {
        let mut include = vec![false; self.layers.len()];
        let mut index = Some(layer);
        while let Some(layer) = index {
            include[layer] = true;
            index = self.layers[layer].parent;
        }
        self.composite_group(frame, None, 0, Some(&include), false)
    }
    fn composite_group(
        &self,
        f: usize,
        parent: Option<usize>,
        depth: usize,
        include: Option<&[bool]>,
        show_references: bool,
    ) -> Vec<Pixel> {
        let mut out = vec![CLEAR; self.width * self.height];
        self.composite_into(f, parent, depth, include, show_references, &mut out);
        out
    }
    fn composite_into(
        &self,
        f: usize,
        parent: Option<usize>,
        depth: usize,
        include: Option<&[bool]>,
        show_references: bool,
        out: &mut Vec<Pixel>,
    ) {
        if depth > self.layers.len() {
            return;
        }
        let mut order: Vec<_> = self
            .layers
            .iter()
            .enumerate()
            .filter(|(index, layer)| {
                layer.parent == parent
                    && layer.visible
                    && (show_references || !layer.is_ase_reference())
                    && include.is_none_or(|flags| flags[*index])
            })
            .map(|(i, l)| (i as i32 + l.cel_meta[f].z_index as i32, i))
            .collect();
        order.sort_by_key(|p| (p.0, self.layers[p.1].cel_meta[f].z_index));
        let mut transparent_rgb_possible = out.iter().any(|p| p[3] == 0 && *p != CLEAR);
        for (_, i) in order {
            let l = &self.layers[i];
            let key = self.sampled_cel_key(i, f);
            if l.group && !self.compose_groups && l.mask.is_none() && key == CelKeyframe::default()
            {
                self.composite_into(f, Some(i), depth + 1, include, show_references, out);
                transparent_rgb_possible = out.iter().any(|p| p[3] == 0 && *p != CLEAR);
                continue;
            }
            let opacity = ((l.opacity as u16 * l.cel_meta[f].opacity as u16 + 127) / 255) as u8;
            let opacity = ((u16::from(opacity) * u16::from(key.opacity) + 127) / 255) as u8;
            let destination = |index: usize| {
                if key.x == 0 && key.y == 0 {
                    return Some(index);
                }
                self.index(
                    (index % self.width) as i32 + key.x,
                    (index / self.width) as i32 + key.y,
                )
            };
            let coverage = |index: usize| {
                l.mask.as_ref().map_or(opacity, |mask| {
                    ((u16::from(opacity) * u16::from(mask[index][3]) + 127) / 255) as u8
                })
            };
            if l.is_ase_reference() && l.cel_meta[f].reference_bounds.is_some() {
                self.visit_scaled_reference(i, f, [key.x, key.y], |source, dest, pixel| {
                    let alpha = source.map_or(opacity, coverage);
                    if pixel[3] != 0 && alpha != 0 {
                        out[dest] = blend(out[dest], pixel, alpha, l.blend);
                    }
                });
                continue;
            }
            if let Some(adjustment) = &l.adjustment {
                for (index, pixel) in out
                    .iter_mut()
                    .enumerate()
                    .filter(|(_, pixel)| pixel[3] != 0)
                {
                    let Some(source) = self.index(
                        (index % self.width) as i32 - key.x,
                        (index / self.width) as i32 - key.y,
                    ) else {
                        continue;
                    };
                    let adjusted = adjustment.apply(*pixel);
                    let alpha = u16::from(coverage(source));
                    for (channel, target) in pixel[..3].iter_mut().zip(adjusted) {
                        *channel = ((u16::from(*channel) * (255 - alpha)
                            + u16::from(target) * alpha
                            + 127)
                            / 255) as u8;
                    }
                }
                continue;
            }
            if (key.x != 0 || key.y != 0) && l.tileset.is_none() && !l.group && l.mask.is_none() {
                for source in &l.cel_meta[f].outside {
                    if let Some(index) = self.index(source.x + key.x, source.y + key.y) {
                        let target = &mut out[index];
                        if source.color[3] != 0 || target[3] == 0 {
                            *target = blend(*target, source.color, opacity, l.blend);
                            transparent_rgb_possible |= target[3] == 0 && *target != CLEAR;
                        }
                    }
                }
            }
            if let Some(set) = l.tileset.and_then(|index| self.tilesets.get(index))
                && (key.x != 0 || key.y != 0)
                && l.mask.is_none()
            {
                let (x, y, columns, rows) = l.cel_meta[f].tilemap.as_ref().map_or_else(
                    || {
                        (
                            0,
                            0,
                            self.width.div_ceil(set.width),
                            self.height.div_ceil(set.height),
                        )
                    },
                    |g| (g.x, g.y, g.columns, g.rows),
                );
                if x < 0
                    || y < 0
                    || i64::from(x) + (columns * set.width) as i64 > self.width as i64
                    || i64::from(y) + (rows * set.height) as i64 > self.height as i64
                {
                    self.visit_tile_pixels(i, f, |x, y, color| {
                        if self.index(x, y).is_none()
                            && let Some(index) = self.index(x + key.x, y + key.y)
                            && (color[3] != 0 || (color != CLEAR && out[index][3] == 0))
                        {
                            out[index] = blend(out[index], color, opacity, l.blend);
                            transparent_rgb_possible |= out[index][3] == 0 && out[index] != CLEAR;
                        }
                    });
                }
            }
            let rendered;
            let pixels = if l.group {
                rendered = self.composite_group(f, Some(i), depth + 1, include, show_references);
                &rendered
            } else if l.tileset.is_some() {
                rendered = self.layer_pixels(i, f);
                &rendered
            } else if let Pixels::Dense(pixels) = &l.cels[f] {
                pixels.as_slice()
            } else {
                if transparent_rgb_possible {
                    for (index, target) in out.iter_mut().enumerate() {
                        if target[3] == 0
                            && *target != CLEAR
                            && self
                                .index(
                                    (index % self.width) as i32 - key.x,
                                    (index / self.width) as i32 - key.y,
                                )
                                .is_some_and(|source| coverage(source) != 0 || l.mask.is_none())
                        {
                            *target = CLEAR;
                        }
                    }
                }
                l.cels[f].visit_nonclear(|index, source| {
                    if let Some(dest) = destination(index) {
                        let target = &mut out[dest];
                        let opacity = coverage(index);
                        if (opacity != 0 || l.mask.is_none()) && (source[3] != 0 || target[3] == 0)
                        {
                            *target = blend(*target, source, opacity, l.blend);
                            transparent_rgb_possible |= target[3] == 0 && *target != CLEAR;
                        }
                    }
                });
                continue;
            };
            if key.x == 0 && key.y == 0 && l.mask.is_none() {
                for (target, &source) in out.iter_mut().zip(pixels) {
                    if opacity == 255 && source[3] == 255 && l.blend == BlendMode::Normal {
                        *target = source;
                    } else if source[3] != 0 || target[3] == 0 {
                        *target = blend(*target, source, opacity, l.blend);
                        transparent_rgb_possible |= target[3] == 0 && *target != CLEAR;
                    }
                }
                continue;
            }
            for (index, &s) in pixels.iter().enumerate() {
                if let Some(dest) = destination(index) {
                    let d = &mut out[dest];
                    let opacity = coverage(index);
                    if (opacity != 0 || l.mask.is_none()) && (s[3] != 0 || d[3] == 0) {
                        *d = blend(*d, s, opacity, l.blend);
                        transparent_rgb_possible |= d[3] == 0 && *d != CLEAR;
                    }
                }
            }
        }
    }
    fn clear_cel(&mut self, layer: usize, frame: usize) {
        let layer = &mut self.layers[layer];
        layer.cels[frame] = Pixels::empty(self.width * self.height);
        let tile_count = layer.tile_cels[frame].len();
        layer.tile_cels[frame] = vec![0; tile_count];
        let geometry = layer.cel_meta[frame].tilemap.take();
        layer.cel_meta[frame] = CelMeta {
            editor_id: layer.cel_meta[frame].editor_id,
            tilemap: geometry,
            ..CelMeta::default()
        };
    }

    pub fn can_add_frame(&self, frame: usize, duplicate: bool) -> bool {
        frame < self.durations.len()
            && self
                .frame_export_ids
                .0
                .as_ref()
                .is_none_or(|ids| u32::try_from(ids.next).is_ok())
            && self.durations.len() < u16::MAX as usize
            && self.layers.len().saturating_mul(self.durations.len() + 1) <= MAX_CEL_SLOTS
            && self.can_grow(self.layers.iter().fold(0usize, |total, layer| {
                let tiles = layer.tile_cels[frame].len();
                let pixels = if duplicate && layer.cels[frame].backing_id().is_some() {
                    layer.cels[frame].allocation_pixels()
                } else {
                    0
                };
                total.saturating_add(tiles).saturating_add(pixels)
            }))
    }

    pub fn can_add_layer(&self) -> bool {
        self.layers.len() < u16::MAX as usize
            && (self.layers.len() + 1).saturating_mul(self.durations.len()) <= MAX_CEL_SLOTS
    }

    pub fn add_frame(&mut self, f: usize, duplicate: bool) {
        if !self.can_add_frame(f, duplicate) {
            return;
        }
        if self.frame_export_ids.0.is_some() || f + 1 < self.durations.len() {
            let ids = self.explicit_frame_export_ids();
            let Ok(id) = u32::try_from(ids.next) else {
                return;
            };
            ids.frames.insert(f + 1, id);
            ids.next += 1;
        }
        for l in &mut self.layers {
            l.cels.insert(
                f + 1,
                if duplicate {
                    l.cels[f].clone()
                } else {
                    Pixels::empty(self.width * self.height)
                },
            );
            l.cel_meta.insert(
                f + 1,
                if duplicate {
                    CelMeta {
                        editor_id: EditorId::default(),
                        ..l.cel_meta[f].clone()
                    }
                } else {
                    CelMeta {
                        tilemap: l.cel_meta[f].tilemap.clone(),
                        ..CelMeta::default()
                    }
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
        if f < self.frame_names.len() {
            let name = if duplicate {
                self.frame_names[f].clone()
            } else {
                String::new()
            };
            self.frame_names.insert(f + 1, name);
            self.prune_frame_names();
        }
        if duplicate {
            for index in 0..self.layers.len() {
                if self.layers[index].link_duplicates && !self.layers[index].group {
                    self.set_cel_link(index, f + 1, f);
                } else {
                    self.layers[index].cel_meta[f + 1].artwork_link = None;
                }
            }
        }

        if !self.frame_palettes.is_empty() {
            let p = self
                .frame_palettes
                .get(f)
                .cloned()
                .unwrap_or_else(|| self.palette.clone());
            self.frame_palettes.insert(f + 1, p);
        }
        for slice in &mut self.slices {
            for key in &mut slice.keys {
                if key.frame > f {
                    key.frame += 1;
                }
            }
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
        if self.durations.len() > 1 && f < self.durations.len() {
            self.metadata.retain_mut(|chunk| {
                if chunk.is_frame_independent() {
                    return true;
                }
                if chunk.frame == f {
                    let replacement = chunk.cel_user_data_layer(&self.layers).and_then(|layer| {
                        let link = layer.cel_meta[f].artwork_link?;
                        if layer.cel_meta[..f]
                            .iter()
                            .any(|cel| cel.artwork_link == Some(link))
                        {
                            return None;
                        }
                        let next = layer.cel_meta[f + 1..]
                            .iter()
                            .position(|cel| cel.artwork_link == Some(link))?;
                        Some(f + 1 + next)
                    });
                    let Some(replacement) = replacement else {
                        return false;
                    };
                    chunk.frame = replacement;
                }
                if chunk.frame > f {
                    chunk.frame -= 1;
                }
                true
            });
            self.explicit_frame_export_ids().frames.remove(f);
            self.durations.remove(f);
            if f < self.frame_names.len() {
                self.frame_names.remove(f);
                self.prune_frame_names();
            }
            if f < self.frame_palettes.len() {
                self.frame_palettes.remove(f);
            }
            for l in &mut self.layers {
                l.cels.remove(f);
                l.cel_meta.remove(f);
                l.tile_cels.remove(f);
                l.prune_cel_links();
            }
            for slice in &mut self.slices {
                slice.keys.retain(|k| k.frame != f);
                for key in &mut slice.keys {
                    if key.frame > f {
                        key.frame -= 1;
                    }
                }
            }
            self.retain_tags(|_, t| !(t.from == f && t.to == f));
            for t in &mut self.tags {
                if t.from > f {
                    t.from -= 1
                }
                if t.to >= f {
                    t.to = t.to.saturating_sub(1)
                }
            }
        }
    }
    /// Remove a tag and its positional ASE user-data chunk.
    pub fn remove_tag(&mut self, index: usize) -> bool {
        if index >= self.tags.len() {
            return false;
        }
        self.retain_tags(|i, _| i != index);
        true
    }

    fn retain_tags(&mut self, mut keep: impl FnMut(usize, &FrameTag) -> bool) {
        let mut keep_tag = Vec::with_capacity(self.tags.len());
        self.tags.retain(|tag| {
            let retain = keep(keep_tag.len(), tag);
            keep_tag.push(retain);
            retain
        });
        if keep_tag.iter().all(|&keep| keep) {
            return;
        }
        let mut index = 0;
        self.metadata.retain(|chunk| {
            if !chunk.is_tag_user_data() {
                return true;
            }
            let retain = keep_tag.get(index).copied().unwrap_or(true);
            index += 1;
            retain
        });
    }

    pub fn add_layer(&mut self) {
        if self.can_add_layer() {
            self.layers.push(Layer::new(
                format!("Layer {}", self.layers.len() + 1),
                self.width,
                self.height,
                self.durations.len(),
            ));
        }
    }
    pub fn add_group(&mut self, name: &str) -> Result<usize, String> {
        if !self.can_add_layer() {
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
        if !self.can_add_layer() || !self.can_grow(count.saturating_mul(self.durations.len())) {
            return Err("Layer pixel limit reached".into());
        }
        self.add_layer();
        let l = self.layers.last_mut().unwrap();
        l.tileset = Some(tileset);
        l.name = "Tile layer".into();
        l.tile_cels = vec![vec![0; count]; self.durations.len()];
        Ok(())
    }
    pub fn tile_at_point(
        &self,
        layer: usize,
        frame: usize,
        point: (i32, i32),
    ) -> Option<(TilemapGeometry, usize, usize)> {
        let layer = self.layers.get(layer)?;
        let set = self.tilesets.get(layer.tileset?)?;
        let geometry = layer
            .cel_meta
            .get(frame)?
            .tilemap
            .clone()
            .unwrap_or(TilemapGeometry {
                x: 0,
                y: 0,
                columns: self.width.div_ceil(set.width),
                rows: self.height.div_ceil(set.height),
            });
        let x = usize::try_from(i64::from(point.0) - i64::from(geometry.x)).ok()? / set.width;
        let y = usize::try_from(i64::from(point.1) - i64::from(geometry.y)).ok()? / set.height;
        if x >= geometry.columns || y >= geometry.rows {
            return None;
        }
        layer.tile_cels.get(frame)?.get(y * geometry.columns + x)?;
        Some((geometry, x, y))
    }

    pub fn paint_matching_tiles(
        &mut self,
        layer: usize,
        frame: usize,
        tile: u32,
        hits: &[bool],
    ) -> Result<(), String> {
        if !self.editable(layer) {
            return Err("Layer is locked".into());
        }
        let layer = &mut self.layers[layer];
        let tile_count = layer
            .tileset
            .and_then(|index| self.tilesets.get(index))
            .ok_or("Select a tile layer")?
            .tiles
            .len();
        if (tile & 0x1fffffff) as usize > tile_count
            || layer
                .tile_cels
                .get(frame)
                .is_none_or(|cells| cells.len() != hits.len())
        {
            return Err("Tile is outside the map or tileset".into());
        }
        let link = layer.cel_meta[frame].artwork_link;
        for (target, cells) in layer.tile_cels.iter_mut().enumerate() {
            if target == frame || link.is_some() && layer.cel_meta[target].artwork_link == link {
                for (cell, hit) in cells.iter_mut().zip(hits) {
                    if *hit {
                        *cell = tile;
                    }
                }
            }
        }
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
        let layer = &mut self.layers[l];
        let link = layer.cel_meta[f].artwork_link;
        for (target, cells) in layer.tile_cels.iter_mut().enumerate() {
            if (target == f || link.is_some() && layer.cel_meta[target].artwork_link == link)
                && let Some(cell) = cells.get_mut(y * cols + x)
            {
                *cell = tile;
            }
        }
        Ok(())
    }
    pub fn edit_tile_pixel(
        &mut self,
        layer: usize,
        frame: usize,
        cell: (usize, usize),
        pixel: (usize, usize),
        color: Pixel,
        variant: bool,
    ) -> Result<(), String> {
        let mode = if variant {
            TilePixelEdit::ReuseVariant
        } else {
            TilePixelEdit::Shared
        };
        self.paint_tile_pixel(layer, frame, cell, pixel, color, mode)
    }

    pub fn paint_tile_pixel(
        &mut self,
        layer: usize,
        frame: usize,
        cell: (usize, usize),
        pixel: (usize, usize),
        color: Pixel,
        mode: TilePixelEdit,
    ) -> Result<(), String> {
        if !self.editable(layer) || frame >= self.durations.len() {
            return Err("Choose an unlocked tile cel".into());
        }
        let tileset = self.layers[layer].tileset.ok_or("Choose a tile layer")?;
        let set = &self.tilesets[tileset];
        let geometry = self.layers[layer].cel_meta[frame].tilemap.as_ref();
        let columns = geometry.map_or(self.width.div_ceil(set.width), |g| g.columns);
        let rows = geometry.map_or(self.height.div_ceil(set.height), |g| g.rows);
        let (x, y) = cell;
        let (px, py) = pixel;
        if x >= columns || y >= rows || px >= set.width || py >= set.height {
            return Err("Tile or pixel is outside the map".into());
        }
        let slot = y * columns + x;
        let placed = self.layers[layer].tile_cels[frame][slot];
        let id = (placed & TILE_ID_MASK) as usize;
        let offset = set
            .source_offset(placed, px, py)
            .ok_or("This pixel is outside the flipped tile")?;
        if id == 0 && color == CLEAR {
            return Ok(());
        }
        if id > set.tiles.len() {
            return Err("Tile placement refers to a missing tile".into());
        }
        if mode == TilePixelEdit::Shared && id != 0 {
            self.tilesets[tileset].tiles[id - 1].set(offset, color);
            return Ok(());
        }
        let mut artwork = if id == 0 {
            Pixels::empty(set.width * set.height)
        } else {
            set.tiles[id - 1].clone()
        };
        if mode == TilePixelEdit::CreateVariant && artwork[offset] == color {
            return Ok(());
        }
        artwork.set(offset, color);
        let existing = if mode == TilePixelEdit::CreateVariant {
            None
        } else {
            set.tiles.iter().position(|tile| *tile == artwork)
        };
        let new_id = if let Some(existing) = existing {
            existing + 1
        } else {
            if !self.can_grow(artwork.allocation_pixels())
                || (set.tiles.len() + 1)
                    .saturating_mul(set.width)
                    .saturating_mul(set.height)
                    > MAX_PIXELS
            {
                return Err("Tile variant exceeds the tileset or project pixel budget".into());
            }
            self.tilesets[tileset].tiles.push(artwork);
            self.tilesets[tileset].tiles.len()
        };
        self.layers[layer].tile_cels[frame][slot] = (placed & !TILE_ID_MASK) | new_id as u32;
        self.layers[layer].cel_meta[frame].artwork_link = None;
        self.layers[layer].prune_cel_links();
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
            if let Some(p) = l.parent
                && p > index
            {
                l.parent = Some(p - 1);
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
                    && i > index
                {
                    m.owner = format!("{prefix}{}", i - 1);
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
                let source = sy * self.width + sx;
                self.restore_pixel(
                    l,
                    f,
                    y * self.width + x,
                    old[source],
                    old_indices.as_ref().map(|indices| indices[source]),
                );
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
