use pixelcraft::{formats::*, model::*};
fn tile_doc() -> Document {
    let mut d = Document::new(3, 2);
    d.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "set".into(),
        width: 2,
        height: 2,
        tiles: vec![
            vec![
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 0, 255],
            ]
            .into(),
        ],
    });
    d.add_tile_layer(0).unwrap();
    d.paint_tile(1, 0, 0, 0, 1).unwrap();
    d
}
fn mutate_chunk(b: &mut Vec<u8>, kind: u16, change: impl FnOnce(&mut Vec<u8>)) {
    let mut p = 144;
    while p < b.len() {
        let n = u32::from_le_bytes(b[p..p + 4].try_into().unwrap()) as usize;
        if u16::from_le_bytes(b[p + 4..p + 6].try_into().unwrap()) == kind {
            let mut c = b[p + 6..p + n].to_vec();
            change(&mut c);
            let mut out = ((c.len() + 6) as u32).to_le_bytes().to_vec();
            out.extend(kind.to_le_bytes());
            out.extend(c);
            let delta = out.len() as i64 - n as i64;
            b.splice(p..p + n, out);
            let len = b.len() as u32;
            b[..4].copy_from_slice(&len.to_le_bytes());
            let size = (u32::from_le_bytes(b[128..132].try_into().unwrap()) as i64 + delta) as u32;
            b[128..132].copy_from_slice(&size.to_le_bytes());
            return;
        }
        p += n;
    }
    panic!("chunk missing");
}
#[test]
fn sparse_tileset_ids_are_not_layer_indices() {
    let d = tile_doc();
    let mut b = encode_ase(&d).unwrap();
    mutate_chunk(&mut b, 0x2023, |c| {
        c[..4].copy_from_slice(&42u32.to_le_bytes())
    });
    assert_eq!(decode_ase(&b).unwrap().composite(0), d.composite(0));
}
#[test]
fn negative_subtile_origin_survives_save() {
    let mut b = encode_ase(&tile_doc()).unwrap();
    let mut p = 144;
    loop {
        let n = u32::from_le_bytes(b[p..p + 4].try_into().unwrap()) as usize;
        if b[p + 4..p + 6] == 0x2005u16.to_le_bytes()
            && u16::from_le_bytes(b[p + 6..p + 8].try_into().unwrap()) == 1
        {
            break;
        }
        p += n;
    }
    b[p + 8..p + 10].copy_from_slice(&(-1i16).to_le_bytes());
    let d = decode_ase(&b).unwrap();
    assert_eq!(d.composite(0)[0], [0, 255, 0, 255]);
    assert_eq!(
        decode_ase(&encode_ase(&d).unwrap()).unwrap().composite(0),
        d.composite(0)
    );
}

#[test]
fn ase_reference_cels_use_precise_bounds_without_exporting_as_artwork() {
    let mut source = Document::new(6, 3);
    let red = [210, 20, 30, 255];
    let green = [35, 190, 20, 255];
    source.layers[0].ase_flags |= 64;
    source.layers[0].cel_meta[0].bounds = Some([-1, 0, 2, 1]);
    source.layers[0].cel_meta[0].outside.push(OutsidePixel {
        x: -1,
        y: 0,
        color: red,
        palette_slot: None,
    });
    source.layers[0].cels[0].set(0, green);
    let mut data = 1u32.to_le_bytes().to_vec();
    for v in [81_920i32, 49_152, 4 * 65_536, 98_304] {
        data.extend(v.to_le_bytes()); // Destination (1.25, .75), size (4, 1.5).
    }
    data.extend([0; 16]);
    source.metadata.push(MetadataChunk {
        frame: 0,
        kind: 0x2006,
        data,
        owner: "cel:0".into(),
    });
    source.add_frame(0, true);
    source.layers[0].cel_meta[0].artwork_link = Some(0);
    source.layers[0].cel_meta[1].artwork_link = Some(0);
    let mut second = source.metadata[0].clone();
    second.frame = 1;
    second.data[4..8].copy_from_slice(&(-16_384i32).to_le_bytes());
    source.metadata.retain(|chunk| chunk.frame != 1);
    source.metadata.push(second);

    let loaded = decode_ase(&encode_ase(&source).unwrap()).unwrap();
    let legacy = decode_project(&encode_project(&source).unwrap()).unwrap();
    let native = decode_project(&encode_project(&loaded).unwrap()).unwrap();
    let ase = decode_ase(&encode_ase(&loaded).unwrap()).unwrap();
    for d in [loaded, legacy, native, ase] {
        let bounds = [81_920, 49_152, 262_144, 98_304];
        assert_eq!(d.layers[0].cel_meta[0].reference_bounds, Some(bounds));
        assert_eq!(
            d.layers[0].cel_meta[1].reference_bounds.unwrap()[0],
            -16_384
        );
        assert!(d.layers[0].cel_meta[1].artwork_link.is_some());
        let sample = |x, y| pixelcraft::productivity::sample_canvas(&d, 0, x, y, true);
        assert_eq!(sample(1, 1), red);
        assert_eq!(sample(2, 1), red);
        assert_eq!(sample(3, 1), green);
        assert_eq!(sample(4, 1), green);
        for (x, y) in [(0, 1), (5, 1), (2, 0), (2, 2)] {
            assert_eq!(sample(x, y), CLEAR);
        }
        assert_eq!(d.composite(0), vec![CLEAR; 18]);
        assert_eq!(
            d.composite_view(1)[6..12],
            [red, red, green, green, CLEAR, CLEAR]
        );
        let mut moved = d.clone();
        moved.layers[0].cel_meta[0]
            .reference_bounds
            .as_mut()
            .unwrap()[0] += 65_536;
        assert!(!d.same_frame_pixels(&moved, 0));
        assert!(d.same_frame_pixels(&moved, 1));
        assert_eq!(
            moved.composite_view(0)[6..12],
            [CLEAR, CLEAR, red, red, green, green]
        );
    }
}

