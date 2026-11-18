use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use pixelcraft::{formats, model::*};
use std::io::{Read, Write};

fn tileset_chunk(bytes: &[u8]) -> (usize, usize) {
    let mut at = 144;
    while at < bytes.len() {
        let size = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        if u16::from_le_bytes(bytes[at + 4..at + 6].try_into().unwrap()) == 0x2023 {
            return (at, size);
        }
        at += size;
    }
    panic!("missing tileset");
}
fn data_offset(chunk: &[u8]) -> usize {
    34 + u16::from_le_bytes(chunk[32..34].try_into().unwrap()) as usize
}
fn tile_indices(bytes: &[u8]) -> Vec<u8> {
    let (at, size) = tileset_chunk(bytes);
    let chunk = &bytes[at + 6..at + size];
    let offset = data_offset(chunk);
    let count = u32::from_le_bytes(chunk[offset..offset + 4].try_into().unwrap()) as usize;
    let mut out = vec![];
    ZlibDecoder::new(&chunk[offset + 4..offset + 4 + count])
        .read_to_end(&mut out)
        .unwrap();
    out
}
fn duplicate_slot_source() -> Vec<u8> {
    let red = [200, 100, 20, 255];
    let mut doc = Document::new(2, 1);
    doc.palette = vec![CLEAR, red, red];
    doc.set_color_mode(ColorMode::Indexed);
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "duplicate slots".into(),
        width: 2,
        height: 1,
        tiles: vec![vec![red, red].into()],
    });
    doc.add_tile_layer(0).unwrap();
    doc.paint_tile(1, 0, 0, 0, 1).unwrap();
    let mut bytes = formats::encode_ase(&doc).unwrap();
    let (at, size) = tileset_chunk(&bytes);
    let mut chunk = bytes[at + 6..at + size].to_vec();
    let offset = data_offset(&chunk);
    let mut encoder = ZlibEncoder::new(vec![], Compression::fast());
    encoder.write_all(&[0, 0, 2, 1]).unwrap();
    let compressed = encoder.finish().unwrap();
    chunk.truncate(offset);
    chunk.extend((compressed.len() as u32).to_le_bytes());
    chunk.extend(compressed);
    let mut replacement = ((chunk.len() + 6) as u32).to_le_bytes().to_vec();
    replacement.extend(0x2023u16.to_le_bytes());
    replacement.extend(chunk);
    let delta = replacement.len() as i64 - size as i64;
    bytes.splice(at..at + size, replacement);
    let length = bytes.len() as u32;
    bytes[..4].copy_from_slice(&length.to_le_bytes());
    let frame = (u32::from_le_bytes(bytes[128..132].try_into().unwrap()) as i64 + delta) as u32;
    bytes[128..132].copy_from_slice(&frame.to_le_bytes());
    bytes
}

#[test]
fn indexed_tiles_preserve_duplicate_palette_slots_through_ase_and_native_saves() {
    let source = duplicate_slot_source();
    assert_eq!(tile_indices(&source), vec![0, 0, 2, 1]);
    let imported = formats::decode_ase(&source).unwrap();
    assert_eq!(
        tile_indices(&formats::encode_ase(&imported).unwrap()),
        vec![0, 0, 2, 1]
    );
    let native = formats::decode_project(&formats::encode_project(&imported).unwrap()).unwrap();
    assert_eq!(
        tile_indices(&formats::encode_ase(&native).unwrap()),
        vec![0, 0, 2, 1]
    );
}

#[test]
fn native_tile_conversion_and_duplication_preserve_or_clear_slots_as_required() {
    let doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    for mode in [IndexedDithering::None, IndexedDithering::FloydSteinberg] {
        let converted = doc
            .indexed_conversion_with_fit_until(None, 2, mode, IndexedFit::Lab, || false)
            .unwrap();
        assert_eq!(
            tile_indices(&formats::encode_ase(&converted).unwrap()),
            vec![2, 2, 0, 1]
        );
        assert_eq!(
            converted.tilesets[0].tiles[0]
                .indices
                .as_ref()
                .unwrap()
                .as_slice(),
            &[0, 1]
        );
    }
    let rgb = doc.rgba_conversion_until(|| false).unwrap();
    assert!(rgb.tilesets[0].tiles[0].indices.is_none());
    let gray = doc
        .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
        .unwrap();
    assert!(gray.tilesets[0].tiles[0].indices.is_none());
    let mut duplicate = doc.clone();
    duplicate
        .edit_tile_definition(0, 1, TileDefinitionEdit::Duplicate)
        .unwrap();
    assert_eq!(
        tile_indices(&formats::encode_ase(&duplicate).unwrap()),
        vec![0, 0, 2, 1, 2, 1]
    );
}

