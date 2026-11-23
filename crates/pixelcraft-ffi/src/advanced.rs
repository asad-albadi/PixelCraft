//! Narrow mobile adapters for shared tile conversion/painting and embedded references.
use super::*;
use pixelcraft_core::{
    model::LayerConversion,
    productivity::{ReferenceImage, ReferenceOptions},
};

fn integer(value: &Value, name: &str, max: usize) -> Result<usize, String> {
    value[name]
        .as_u64()
        .filter(|v| *v <= max as u64)
        .map(|v| v as usize)
        .ok_or_else(|| format!("Invalid {name}"))
}
fn reserve(doc: &Document, bytes: usize) -> Result<(), String> {
    if doc.stored_pixels().saturating_mul(4).saturating_add(bytes) > RASTER_BUDGET {
        return Err("Action exceeds the tablet raster budget".into());
    }
    Ok(())
}
fn reference_index(doc: &Document, value: &Value) -> Result<usize, String> {
    let index = integer(value, "index", 3)?;
    let reference = doc.references.get(index).ok_or("Unknown reference")?;
    if reference.options.locked {
        return Err("Reference is locked".into());
    }
    Ok(index)
}
fn number(value: &Value, name: &str, min: f64, max: f64) -> Result<f32, String> {
    value[name]
        .as_f64()
        .filter(|v| v.is_finite() && *v >= min && *v <= max)
        .map(|v| v as f32)
        .ok_or_else(|| format!("Invalid {name}"))
}
fn flag(value: &Value, name: &str) -> Result<bool, String> {
    match value.get(name) {
        None => Ok(false),
        Some(value) => value.as_bool().ok_or_else(|| format!("Invalid {name}")),
    }
}

pub(super) fn apply(session: &mut MobileSession, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 16 * 1024 {
        return Err("Advanced action exceeds metadata budget".into());
    }
    let request: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if request["revision"].as_u64() != Some(session.revision.into()) {
        return Err("Artwork changed; retry action".into());
    }
    let mut doc = session.editor.doc.clone();
    let mut layer = None;
    match request["action"]
        .as_str()
        .ok_or("Missing advanced action")?
    {
        "create-tileset" => {
            if doc.layers.len() >= 32 || doc.tilesets.len() >= 8 {
                return Err("Tile layer or tileset limit reached".into());
            }
            let hide_source = flag(&request, "hide_source")?;
            let width = integer(&request, "tile_width", 64)?;
            let height = integer(&request, "tile_height", 64)?;
            if width == 0 || height == 0 {
                return Err("Tile dimensions must be 1 to 64".into());
            }
            let cells = doc.width.div_ceil(width) * doc.height.div_ceil(height);
            if cells > 256 {
                return Err("Choose at most 256 source grid cells".into());
            }
            reserve(
                &doc,
                cells * width * height * 16
                    + cells * doc.durations.len() * 4
                    + doc.width * doc.height * 4,
            )?;
            let mut source = Document::new(doc.width, doc.height);
            source.layers[0].cels[0] = doc.composite(session.editor.frame).into();
            source.convert_layer(0, LayerConversion::Tilemap { width, height })?;
            let set = source
                .tilesets
                .pop()
                .ok_or("Conversion produced no tileset")?;
            let set_index = doc.tilesets.len();
            doc.tilesets.push(set);
            if hide_source {
                for layer in &mut doc.layers {
                    layer.visible = false;
                }
            }
            doc.add_tile_layer(set_index)?;
            let target = doc.layers.last_mut().ok_or("Missing tile layer")?;
            target.tile_cels[session.editor.frame] = source.layers[0].tile_cels[0].clone();
            target.cel_meta[session.editor.frame].tilemap =
                source.layers[0].cel_meta[0].tilemap.clone();
            layer = Some(doc.layers.len() - 1);
        }
        "paint-tile" => {
            let x = integer(&request, "x", doc.width - 1)?;
            let y = integer(&request, "y", doc.height - 1)?;
            let tile = integer(&request, "tile", 256)? as u32;
            let (_, column, row) = doc
                .tile_at_point(
                    session.editor.layer,
                    session.editor.frame,
                    (x as i32, y as i32),
                )
                .ok_or("Choose a tile layer and canvas position")?;
            let bits = (u32::from(flag(&request, "flip_x")?) << 31)
                | (u32::from(flag(&request, "flip_y")?) << 30)
                | (u32::from(flag(&request, "flip_diagonal")?) << 29);
            reserve(
                &doc,
                doc.layers[session.editor.layer]
                    .tile_cels
                    .iter()
                    .map(|c| c.len() * 4)
                    .sum(),
            )?;
            doc.paint_tile(
                session.editor.layer,
                session.editor.frame,
                column,
                row,
                tile | bits,
            )?;
        }
        "reference-set" => {
            let index = reference_index(&doc, &request)?;
            let x = number(&request, "x", -4096., 4096.)?;
            let y = number(&request, "y", -4096., 4096.)?;
            let scale = number(&request, "scale", 0.01, 16.)?;
            let opacity = integer(&request, "opacity", 255)? as u8;
            let visible = request["visible"].as_bool().ok_or("Invalid visible flag")?;
            let reference = &mut doc.references[index];
            reference.x = x;
            reference.y = y;
            reference.scale = scale;
            if let Some(scale_y) = request.get("scale_y") {
                reference.options.scale_y = if scale_y.is_null() {
                    None
                } else {
                    Some(number(&request, "scale_y", 0.01, 16.)?)
                };
            }
            reference.opacity = opacity;
            reference.visible = visible;
        }
        "reference-remove" => {
            let index = reference_index(&doc, &request)?;
            doc.references.remove(index);
        }
        "reference-to-layer" => {
            let index = reference_index(&doc, &request)?;
            if doc.layers.len() >= 32 {
                return Err("Layer limit reached".into());
            }
            reserve(&doc, doc.width * doc.height * 8)?;
            let reference = doc.references[index].clone();
            pixelcraft_core::productivity::reference_to_layer(
                &mut doc,
                &reference,
                session.editor.frame,
            )?;
            layer = Some(doc.layers.len() - 1);
        }
        _ => return Err("Unknown advanced action".into()),
    }
    session.commit(doc, "Advanced edit")?;
    if let Some(layer) = layer {
        session.editor.layer = layer;
    }
    Ok(())
}