#[test]
fn ase_reference_rotation_and_flip_transform_both_geometry_and_source_pixels() {
    use pixelcraft::workflows::{Orientation, orient_sprite};

    let mut source = Document::new(6, 3);
    let red = [210, 20, 30, 255];
    let green = [35, 190, 20, 255];
    source.layers[0].ase_flags |= 64;
    let meta = &mut source.layers[0].cel_meta[0];
    meta.bounds = Some([-1, 0, 2, 1]);
    meta.reference_bounds = Some([81_920, 49_152, 262_144, 98_304]);
    meta.outside.push(OutsidePixel {
        x: -1,
        y: 0,
        color: red,
        palette_slot: None,
    });
    source.layers[0].cels[0].set(0, green);
    let source = decode_ase(&encode_ase(&source).unwrap()).unwrap();
    let original = source.composite_view(0);
    let original_id = source.layers[0].cel_meta[0].image_id.runtime_key();
    for (orientation, bounds, preserves_image) in [
        (
            Orientation::TurnRight,
            [49_152, 81_920, 98_304, 262_144],
            false,
        ),
        (
            Orientation::TurnLeft,
            [49_152, 49_152, 98_304, 262_144],
            false,
        ),
        (
            Orientation::TurnHalf,
            [49_152, 49_152, 262_144, 98_304],
            false,
        ),
        (
            Orientation::FlipHorizontal,
            [49_152, 49_152, 262_144, 98_304],
            true,
        ),
        (
            Orientation::FlipVertical,
            [81_920, 49_152, 262_144, 98_304],
            true,
        ),
    ] {
        let transformed = orient_sprite(&source, orientation).unwrap();
        let meta = &transformed.layers[0].cel_meta[0];
        assert_eq!(meta.reference_bounds, Some(bounds));
        assert_eq!(meta.image_id.runtime_key() == original_id, preserves_image);
        let reopened = decode_ase(&encode_ase(&transformed).unwrap()).unwrap();
        assert_eq!(
            reopened.layers[0].cel_meta[0].reference_bounds,
            Some(bounds)
        );
        let view = transformed.composite_view(0);
        assert_eq!(reopened.composite_view(0), view);
        for y in 0..source.height {
            for x in 0..source.width {
                let (tx, ty) = match orientation {
                    Orientation::TurnRight => (source.height - y - 1, x),
                    Orientation::TurnLeft => (y, source.width - x - 1),
                    Orientation::TurnHalf => (source.width - x - 1, source.height - y - 1),
                    Orientation::FlipHorizontal => (source.width - x - 1, y),
                    Orientation::FlipVertical => (x, source.height - y - 1),
                };
                assert_eq!(
                    view[ty * transformed.width + tx],
                    original[y * source.width + x]
                );
            }
        }
        assert_eq!(transformed.composite(0), vec![CLEAR; view.len()]);
    }
}