#[test]
fn indexed_tile_wire_accepts_legacy_shapes_and_rejects_mismatched_metadata() {
    for wire in [
        serde_json::json!([[1, 2, 3, 4]]),
        serde_json::json!({"clear_pixels":1}),
    ] {
        let tile: TileRaster = serde_json::from_value(wire).unwrap();
        assert!(tile.indices.is_none());
    }
    assert!(
        serde_json::from_value::<TileRaster>(
            serde_json::json!({"pixels":[[1,2,3,4]],"indices":[2,1]})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<TileRaster>(
            serde_json::json!({"pixels":{"clear_pixels":1},"indices":[256]})
        )
        .is_err()
    );
    let mut tile: TileRaster = vec![[1, 2, 3, 4]].into();
    tile.indices = Some(std::sync::Arc::new(vec![1, 2]));
    assert_eq!(tile.matching_slot(1, &[[0; 4], [1, 2, 3, 4]], 0), None);
}

#[test]
fn indexed_tile_geometry_moves_slots_with_pixels_and_direct_mode_changes_clear_them() {
    use pixelcraft::workflows::{self, Orientation, SpriteResampling};
    let doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let flipped = workflows::orient_sprite(&doc, Orientation::FlipHorizontal).unwrap();
    assert_eq!(
        tile_indices(&formats::encode_ase(&flipped).unwrap()),
        vec![0, 0, 1, 2]
    );
    for method in [
        SpriteResampling::Nearest,
        SpriteResampling::Bilinear,
        SpriteResampling::RotSprite,
    ] {
        let scaled = workflows::resample_sprite_with(&doc, [4, 2], method).unwrap();
        assert_eq!(
            scaled.tilesets[0].tiles[0]
                .indices
                .as_ref()
                .unwrap()
                .as_slice(),
            &[2, 2, 1, 1, 2, 2, 1, 1]
        );
        scaled.validate().unwrap();
    }
    let mut rgb = doc.clone();
    rgb.set_color_mode(ColorMode::Rgba);
    assert!(rgb.tilesets[0].tiles[0].indices.is_none());
}

#[test]
fn indexed_tile_export_rejects_unpopulated_transparency_without_panicking() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.transparent_index = 7;
    assert!(
        formats::encode_ase(&doc)
            .unwrap_err()
            .contains("transparent slot")
    );
}

#[test]
fn native_indexed_tile_painting_updates_slots_before_conversion() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let blue = [20, 100, 200, 255];
    doc.palette.push(blue);
    doc.drawing_index = Some(3);
    doc.paint_tile_pixel(1, 0, (0, 0), (0, 0), blue, TilePixelEdit::Shared)
        .unwrap();
    let rgb = doc.rgba_conversion_until(|| false).unwrap();
    assert_eq!(rgb.tilesets[0].tiles[0][0], blue);
    assert_eq!(
        doc.tilesets[0].tiles[0]
            .indices
            .as_ref()
            .unwrap()
            .as_slice(),
        &[3, 1]
    );
    doc.drawing_index = Some(2);
    doc.paint_tile_pixel(1, 0, (0, 0), (1, 0), doc.palette[2], TilePixelEdit::Shared)
        .unwrap();
    assert_eq!(
        doc.tilesets[0].tiles[0]
            .indices
            .as_ref()
            .unwrap()
            .as_slice(),
        &[3, 2]
    );
}

#[test]
fn shared_tile_painting_reuses_unique_raster_storage() {
    let mut doc = Document::new(64, 64);
    let red = [200, 100, 20, 255];
    let blue = [20, 100, 200, 255];
    doc.palette = vec![CLEAR, red, blue];
    doc.color_mode = ColorMode::Indexed;
    let mut tile: TileRaster = Pixels::Dense(std::sync::Arc::new(vec![red; 4096])).into();
    tile.indices = Some(std::sync::Arc::new(vec![1; 4096]));
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "shared edits".into(),
        width: 64,
        height: 64,
        tiles: vec![tile],
    });
    doc.layers[0].tileset = Some(0);
    doc.layers[0].tile_cels[0] = vec![1].into();
    doc.drawing_index = Some(2);
    let backing = doc.tilesets[0].tiles[0].backing_id();
    for x in 0..16 {
        doc.paint_tile_pixel(0, 0, (0, 0), (x, 0), blue, TilePixelEdit::Shared)
            .unwrap();
    }
    assert_eq!(
        doc.tilesets[0].tiles[0].backing_id(),
        backing,
        "shared painting must not clone the full tile per pixel"
    );
}