pub(super) fn import_reference(
    session: &mut MobileSession,
    revision: u32,
    bytes: &[u8],
) -> Result<(), String> {
    if revision != session.revision {
        return Err("Artwork changed; retry reference import".into());
    }
    if session.editor.doc.references.len() >= 4 {
        return Err("Choose at most four embedded references".into());
    }
    let source = formats::decode_mobile_format(1, bytes)?;
    reserve(&session.editor.doc, source.width * source.height * 4)?;
    let reference = ReferenceImage {
        behind: false,
        floating: false,
        name: format!("Reference {}", session.editor.doc.references.len() + 1),
        width: source.width,
        height: source.height,
        pixels: source.composite(0).into(),
        x: 0.,
        y: 0.,
        scale: 1.,
        opacity: 255,
        visible: true,
        options: ReferenceOptions::default(),
    };
    let mut doc = session.editor.doc.clone();
    doc.references.push(reference);
    session.commit(doc, "Add reference")
}

pub(super) fn preview(session: &MobileSession, index: u32) -> Result<Vec<u8>, String> {
    let reference = session
        .editor
        .doc
        .references
        .get(index as usize)
        .ok_or("Unknown reference")?;
    Ok(reference.pixels.iter().flatten().copied().collect())
}

pub(super) fn check(doc: &Document) -> Result<(), String> {
    if doc.references.len() > 4
        || doc.tilesets.len() > 8
        || doc.tilesets.iter().any(|set| {
            set.width == 0
                || set.height == 0
                || set.width > 64
                || set.height > 64
                || set.tiles.len() > 256
        })
    {
        return Err("Document exceeds mobile reference or tileset limits".into());
    }
    if doc.references.iter().any(|r| {
        r.width > MAX_SIDE
            || r.height > MAX_SIDE
            || r.name.len() > 256
            || !r.x.is_finite()
            || !r.y.is_finite()
            || r.x.abs() > 4096.
            || r.y.abs() > 4096.
            || !r.scale.is_finite()
            || r.scale < 0.01
            || r.scale > 16.
            || r.options
                .scale_y
                .is_some_and(|s| !s.is_finite() || !(0.01..=16.).contains(&s))
            || r.options.source.is_some()
    }) {
        return Err(
            "Reference exceeds mobile dimensions or transform limits; use embedded images".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session(doc: Document) -> MobileSession {
        MobileSession {
            editor: EditorSession::new(doc),
            stroke: None,
            revision: 1,
            raster_pixels: RASTER_BUDGET / 4,
            clipboard: None,
            prepared_import: None,
        }
    }
    fn act(session: &mut MobileSession, request: Value) -> Result<(), String> {
        apply(session, &serde_json::to_vec(&request).unwrap())
    }
    #[test]
    fn reference_visibility_preserves_nonuniform_geometry_unless_explicitly_changed() {
        let mut session = session(Document::new(8, 8));
        let png = formats::encode_mobile_format(1, &Document::new(2, 2), 0).unwrap();
        import_reference(&mut session, 1, &png).unwrap();
        session.editor.doc.references[0].options.scale_y = Some(3.);
        session.editor.doc.references[0].behind = true;
        session.editor.doc.references[0].floating = true;
        let revision = session.revision;
        act(&mut session,json!({"revision":revision,"action":"reference-set","index":0,"x":0,"y":0,"scale":1,"opacity":255,"visible":false})).unwrap();
        assert_eq!(session.editor.doc.references[0].options.scale_y, Some(3.));
        assert!(
            session.editor.doc.references[0].behind && session.editor.doc.references[0].floating
        );
        let revision = session.revision;
        let before = session.editor.doc.clone();
        for scale in [json!(0), json!(17), json!("NaN")] {
            assert!(act(&mut session,json!({"revision":revision,"action":"reference-set","index":0,"x":0,"y":0,"scale":1,"scale_y":scale,"opacity":255,"visible":true})).is_err());
            assert_eq!(session.editor.doc, before);
        }
        act(&mut session,json!({"revision":revision,"action":"reference-set","index":0,"x":0,"y":0,"scale":1,"scale_y":2,"opacity":255,"visible":true})).unwrap();
        assert_eq!(session.editor.doc.references[0].options.scale_y, Some(2.));
        let revision = session.revision;
        act(&mut session,json!({"revision":revision,"action":"reference-set","index":0,"x":0,"y":0,"scale":1,"scale_y":null,"opacity":255,"visible":true})).unwrap();
        assert_eq!(session.editor.doc.references[0].options.scale_y, None);
    }

    #[test]
    fn hiding_tile_sources_is_one_undo_step() {
        let mut session = session(Document::new(8, 8));
        act(&mut session,json!({"revision":1,"action":"create-tileset","tile_width":4,"tile_height":4,"hide_source":true})).unwrap();
        assert!(!session.editor.doc.layers[0].visible);
        assert!(session.editor.doc.layers[1].visible);
        assert_eq!(session.editor.undo.len(), 1);
        session.editor.jump_history(0);
        assert_eq!(session.editor.doc.layers.len(), 1);
        assert!(session.editor.doc.layers[0].visible);
    }

    #[test]
    fn locked_tile_and_reference_targets_reject_without_history_or_revision() {
        let mut session = session(Document::new(8, 8));
        act(
            &mut session,
            json!({"revision":1,"action":"create-tileset","tile_width":4,"tile_height":4}),
        )
        .unwrap();
        session.editor.doc.layers[1].locked = true;
        let before = session.editor.doc.clone();
        let revision = session.revision;
        let history = session.editor.undo.len();
        assert!(
            act(
                &mut session,
                json!({"revision":revision,"action":"paint-tile","x":0,"y":0,"tile":0})
            )
            .is_err()
        );
        assert_eq!(session.editor.doc, before);
        assert_eq!(session.revision, revision);
        assert_eq!(session.editor.undo.len(), history);
        let png = formats::encode_mobile_format(1, &Document::new(2, 2), 0).unwrap();
        import_reference(&mut session, revision, &png).unwrap();
        session.editor.doc.references[0].options.locked = true;
        let before = session.editor.doc.clone();
        let revision = session.revision;
        let history = session.editor.undo.len();
        for action in ["reference-remove", "reference-to-layer", "reference-set"] {
            assert!(act(&mut session,json!({"revision":revision,"action":action,"index":0,"x":0,"y":0,"scale":1,"opacity":255,"visible":true})).unwrap_err().contains("locked"));
            assert_eq!(session.editor.doc, before);
            assert_eq!(session.revision, revision);
            assert_eq!(session.editor.undo.len(), history);
        }
    }
    #[test]
    fn invalid_transforms_tile_bounds_and_reference_limit_are_atomic() {
        let mut session = session(Document::new(8, 8));
        let before = session.editor.doc.clone();
        for request in [
            json!({"revision":1,"action":"create-tileset","tile_width":0,"tile_height":4}),
            json!({"revision":1,"action":"create-tileset","tile_width":65,"tile_height":4}),
            json!({"revision":0,"action":"create-tileset","tile_width":4,"tile_height":4}),
        ] {
            assert!(act(&mut session, request).is_err());
            assert_eq!(session.editor.doc, before);
            assert_eq!(session.revision, 1);
        }
        act(
            &mut session,
            json!({"revision":1,"action":"create-tileset","tile_width":4,"tile_height":4}),
        )
        .unwrap();
        let before = session.editor.doc.clone();
        let revision = session.revision;
        for request in [
            json!({"revision":revision,"action":"paint-tile","x":8,"y":0,"tile":0}),
            json!({"revision":revision,"action":"paint-tile","x":0,"y":0,"tile":257}),
            json!({"revision":revision,"action":"paint-tile","x":0,"y":0,"tile":1}),
            json!({"revision":revision,"action":"paint-tile","x":0,"y":0,"tile":0,"flip_x":"true"}),
        ] {
            assert!(act(&mut session, request).is_err());
            assert_eq!(session.editor.doc, before);
            assert_eq!(session.revision, revision);
        }
        let png = formats::encode_mobile_format(1, &Document::new(2, 2), 0).unwrap();
        for _ in 0..4 {
            let revision = session.revision;
            import_reference(&mut session, revision, &png).unwrap();
        }
        let before = session.editor.doc.clone();
        let revision = session.revision;
        assert!(import_reference(&mut session, revision, &png).is_err());
        assert!(import_reference(&mut session, revision - 1, &png).is_err());
        for (x, scale) in [(4097., 1.), (0., 0.), (0., 17.)] {
            assert!(act(&mut session,json!({"revision":revision,"action":"reference-set","index":0,"x":x,"y":0,"scale":scale,"opacity":255,"visible":true})).is_err());
        }
        assert_eq!(session.editor.doc, before);
        assert_eq!(session.revision, revision);
    }
    #[test]
    fn indexed_tile_and_reference_documents_roundtrip_in_native_format() {
        let mut doc = Document::new(8, 8);
        doc.palette = vec![[0, 0, 0, 0], [255, 0, 0, 255]];
        doc.color_mode = pixelcraft_core::model::ColorMode::Indexed;
        doc.layers[0].cels[0] = vec![[255, 0, 0, 255]; 64].into();
        doc.layers[0].cel_meta[0].indices = Some(vec![1; 64].into());
        let mut session = session(doc);
        act(
            &mut session,
            json!({"revision":1,"action":"create-tileset","tile_width":4,"tile_height":4}),
        )
        .unwrap();
        let png = formats::encode_mobile_format(1, &session.editor.doc, 0).unwrap();
        let revision = session.revision;
        import_reference(&mut session, revision, &png).unwrap();
        let encoded = encode_mobile(&session.editor.doc).unwrap();
        let reopened = document(6, 0, 0, &encoded).unwrap();
        assert_eq!(reopened, session.editor.doc);
        assert_eq!(
            reopened.color_mode,
            pixelcraft_core::model::ColorMode::Indexed
        );
        assert_eq!(
            reopened.layers[0].cel_meta[0].indices.as_deref().unwrap(),
            &vec![1; 64]
        );
        assert_eq!(reopened.references.len(), 1);
        assert_eq!(reopened.tilesets.len(), 1);
    }
}