#[test]
fn ase_reference_crop_and_resample_preserve_source_artwork() {
    use pixelcraft::workflows::{crop, resample_sprite, resize_canvas};

    let mut source = Document::new(6, 3);
    let red = [210, 20, 30, 255];
    let green = [35, 190, 20, 255];
    // Put one source pixel beyond the resized canvas and enlarge it in the view.
    source.layers[0].ase_flags |= 64;
    let meta = &mut source.layers[0].cel_meta[0];
    meta.bounds = Some([3, 0, 2, 1]);
    meta.reference_bounds = Some([81_920, 49_152, 262_144, 98_304]);
    source.layers[0].cels[0].set(3, red);
    source.layers[0].cels[0].set(4, green);
    let source = decode_ase(&encode_ase(&source).unwrap()).unwrap();
    let id = source.layers[0].cel_meta[0].image_id.runtime_key();

    let verify = |doc: &Document, bounds, rendered: &[Pixel]| {
        let meta = &doc.layers[0].cel_meta[0];
        assert_eq!(meta.reference_bounds, Some(bounds));
        assert_eq!(meta.bounds, Some([3, 0, 2, 1]));
        assert_eq!(meta.image_id.runtime_key(), id);
        assert_eq!(doc.composite_view(0), rendered);
        for reopened in [
            decode_ase(&encode_ase(doc).unwrap()).unwrap(),
            decode_project(&encode_project(doc).unwrap()).unwrap(),
        ] {
            assert_eq!(reopened.composite_view(0), rendered);
            assert_eq!(reopened.layers[0].cel_meta[0].bounds, Some([3, 0, 2, 1]));
        }
    };

    let mut cropped = source.clone();
    crop(&mut cropped, [1, 1, 3, 2]).unwrap();
    let mut resized = source.clone();
    resize_canvas(&mut resized, [3, 2], [-1, -1]).unwrap();
    let cropped_view = [red, red, green, CLEAR, CLEAR, CLEAR];
    for mut doc in [cropped, resized] {
        verify(&doc, [16_384, -16_384, 262_144, 98_304], &cropped_view);
        assert_eq!(doc.layers[0].cel_meta[0].outside.len(), 2);
        resize_canvas(&mut doc, [6, 3], [1, 1]).unwrap();
        assert!(doc.layers[0].cel_meta[0].outside.is_empty());
        verify(
            &doc,
            [81_920, 49_152, 262_144, 98_304],
            &source.composite_view(0),
        );
    }

    let sampled = resample_sprite(&source, [3, 6]).unwrap();
    let mut sampled_view = vec![CLEAR; 18];
    for row in 1..4 {
        sampled_view[row * 3 + 1] = red;
        sampled_view[row * 3 + 2] = green;
    }
    verify(&sampled, [40_960, 98_304, 131_072, 196_608], &sampled_view);
}

#[test]
fn precise_reference_uses_its_source_mask_when_canvas_resizes() {
    use pixelcraft::workflows::{crop, resample_sprite};

    let mut doc = Document::new(6, 2);
    let ink = [210, 61, 35, 255];
    doc.layers[0].ase_flags |= 64;
    let cel = &mut doc.layers[0].cel_meta[0];
    cel.bounds = Some([4, 0, 1, 1]);
    cel.reference_bounds = Some([2 * 65_536, 0, 2 * 65_536, 65_536]);
    doc.layers[0].cels[0].set(4, ink);
    let mut mask = Pixels::empty(12);
    mask.set(4, [0, 0, 0, 127]);
    doc.set_layer_mask(0, Some(mask)).unwrap();
    let expected = doc.composite_view(0)[2];
    assert_eq!(expected, [210, 61, 35, 127]);
    assert_eq!(doc.sample_composite(0, 2, 0, true), Some(expected));
    assert_eq!(
        doc.sample_composite(0, 2, 0, false),
        Some(doc.composite(0)[2])
    );
    assert_eq!(doc.sample_composite(0, -1, 0, true), None);

    let sampled = resample_sprite(&doc, [3, 2]).unwrap();
    assert_eq!(sampled.composite_view(0)[1], expected);
    crop(&mut doc, [1, 0, 3, 2]).unwrap();
    assert_eq!(doc.composite_view(0)[1], expected);
    assert_eq!(
        doc.composite_view_region(0, [1, 0], [1, 1]).unwrap(),
        [expected]
    );
}