#[test]
fn indexed_tile_slots_survive_workspace_recovery() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Recovery {
        #[serde(with = "pixelcraft::formats::recovery_document")]
        document: Document,
    }
    let original = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let bytes = serde_json::to_vec(&Recovery { document: original }).unwrap();
    let restored: Recovery = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        tile_indices(&formats::encode_ase(&restored.document).unwrap()),
        vec![0, 0, 2, 1]
    );
}

#[test]
fn indexed_tile_variants_distinguish_equal_colors_with_different_slots() {
    for mode in [TilePixelEdit::ReuseVariant, TilePixelEdit::CreateVariant] {
        let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
        doc.drawing_index = Some(2);
        let color = doc.palette[2];
        doc.paint_tile_pixel(1, 0, (0, 0), (1, 0), color, mode)
            .unwrap();
        assert_eq!(doc.tilesets[0].tiles.len(), 2);
        assert_eq!(doc.layers[1].tile_cels[0], vec![2]);
        assert_eq!(
            doc.tilesets[0].tiles[0]
                .indices
                .as_ref()
                .unwrap()
                .as_slice(),
            &[2, 1]
        );
        assert_eq!(
            doc.tilesets[0].tiles[1]
                .indices
                .as_ref()
                .unwrap()
                .as_slice(),
            &[2, 2]
        );
        assert_eq!(
            tile_indices(&formats::encode_ase(&doc).unwrap()),
            vec![0, 0, 2, 1, 2, 2]
        );
        if matches!(mode, TilePixelEdit::ReuseVariant) {
            doc.paint_tile_pixel(1, 0, (0, 0), (1, 0), color, mode)
                .unwrap();
            assert_eq!(doc.tilesets[0].tiles.len(), 2);
        }
    }
}

#[test]
fn indexed_tile_palette_reversal_and_sort_remap_authored_slots() {
    let original = formats::decode_ase(&duplicate_slot_source()).unwrap();
    for sort in [false, true] {
        let mut doc = original.clone();
        doc.palette_names = vec!["mask".into(), "z-first".into(), "a-last".into()];
        let pixels = doc.composite_view(0);
        if sort {
            doc.sort_palette(&[], PaletteSort::Name).unwrap();
        } else {
            doc.reverse_palette_entries(&[1, 2]).unwrap();
        }
        assert_eq!(
            doc.tilesets[0].tiles[0]
                .indices
                .as_ref()
                .unwrap()
                .as_slice(),
            &[1, 2]
        );
        assert_eq!(doc.composite_view(0), pixels);
        assert_eq!(
            tile_indices(&formats::encode_ase(&doc).unwrap()),
            vec![0, 0, 1, 2]
        );
    }
    let mut doc = original.clone();
    doc.reverse_palette_entries(&[0, 2]).unwrap();
    assert_eq!(doc.transparent_index, 2);
    assert_eq!(
        tile_indices(&formats::encode_ase(&doc).unwrap()),
        vec![2, 2, 0, 1]
    );
    assert_eq!(doc.composite_view(0), original.composite_view(0));
}

#[test]
fn indexed_tile_ramps_remap_equal_colors_by_authored_slot() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let red = doc.palette[1];
    doc.palette = vec![CLEAR, [0, 0, 0, 255], red, red];
    doc.frame_palettes.clear();
    doc.tilesets[0].tiles[0].indices = Some(std::sync::Arc::new(vec![2, 3]));
    doc.replace_palette_span(1, 3, 3).unwrap();
    assert_eq!(doc.tilesets[0].tiles[0][0], [100, 50, 10, 255]);
    assert_eq!(doc.tilesets[0].tiles[0][1], red);
    assert_eq!(
        tile_indices(&formats::encode_ase(&doc).unwrap()),
        vec![0, 0, 2, 3]
    );
    assert_eq!(
        doc.rgba_conversion_until(|| false).unwrap().tilesets[0].tiles[0].pixels,
        doc.tilesets[0].tiles[0].pixels
    );

    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.edit_tile_definition(0, 1, TileDefinitionEdit::Duplicate)
        .unwrap();
    let pixels = doc.tilesets[0].tiles[0].backing_id();
    doc.replace_palette_span(1, 2, 3).unwrap();
    assert_eq!(
        doc.tilesets[0].tiles[0]
            .indices
            .as_ref()
            .unwrap()
            .as_slice(),
        &[3, 1]
    );
    assert!(std::sync::Arc::ptr_eq(
        doc.tilesets[0].tiles[0].indices.as_ref().unwrap(),
        doc.tilesets[0].tiles[1].indices.as_ref().unwrap()
    ));
    assert_eq!(doc.tilesets[0].tiles[0].backing_id(), pixels);
}

