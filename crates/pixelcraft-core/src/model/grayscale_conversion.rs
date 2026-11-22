use super::{ColorMode, Document, Pixel, Pixels};
use std::collections::HashMap;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum GrayscaleMethod {
    #[default]
    Luminance,
    HsvValue,
    HslLightness,
}

impl GrayscaleMethod {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "luma" | "luminance" | "default" => Ok(Self::Luminance),
            "hsv" | "value" => Ok(Self::HsvValue),
            "hsl" | "lightness" => Ok(Self::HslLightness),
            _ => Err("Unsupported grayscale method".into()),
        }
    }

    fn convert(self, pixel: Pixel) -> Pixel {
        let [r, g, b, alpha] = pixel;
        let value = match self {
            Self::Luminance => {
                ((u32::from(r) * 2126 + u32::from(g) * 7152 + u32::from(b) * 722) / 10000) as u8
            }
            Self::HsvValue => r.max(g).max(b),
            Self::HslLightness => {
                ((u16::from(r.max(g).max(b)) + u16::from(r.min(g).min(b))) / 2) as u8
            }
        };
        [value, value, value, alpha]
    }
}

impl Document {
    pub fn grayscale_conversion_until(
        &self,
        method: GrayscaleMethod,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, String> {
        self.grayscale_conversion_with_tile_frame_until(method, 0, cancelled)
    }

    /// Convert shared tiles with a chosen frame palette; raster cels keep their own.
    pub fn grayscale_conversion_with_tile_frame_until(
        &self,
        method: GrayscaleMethod,
        tile_frame: usize,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, String> {
        if tile_frame >= self.durations.len() {
            return Err("Choose a valid tile conversion frame".into());
        }
        let indexed = self.color_mode == ColorMode::Indexed;
        let mut next = if indexed {
            self.resolve_indexed_conversion_until(
                ColorMode::Grayscale,
                tile_frame,
                |pixel| method.convert(pixel),
                &cancelled,
            )?
        } else {
            self.validate()?;
            self.clone()
        };
        let mut shared = HashMap::<(usize, usize), Pixels>::new();
        let mut convert = |source: &Pixels| -> Result<Pixels, String> {
            if cancelled() {
                return Err("Grayscale conversion cancelled".into());
            }
            let Some(id) = source.backing_id() else {
                return Ok(source.clone());
            };
            let key = (id, source.len());
            if let Some(pixels) = shared.get(&key) {
                return Ok(pixels.clone());
            }
            let mut pixels = source.clone();
            for i in 0..source.len() {
                if i % 8192 == 0 && cancelled() {
                    return Err("Grayscale conversion cancelled".into());
                }
                let pixel = method.convert(source[i]);
                if pixel != source[i] {
                    pixels.set(i, pixel);
                }
            }
            shared.insert(key, pixels.clone());
            Ok(pixels)
        };
        for (layer_index, layer) in next.layers.iter_mut().enumerate() {
            for (frame, (pixels, meta)) in
                layer.cels.iter_mut().zip(&mut layer.cel_meta).enumerate()
            {
                // Indexed cels with authored slots were converted during expansion.
                // Legacy files may have only cached colors, which still need mapping.
                if !indexed || self.layers[layer_index].cel_meta[frame].indices.is_none() {
                    *pixels = convert(pixels)?;
                }
                meta.indices = None;
                if indexed {
                    continue;
                }
                for (i, hidden) in meta.outside.iter_mut().enumerate() {
                    if i % 8192 == 0 && cancelled() {
                        return Err("Grayscale conversion cancelled".into());
                    }
                    hidden.color = method.convert(hidden.color);
                    hidden.palette_slot = None;
                }
            }
        }
        if !indexed {
            for set in &mut next.tilesets {
                for tile in &mut set.tiles {
                    **tile = convert(tile)?;
                    tile.indices = None;
                }
            }
        }
        next.color_mode = ColorMode::Grayscale;
        next.drawing_index = None;
        next.validate()?;
        Ok(next)
    }
}
