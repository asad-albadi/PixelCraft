use super::{Pixel, Pixels, TileRaster, decode_indices};
use serde::{
    Deserialize, Serialize,
    de::{
        Error, MapAccess, SeqAccess, Visitor,
        value::{MapAccessDeserializer, SeqAccessDeserializer},
    },
    ser::SerializeStruct,
};

impl Serialize for TileRaster {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.indices.is_none() {
            return self.pixels.serialize(serializer);
        }
        let mut state = serializer.serialize_struct("IndexedTileRaster", 2)?;
        state.serialize_field("pixels", &self.pixels)?;
        state.serialize_field("indices", &self.indices)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for TileRaster {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct TileVisitor;
        impl<'de> Visitor<'de> for TileVisitor {
            type Value = TileRaster;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a tile raster or indexed tile raster")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<TileRaster, A::Error> {
                Pixels::deserialize(SeqAccessDeserializer::new(seq)).map(Into::into)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<TileRaster, A::Error> {
                let first = map
                    .next_key::<String>()?
                    .ok_or_else(|| A::Error::custom("Missing tile raster fields"))?;
                let indexed = matches!(first.as_str(), "pixels" | "indices");
                let map = FirstKey {
                    first: Some(first),
                    map,
                };
                if !indexed {
                    return Pixels::deserialize(MapAccessDeserializer::new(map)).map(Into::into);
                }
                #[derive(Deserialize)]
                struct Indexed {
                    pixels: Pixels,
                    #[serde(default, deserialize_with = "decode_indices")]
                    indices: Option<std::sync::Arc<Vec<u8>>>,
                }
                let indexed = Indexed::deserialize(MapAccessDeserializer::new(map))?;
                if indexed
                    .indices
                    .as_ref()
                    .is_some_and(|indices| indices.len() != indexed.pixels.len())
                {
                    return Err(A::Error::custom(
                        "Indexed tile size does not match its pixels",
                    ));
                }
                let mut tile: TileRaster = indexed.pixels.into();
                tile.indices = indexed.indices;
                Ok(tile)
            }
        }
        decoder.deserialize_any(TileVisitor)
    }
}

struct FirstKey<A> {
    first: Option<String>,
    map: A,
}
impl<'de, A: MapAccess<'de>> MapAccess<'de> for FirstKey<A> {
    type Error = A::Error;
    fn next_key_seed<K: serde::de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, A::Error> {
        match self.first.take() {
            Some(key) => seed
                .deserialize(serde::de::value::StringDeserializer::new(key))
                .map(Some),
            None => self.map.next_key_seed(seed),
        }
    }
    fn next_value_seed<V: serde::de::DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, A::Error> {
        self.map.next_value_seed(seed)
    }
}

impl TileRaster {
    /// Resolve a valid authored slot without allocating or overwriting cached artwork.
    /// Raw edits and legacy rasters retain their stored color when slot metadata is stale.
    pub fn pixel_for_palette(
        &self,
        index: usize,
        base: &[Pixel],
        palette: &[Pixel],
        transparent: usize,
    ) -> Pixel {
        if index >= self.pixels.len() {
            return super::CLEAR;
        }
        let fallback = self.pixels[index];
        let Some(slot) = self.matching_slot(index, base, transparent) else {
            return fallback;
        };
        let Some(mut color) = palette.get(slot as usize).copied() else {
            return fallback;
        };
        if slot as usize == transparent {
            color[3] = 0;
        }
        color
    }

    /// Return an authored slot only while its cached color still agrees.
    pub fn matching_slot(&self, index: usize, palette: &[Pixel], transparent: usize) -> Option<u8> {
        if index >= self.pixels.len() {
            return None;
        }
        let slot = *self.indices.as_ref()?.get(index)?;
        let mut color = *palette.get(slot as usize)?;
        let unmasked = color;
        if slot as usize == transparent {
            color[3] = 0;
        }
        // A newly selected mask slot may still have its former opaque cache.
        (self.pixels[index] == color || self.pixels[index] == unmasked).then_some(slot)
    }
}