#[test]
fn indexed_tile_ramp_moves_the_mask_slot_and_keeps_transparency() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let red = doc.palette[1];
    doc.palette = vec![[0, 0, 0, 255], CLEAR, red, [255; 4]];
    doc.frame_palettes.clear();
    doc.transparent_index = 1;
    doc.tilesets[0].tiles[0].pixels = vec![CLEAR, red].into();
    doc.tilesets[0].tiles[0].indices = Some(std::sync::Arc::new(vec![1, 2]));
    doc.replace_palette_span(0, 3, 7).unwrap();
    assert_eq!(doc.transparent_index, 2);
    assert_eq!(doc.tilesets[0].tiles[0][0], CLEAR);
    assert_eq!(doc.tilesets[0].tiles[0][1], [153, 153, 153, 255]);
    assert_eq!(
        tile_indices(&formats::encode_ase(&doc).unwrap()),
        vec![2, 2, 2, 4]
    );
}

#[test]
fn tile_palette_edits_preserve_shared_slots_and_native_round_trip() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.edit_tile_definition(0, 1, TileDefinitionEdit::Duplicate)
        .unwrap();
    let slots = doc.tilesets[0].tiles[0].indices.clone();
    doc.layers[0].cel_meta[0].indices = slots;
    let before = doc.clone();
    doc.reverse_palette_entries(&[1, 2]).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        doc.layers[0].cel_meta[0].indices.as_ref().unwrap(),
        doc.tilesets[0].tiles[0].indices.as_ref().unwrap()
    ));
    assert!(std::sync::Arc::ptr_eq(
        doc.tilesets[0].tiles[0].indices.as_ref().unwrap(),
        doc.tilesets[0].tiles[1].indices.as_ref().unwrap()
    ));
    assert_eq!(
        before.tilesets[0].tiles[0]
            .indices
            .as_ref()
            .unwrap()
            .as_slice(),
        &[2, 1]
    );
    let restored = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
    assert_eq!(
        tile_indices(&formats::encode_ase(&restored).unwrap()),
        vec![0, 0, 1, 2, 1, 2]
    );
    doc.replace_palette_span(1, 2, 3).unwrap();
    let restored = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
    assert_eq!(
        tile_indices(&formats::encode_ase(&restored).unwrap()),
        vec![0, 0, 1, 3, 1, 3]
    );
}

#[test]
fn tile_palette_ramp_handles_stale_and_legacy_slots_without_restoring_old_pixels() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let red = doc.palette[1];
    doc.palette = vec![CLEAR, [0, 0, 0, 255], red, red];
    doc.frame_palettes.clear();
    doc.tilesets[0].tiles[0].indices = Some(std::sync::Arc::new(vec![2, 3]));
    let raw = [11, 22, 33, 255];
    doc.tilesets[0].tiles[0].pixels.set(0, raw);
    doc.edit_tile_definition(0, 1, TileDefinitionEdit::Duplicate)
        .unwrap();
    doc.tilesets[0].tiles[1].indices = None;
    let original = doc.clone();
    doc.replace_palette_span(1, 3, 3).unwrap();
    assert_eq!(doc.tilesets[0].tiles[0][0], raw);
    assert_eq!(doc.tilesets[0].tiles[1][0], raw);
    assert_eq!(doc.tilesets[0].tiles[1][1], red);
    assert!(doc.tilesets[0].tiles[1].indices.is_none());
    assert_eq!(
        doc.rgba_conversion_until(|| false).unwrap().tilesets[0].tiles[0][0],
        raw
    );
    assert_eq!(original.tilesets[0].tiles[0][0], raw);
}