#[test]
fn compact_tile_encodings_and_legacy_empty_ids() {
    use std::io::Write;
    fn z(bytes: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }
    // Remove the image layer so the first cel is the tilemap under test.
    let mut d = tile_doc();
    d.remove_layer(0);
    for bits in [8u16, 16, 32] {
        let mut b = encode_ase(&d).unwrap();
        mutate_chunk(&mut b, 0x2005, |c| {
            c[20..22].copy_from_slice(&bits.to_le_bytes());
            let mask = (1u32 << (bits - 3)) - 1;
            for (offset, v) in [
                (22, mask),
                (26, 1 << (bits - 1)),
                (30, 1 << (bits - 2)),
                (34, 1 << (bits - 3)),
            ] {
                c[offset..offset + 4].copy_from_slice(&v.to_le_bytes());
            }
            let mut bytes = vec![];
            for v in &d.layers[0].tile_cels[0] {
                bytes.extend(&v.to_le_bytes()[..bits as usize / 8]);
            }
            c.truncate(48);
            c.extend(z(&bytes));
        });
        let decoded = decode_ase(&b).unwrap();
        assert_eq!(decoded.composite(0), d.composite(0));
        assert_eq!(
            decode_ase(&encode_ase(&decoded).unwrap())
                .unwrap()
                .composite(0),
            d.composite(0)
        );
    }
    let mut b = encode_ase(&d).unwrap();
    mutate_chunk(&mut b, 0x2023, |c| {
        c[4..8].copy_from_slice(&2u32.to_le_bytes());
        c[8..12].copy_from_slice(&1u32.to_le_bytes());
        let name = u16::from_le_bytes(c[32..34].try_into().unwrap()) as usize;
        let end = 34 + name;
        c.truncate(end);
        let bytes: Vec<u8> = d.tilesets[0].tiles[0].iter().flatten().copied().collect();
        let compressed = z(&bytes);
        c.extend((compressed.len() as u32).to_le_bytes());
        c.extend(compressed);
    });
    mutate_chunk(&mut b, 0x2005, |c| {
        c.truncate(48);
        c.extend(z(&[0u32.to_le_bytes(), u32::MAX.to_le_bytes()].concat()));
    });
    let decoded = decode_ase(&b).unwrap();
    assert_eq!(decoded.composite(0), d.composite(0));
}
#[test]
fn external_tilesets_resolve_relative_to_file_and_embed_on_save() {
    let mut d = tile_doc();
    d.ase_tilesets.push(AseTilesetInfo {
        index: 0,
        id: 42,
        flags: 7,
        base_index: 1,
        external: Some([7, 42]),
    });
    let mut entry = vec![];
    entry.extend(1u32.to_le_bytes());
    entry.extend([0; 8]);
    entry.extend(7u32.to_le_bytes());
    entry.push(1);
    entry.extend([0; 7]);
    entry.extend(14u16.to_le_bytes());
    entry.extend(b"tiles.aseprite");
    d.metadata.push(MetadataChunk {
        frame: 0,
        kind: 0x2008,
        owner: String::new(),
        data: entry,
    });
    let mut bytes = encode_ase(&d).unwrap();
    mutate_chunk(&mut bytes, 0x2023, |c| {
        c[4..8].copy_from_slice(&5u32.to_le_bytes());
        let n = u16::from_le_bytes(c[32..34].try_into().unwrap()) as usize;
        c.truncate(34 + n + 8);
    });
    assert!(decode_ase(&bytes).unwrap_err().contains("resolver"));
    let mut calls = 0;
    let decoded = decode_ase_with_resolver(&bytes, &mut |file, id| {
        assert_eq!(file, "tiles.aseprite");
        assert_eq!(id, 42);
        calls += 1;
        Ok(d.tilesets[0].clone())
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_ne!(
        decoded.tilesets[0].tiles[0].editor_id.runtime_key(),
        d.tilesets[0].tiles[0].editor_id.runtime_key(),
        "external definitions need fresh document-local editing identities"
    );
    assert_eq!(decoded.composite(0), d.composite(0));
    let root = std::env::temp_dir().join(format!("pixelcraft-external-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    save(&root.join("tiles.aseprite"), &d).unwrap();
    std::fs::write(root.join("main.aseprite"), &bytes).unwrap();
    assert_eq!(
        load(&root.join("main.aseprite")).unwrap().composite(0),
        d.composite(0)
    );
    let standalone = encode_ase(&decoded).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(
        decode_ase(&standalone).unwrap().composite(0),
        d.composite(0)
    );
}
