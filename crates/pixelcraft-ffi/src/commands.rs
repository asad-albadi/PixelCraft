//! Mobile allowlist over the same validated commands used by desktop automation.
use super::*;
pub(super) fn apply(session: &mut MobileSession, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 64 * 1024 {
        return Err("Command packet exceeds the mobile budget".into());
    }
    let packet: Value = serde_json::from_slice(bytes).map_err(|_| "Invalid command packet")?;
    if packet["revision"].as_u64() != Some(u64::from(session.revision)) {
        return Err("Artwork changed; retry the action".into());
    }
    let commands = packet["commands"].as_array().ok_or("Expected commands")?;
    if commands.is_empty() || commands.len() > 32 {
        return Err("Choose 1 to 32 editing commands".into());
    }
    if commands.len() == 1 && commands[0]["op"] == "palette-entry" {
        let c = &commands[0];
        let name = c["name"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 128)
            .ok_or("Choose a short color name")?;
        let color: [u8; 4] =
            serde_json::from_value(c["color"].clone()).map_err(|_| "Invalid RGBA color")?;
        let mut doc = session.editor.doc.clone();
        let index = doc
            .palette_names
            .iter()
            .take(doc.palette.len())
            .position(|n| n == name)
            .or_else(|| doc.palette.iter().position(|p| *p == color));
        let index = match index {
            Some(i) => i,
            None => {
                if doc.palette.len() >= 256 {
                    return Err("Palette is full; rename or edit an existing color".into());
                }
                doc.palette.push(color);
                doc.palette.len() - 1
            }
        };
        doc.set_palette_color(index, color);
        doc.palette_names.resize(doc.palette.len(), String::new());
        doc.palette_names[index] = name.to_string();
        return session.commit(doc, "Palette");
    }
    let (mut layers, mut frames) = (
        session.editor.doc.layers.len(),
        session.editor.doc.durations.len(),
    );
    for c in commands {
        match c["op"].as_str().ok_or("Missing edit operation")? {
            "add-layer" | "add-group" => layers += 1,
            "add-frame" => frames += 1,
            "remove-layer" => layers = layers.saturating_sub(1),
            "remove-frame" => frames = frames.saturating_sub(1),
            "convert-rgb" | "convert-grayscale" | "convert-indexed" => {
                let conversion_bytes = session
                    .editor
                    .doc
                    .width
                    .saturating_mul(session.editor.doc.height)
                    .saturating_mul(layers)
                    .saturating_mul(frames)
                    .saturating_mul(8);
                if conversion_bytes > RASTER_BUDGET {
                    return Err("Color conversion exceeds the mobile allocation budget".into());
                }
            }
            "set-layer" | "move-layer" | "set-frame" | "link-cel" | "unlink-cel"
            | "clear-cel-range" | "set-range-duration" | "add-tag" | "set-tag" | "remove-tag"
            | "shape" | "fill" | "stroke" | "clear-region" | "copy-region" | "move-region"
            | "recolor-region" | "edit-tile-pixel" => {}
            _ => return Err("This editing command is not exposed on Android".into()),
        }
        if layers > 32 || frames > 64 {
            return Err("Mobile layer/frame limit reached".into());
        }
    }
    let paints = commands.iter().any(|c| {
        matches!(
            c["op"].as_str(),
            Some(
                "shape"
                    | "fill"
                    | "stroke"
                    | "clear-region"
                    | "copy-region"
                    | "move-region"
                    | "recolor-region"
            )
        )
    });
    if session.editor.selection.is_some()
        && paints
        && commands.iter().any(|c| {
            !matches!(
                c["op"].as_str(),
                Some(
                    "shape"
                        | "fill"
                        | "stroke"
                        | "clear-region"
                        | "copy-region"
                        | "move-region"
                        | "recolor-region"
                )
            ) || c["layer"].as_u64() != Some(session.editor.layer as u64)
                || c["frame"].as_u64() != Some(session.editor.frame as u64)
                || c.get("layer_id").is_some()
                || c.get("cel_id").is_some()
                || c["from_layer"]
                    .as_u64()
                    .is_some_and(|l| l != session.editor.layer as u64)
                || c["from_frame"]
                    .as_u64()
                    .is_some_and(|f| f != session.editor.frame as u64)
        })
    {
        return Err("Paint the active selection target separately, or deselect first".into());
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut next = session.editor.doc.clone();
    for c in commands {
        if std::time::Instant::now() >= deadline {
            return Err("Editing action expired without applying changes".into());
        }
        if c["op"] == "fill" && session.editor.selection.is_some() {
            use pixelcraft_core::productivity::{FloodOptions, flood_raster};
            let (l, f) = (session.editor.layer, session.editor.frame);
            if !next.editable(l) || next.layers[l].group || next.layers[l].tileset.is_some() {
                return Err("Choose an unlocked image layer".into());
            }
            let (x, y) = (
                c["x"].as_u64().ok_or("Invalid fill seed")?,
                c["y"].as_u64().ok_or("Invalid fill seed")?,
            );
            if x >= next.width as u64 || y >= next.height as u64 {
                return Err("Fill seed outside canvas".into());
            }
            let color: [u8; 4] =
                serde_json::from_value(c["color"].clone()).map_err(|_| "Invalid fill color")?;
            let tolerance = c
                .get("tolerance")
                .map(|v| serde_json::from_value::<u8>(v.clone()))
                .transpose()
                .map_err(|_| "Invalid fill tolerance")?
                .unwrap_or(0);
            let contiguous = c
                .get("contiguous")
                .map(|v| serde_json::from_value::<bool>(v.clone()))
                .transpose()
                .map_err(|_| "Invalid contiguous option")?
                .unwrap_or(true);
            let mask = selection::brush_mask(&session.editor).unwrap();
            let hits = flood_raster(
                &next,
                (l, f),
                (x as i32, y as i32),
                (tolerance, contiguous),
                next.wraparound.into(),
                FloodOptions::default(),
                |i| mask[i],
            );
            let color = next.snapped_color(f, color);
            for (i, hit) in hits.into_iter().enumerate() {
                if hit {
                    next.set_pixel(l, f, i, color);
                }
            }
        } else {
            next = pixelcraft_automation::apply_command_batch_until(
                &next,
                &json!({"commands":[c]}),
                Some(deadline),
            )?;
        }
        if paints && session.editor.selection.is_some() {
            selection::clip_paint(&session.editor, &mut next);
        }
    }
    session.commit(next, "Edit")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mobile(doc: Document) -> MobileSession {
        MobileSession {
            editor: EditorSession::new(doc),
            stroke: None,
            revision: 1,
            raster_pixels: RASTER_BUDGET / 4,
            clipboard: None,
        }
    }
    #[test]
    fn sparse_color_conversion_is_rejected_before_dense_expansion() {
        let mut doc = Document::new(256, 256);
        for f in 0..32 {
            doc.set_pixel(0, f, 0, [255; 4]);
            doc.add_frame(f, false);
        }
        let mut s = mobile(doc);
        let original = s.editor.doc.clone();
        let error=apply(&mut s, br#"{"revision":1,"commands":[{"op":"convert-indexed","palette":[[0,0,0,0],[255,255,255,255]]}]}"#).unwrap_err();
        assert!(error.contains("allocation budget"));
        assert_eq!(s.editor.doc, original);
        assert_eq!(s.revision, 1);
    }
    #[test]
    fn remove_then_add_at_layer_limit_is_valid() {
        let mut doc = Document::new(4, 4);
        for _ in 1..32 {
            doc.add_layer();
        }
        let mut s = mobile(doc);
        apply(&mut s,br#"{"revision":1,"commands":[{"op":"remove-layer","layer":31},{"op":"add-layer","name":"Replacement"}]}"#).unwrap();
        assert_eq!(s.editor.doc.layers.len(), 32);
        assert_eq!(s.editor.doc.layers[31].name, "Replacement");
    }
    #[test]
    fn selection_rejects_alternate_targets_sources_and_mixed_layout_atomically() {
        let mut doc = Document::new(8, 8);
        doc.add_layer();
        doc.set_pixel(1, 0, 0, [10, 20, 30, 255]);
        let other = doc.layers[1].editor_id.to_string();
        let mut s = mobile(doc);
        s.editor.selection = Some((1, 1, 2, 2));
        let original = s.editor.doc.clone();
        for command in [
            json!({"op":"shape","layer":0,"frame":0,"layer_id":other,"kind":"line","x":0,"y":0,"x2":7,"y2":7,"color":[255,255,255,255]}),
            json!({"op":"move-region","layer":0,"frame":0,"from_layer":1,"from_frame":0,"x":0,"y":0,"width":1,"height":1,"to_x":2,"to_y":2}),
        ] {
            assert!(
                apply(
                    &mut s,
                    &serde_json::to_vec(&json!({"revision":1,"commands":[command]})).unwrap()
                )
                .is_err()
            );
            assert_eq!(s.editor.doc, original);
            assert_eq!(s.revision, 1);
        }
        assert!(apply(&mut s,br#"{"revision":1,"commands":[{"op":"add-layer"},{"op":"shape","layer":0,"frame":0,"x":0,"y":0,"x2":7,"y2":7,"color":[255,255,255,255]}]}"#).is_err());
        assert_eq!(s.editor.doc, original);
    }
    #[test]
    fn named_palette_edits_match_existing_entries_and_reject_invalid_data_atomically() {
        let mut s = mobile(Document::new(8, 8));
        apply(&mut s,br#"{"revision":1,"commands":[{"op":"palette-entry","name":"Ocean","color":[17,34,51,128]}]}"#).unwrap();
        let count = s.editor.doc.palette.len();
        let packet = json!({"revision":s.revision,"commands":[{"op":"palette-entry","name":"Ocean","color":[10,20,30,255]}]});
        apply(&mut s, &serde_json::to_vec(&packet).unwrap()).unwrap();
        assert_eq!(s.editor.doc.palette.len(), count);
        let original = s.editor.doc.clone();
        let invalid = json!({"revision":s.revision,"commands":[{"op":"palette-entry","name":"x".repeat(129),"color":[0,0,0,255]}]});
        assert!(apply(&mut s, &serde_json::to_vec(&invalid).unwrap()).is_err());
        assert_eq!(s.editor.doc, original);
    }
    #[test]
    fn selected_fill_cannot_leave_selection_to_bypass_a_barrier() {
        let mut doc = Document::new(8, 8);
        for y in 1..=6 {
            doc.set_pixel(0, 0, y * 8 + 3, [200, 30, 40, 255]);
        }
        let mut s = mobile(doc);
        s.editor.selection = Some((1, 1, 6, 6));
        apply(&mut s,br#"{"revision":1,"commands":[{"op":"fill","layer":0,"frame":0,"x":2,"y":2,"color":[255,255,255,255]}]}"#).unwrap();
        assert_eq!(s.editor.doc.layers[0].cels[0][2 * 8 + 2], [255; 4]);
        assert_eq!(s.editor.doc.layers[0].cels[0][2 * 8 + 5], [0; 4]);
        assert_eq!(s.editor.doc.layers[0].cels[0][2], [0; 4]);
    }
    #[test]
    fn selected_batch_cannot_read_temporary_pixels_outside_selection() {
        let mut s = mobile(Document::new(8, 8));
        s.editor.selection = Some((2, 2, 3, 3));
        apply(&mut s,br#"{"revision":1,"commands":[{"op":"shape","kind":"line","layer":0,"frame":0,"x":0,"y":0,"x2":0,"y2":0,"color":[255,255,255,255]},{"op":"copy-region","layer":0,"frame":0,"x":0,"y":0,"width":1,"height":1,"to_x":2,"to_y":2}]}"#).unwrap();
        assert_eq!(s.editor.doc.layers[0].cels[0][2 * 8 + 2], [0; 4]);
    }
}