#[test]
fn invalid_tile_palette_edits_leave_document_unchanged() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let before = doc.clone();
    assert!(doc.replace_palette_span(0, 2, 1).is_err());
    assert_eq!(doc, before);
    doc.frame_palettes = vec![vec![CLEAR, doc.palette[1]]];
    let before = doc.clone();
    assert!(doc.reverse_palette_entries(&[1, 2]).is_err());
    assert_eq!(doc, before);
    assert!(doc.replace_palette_span(1, 2, 3).is_err());
    assert_eq!(doc, before);
}

#[test]
fn indexed_tiles_render_effective_frame_palettes_in_every_sampling_path() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.add_frame(0, true);
    let blue = [10, 20, 240, 255];
    let green = [10, 230, 20, 255];
    doc.frame_palettes = vec![doc.palette.clone(), vec![CLEAR, green, blue]];
    assert_eq!(doc.layer_pixels(1, 1), vec![blue, green]);
    assert_eq!(doc.sample_layer(1, 1, 0, 0), Some(blue));
    assert_eq!(doc.sample_composite(1, 0, 0, false), Some(blue));
    assert_eq!(doc.composite_view(1), vec![blue, green]);
    assert_eq!(
        doc.composite_view_region(1, [0, 0], [1, 1]),
        Some(vec![blue])
    );
    doc.layers[1].tile_cels[1][0] = 0x8000_0001;
    assert_eq!(doc.composite(1), vec![green, blue]);
    let restored = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
    assert_eq!(restored.composite(1), doc.composite(1));
    let restored = formats::decode_ase(&formats::encode_ase(&doc).unwrap()).unwrap();
    assert_eq!(restored.composite(1), doc.composite(1));
}

#[test]
fn indexed_tile_palette_changes_invalidate_frame_render_comparisons() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let before = doc.clone();
    doc.frame_palettes = vec![doc.palette.clone()];
    doc.frame_palettes[0][2] = [1, 2, 3, 255];
    assert!(!doc.same_frame_pixels(&before, 0));
    let mut doc = before.clone();
    doc.transparent_index = 2;
    assert!(!doc.same_frame_pixels(&before, 0));
}

#[test]
fn indexed_tile_frame_palette_keeps_mask_and_stale_or_legacy_fallbacks() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.add_frame(0, true);
    let red = doc.palette[1];
    doc.tilesets[0].tiles[0].pixels = vec![CLEAR, red].into();
    doc.tilesets[0].tiles[0].indices = Some(std::sync::Arc::new(vec![0, 1]));
    doc.frame_palettes = vec![
        doc.palette.clone(),
        vec![[77, 88, 99, 255], [1, 2, 3, 255], red],
    ];
    assert_eq!(
        doc.layer_pixels(1, 1),
        vec![[77, 88, 99, 0], [1, 2, 3, 255]]
    );
    let raw = [8, 9, 10, 255];
    doc.tilesets[0].tiles[0].pixels.set(1, raw);
    assert_eq!(doc.layer_pixels(1, 1)[1], raw);
    doc.tilesets[0].tiles[0].indices = None;
    assert_eq!(doc.layer_pixels(1, 1), vec![CLEAR, raw]);
}

#[test]
fn indexed_tile_render_mask_changes_apply_to_previously_opaque_slots() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let red = doc.palette[1];
    doc.transparent_index = 2;
    assert_eq!(
        doc.layer_pixels(1, 0),
        vec![[red[0], red[1], red[2], 0], red]
    );
}

#[test]
fn indexed_tile_paint_chooses_slots_from_the_active_frame_palette() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.add_frame(0, true);
    let blue = [10, 20, 240, 255];
    let green = [10, 230, 20, 255];
    doc.frame_palettes = vec![doc.palette.clone(), vec![CLEAR, green, blue]];
    doc.drawing_index = Some(2);
    doc.paint_tile_pixel(1, 1, (0, 0), (1, 0), blue, TilePixelEdit::Shared)
        .unwrap();
    assert_eq!(
        doc.tilesets[0].tiles[0]
            .indices
            .as_deref()
            .unwrap()
            .as_slice(),
        &[2, 2]
    );
    assert_eq!(doc.layer_pixels(1, 1), vec![blue, blue]);
    assert_eq!(doc.layer_pixels(1, 0), vec![doc.palette[2]; 2]);
}

#[test]
fn indexed_tile_slot_only_edits_invalidate_render_damage() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.frame_palettes = vec![vec![CLEAR, [1, 2, 3, 255], [4, 5, 6, 255]]];
    let before = doc.clone();
    doc.tilesets[0].tiles[0].indices = Some(std::sync::Arc::new(vec![1, 1]));
    assert_ne!(doc.frame_pixel_damage(&before, 0), Some(vec![]));
    assert_ne!(doc.composite(0), before.composite(0));
}

#[test]
fn indexed_tile_paint_accepts_frame_only_palette_slots() {
    for mode in [
        TilePixelEdit::Shared,
        TilePixelEdit::CreateVariant,
        TilePixelEdit::ReuseVariant,
    ] {
        let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
        doc.add_frame(0, true);
        let yellow = [250, 240, 10, 255];
        doc.frame_palettes = vec![
            doc.palette.clone(),
            vec![CLEAR, [1, 2, 3, 255], [4, 5, 6, 255], yellow],
        ];
        doc.drawing_index = Some(3);
        doc.paint_tile_pixel(1, 1, (0, 0), (1, 0), yellow, mode)
            .unwrap();
        assert_eq!(doc.sample_layer(1, 1, 1, 0), Some(yellow));
        assert_eq!(doc.palette[3], yellow);
        let restored = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
        assert_eq!(restored.sample_layer(1, 1, 1, 0), Some(yellow));
    }
}

#[test]
fn indexed_tile_paint_rejects_missing_mask_without_panicking_or_mutating() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.transparent_index = 7;
    let original = doc.clone();
    assert!(
        doc.paint_tile_pixel(1, 0, (0, 0), (0, 0), CLEAR, TilePixelEdit::Shared)
            .is_err()
    );
    assert_eq!(doc, original);
}

#[test]
fn indexed_tile_conversion_uses_effective_first_frame_and_snapshot_palette() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.add_frame(0, true);
    let blue = [10, 20, 240, 255];
    let green = [10, 230, 20, 255];
    doc.frame_palettes = vec![vec![CLEAR, green, blue], vec![CLEAR, blue, green]];
    let original = doc.clone();
    let rgb = doc.rgba_conversion_until(|| false).unwrap();
    assert_eq!(rgb.layer_pixels(1, 0), vec![blue, green]);
    assert_eq!(
        rgb.layer_pixels(1, 1),
        vec![blue, green],
        "shared tile conversion uses first frame"
    );
    let gray = doc
        .grayscale_conversion_until(GrayscaleMethod::HsvValue, || false)
        .unwrap();
    assert_eq!(
        gray.layer_pixels(1, 0),
        vec![[240, 240, 240, 255], [230, 230, 230, 255]]
    );
    let snapshot = doc.conversion_frame_snapshot(1).unwrap();
    assert_eq!(
        snapshot
            .rgba_conversion_until(|| false)
            .unwrap()
            .layer_pixels(1, 0),
        vec![green, blue]
    );
    assert_eq!(
        snapshot
            .grayscale_conversion_until(GrayscaleMethod::HsvValue, || false)
            .unwrap()
            .layer_pixels(1, 0),
        vec![[230, 230, 230, 255], [240, 240, 240, 255]]
    );
    assert_eq!(doc, original);
}

#[test]
fn indexed_tile_direct_mode_conversion_uses_first_frame_palette() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    let blue = [10, 20, 240, 255];
    let green = [10, 230, 20, 255];
    doc.frame_palettes = vec![vec![CLEAR, green, blue]];
    let mut rgb = doc.clone();
    rgb.set_color_mode(ColorMode::Rgba);
    assert_eq!(rgb.layer_pixels(1, 0), vec![blue, green]);
    assert!(rgb.tilesets[0].tiles[0].indices.is_none());
    // Direct grayscale uses its existing default conversion formula.
    let mut expected = Document::new(2, 1);
    expected.layers[0].cels[0] = vec![blue, green].into();
    expected.set_color_mode(ColorMode::Grayscale);
    doc.set_color_mode(ColorMode::Grayscale);
    assert_eq!(doc.layer_pixels(1, 0), expected.layer_pixels(0, 0));
}

#[test]
fn indexed_tile_conversion_palette_preserves_sharing_fallbacks_and_source() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.frame_palettes = vec![vec![CLEAR, [1, 2, 3, 255], [4, 5, 6, 255]]];
    doc.edit_tile_definition(0, 1, TileDefinitionEdit::Duplicate)
        .unwrap();
    let original = doc.clone();
    let converted = doc.rgba_conversion_until(|| false).unwrap();
    assert_eq!(
        converted.tilesets[0].tiles[0].backing_id(),
        converted.tilesets[0].tiles[1].backing_id()
    );
    let restored = formats::decode_project(&formats::encode_project(&converted).unwrap()).unwrap();
    assert_eq!(restored.layer_pixels(1, 0), converted.layer_pixels(1, 0));
    let restored = formats::decode_ase(&formats::encode_ase(&converted).unwrap()).unwrap();
    assert_eq!(restored.layer_pixels(1, 0), converted.layer_pixels(1, 0));
    let calls = std::cell::Cell::new(0);
    assert!(
        doc.rgba_conversion_until(|| {
            calls.set(calls.get() + 1);
            calls.get() >= 3
        })
        .is_err()
    );
    assert_eq!(doc, original);
    // A valid base slot may be absent from a shorter effective palette.
    doc.frame_palettes = vec![vec![CLEAR, [1, 2, 3, 255]]];
    let converted = doc.rgba_conversion_until(|| false).unwrap();
    assert_eq!(converted.tilesets[0].tiles[0][0], doc.palette[2]);
    assert_eq!(converted.tilesets[0].tiles[0][1], [1, 2, 3, 255]);
    let raw = [11, 22, 33, 255];
    doc.tilesets[0].tiles[0].pixels.set(1, raw);
    assert_eq!(
        doc.rgba_conversion_until(|| false).unwrap().tilesets[0].tiles[0][1],
        raw
    );
    let mut direct = doc.clone();
    direct.set_color_mode(ColorMode::Rgba);
    assert_eq!(direct.tilesets[0].tiles[0][0], doc.palette[2]);
    assert_eq!(direct.tilesets[0].tiles[0][1], raw);
}

#[test]
fn indexed_tile_explicit_conversion_frame_matches_snapshot_and_rejects_invalid_frames() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.add_frame(0, true);
    doc.frame_palettes = vec![
        doc.palette.clone(),
        vec![CLEAR, [10, 20, 240, 255], [10, 230, 20, 255]],
    ];
    let original = doc.clone();
    assert_eq!(
        doc.rgba_conversion_with_tile_frame_until(1, || false)
            .unwrap()
            .layer_pixels(1, 1),
        doc.conversion_frame_snapshot(1)
            .unwrap()
            .rgba_conversion_until(|| false)
            .unwrap()
            .layer_pixels(1, 0)
    );
    assert_eq!(
        doc.grayscale_conversion_with_tile_frame_until(GrayscaleMethod::HsvValue, 1, || false)
            .unwrap()
            .layer_pixels(1, 1),
        doc.conversion_frame_snapshot(1)
            .unwrap()
            .grayscale_conversion_until(GrayscaleMethod::HsvValue, || false)
            .unwrap()
            .layer_pixels(1, 0)
    );
    assert!(
        doc.rgba_conversion_with_tile_frame_until(2, || false)
            .is_err()
    );
    assert!(
        doc.grayscale_conversion_with_tile_frame_until(
            GrayscaleMethod::HsvValue,
            usize::MAX,
            || false
        )
        .is_err()
    );
    assert_eq!(doc, original);
}

#[test]
fn command_tile_conversion_selects_frame_palette_and_rejects_invalid_frame() {
    let mut doc = formats::decode_ase(&duplicate_slot_source()).unwrap();
    doc.add_frame(0, true);
    doc.frame_palettes = vec![
        doc.palette.clone(),
        vec![CLEAR, [10, 20, 240, 255], [10, 230, 20, 255]],
    ];
    for op in ["convert-rgb", "convert-grayscale"] {
        let converted = pixelcraft::automation::apply_command_batch(
            &doc,
            &serde_json::json!({"commands":[{"op":op,"frame":1,"method":"hsv"}]}),
        )
        .unwrap();
        let expected = if op == "convert-rgb" {
            doc.rgba_conversion_with_tile_frame_until(1, || false)
                .unwrap()
        } else {
            doc.grayscale_conversion_with_tile_frame_until(GrayscaleMethod::HsvValue, 1, || false)
                .unwrap()
        };
        assert_eq!(converted.layer_pixels(1, 1), expected.layer_pixels(1, 1));
        assert!(
            pixelcraft::automation::apply_command_batch(
                &doc,
                &serde_json::json!({"commands":[{"op":op,"frame":2}]})
            )
            .is_err()
        );
    }
}
