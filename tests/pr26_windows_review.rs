//! Regression coverage for Windows review findings in PR26.
use pixelcraft::{formats, model::*, productivity::*};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "pixelcraft-pr26-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
#[test]
fn oversized_palette_is_rejected_before_parsing() {
    let mut bytes = b"FF0000\n".to_vec();
    bytes.extend_from_slice("; long comment\n".repeat(150_000).as_bytes());
    let error =
        parse_palette(&bytes, "hex").expect_err("oversized palette must be bounded before parsing");
    assert!(error.contains("limit"), "{error}");
}
#[test]
fn palette_stops_at_color_257_before_reading_invalid_tail() {
    let mut text = "000000\n".repeat(257);
    text.push_str("malformed tail");
    let error = parse_palette(text.as_bytes(), "hex").unwrap_err();
    assert!(error.contains("256"), "{error}");
}
#[test]
fn project_save_does_not_follow_a_preexisting_temporary_hard_link() {
    let root = scratch("temp-link");
    let output = root.join("art.pixel");
    let victim = root.join("unrelated.txt");
    std::fs::write(&victim, b"unrelated data").unwrap();
    let predicted = root.join(format!(".art.pixel.{}.tmp", std::process::id()));
    std::fs::hard_link(&victim, &predicted).unwrap();
    let doc = Document::new(2, 2);
    formats::save(&output, &doc).unwrap();
    assert_eq!(
        std::fs::read(&victim).unwrap(),
        b"unrelated data",
        "saving followed an existing temporary-file link"
    );
    assert!(formats::load(&output).unwrap() == doc);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn tag_gif_scale_keeps_timing_order_and_repeat() {
    let root = scratch("tag-gif");
    let mut doc = Document::new(2, 3);
    doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
    doc.add_frame(0, false);
    doc.set_pixel(0, 1, 0, [0, 255, 0, 255]);
    doc.durations = vec![80, 120];
    doc.tags.push(FrameTag {
        editor_id: Default::default(),
        name: "walk".into(),
        from: 0,
        to: 1,
        direction: 0,
        repeat: 2,
        color: [1, 2, 3],
    });
    let options = pixelcraft::workflows::SheetOptions {
        scale: 2,
        ..Default::default()
    };
    let paths = batch_tags(&root, &doc, true, &options).unwrap();
    let gif = formats::load(&paths[0]).unwrap();
    assert_eq!((gif.width, gif.height), (4, 6));
    assert_eq!(gif.durations, doc.durations);
    assert_eq!(gif.composite(0)[0], [255, 0, 0, 255]);
    assert_eq!(gif.composite(1)[0], [0, 255, 0, 255]);
    let bytes = std::fs::read(&paths[0]).unwrap();
    assert!(
        bytes.windows(5).any(|w| w == [3, 1, 2, 0, 0]),
        "finite repeat count lost"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn staged_new_output_preserves_a_file_created_during_encoding() {
    let root = scratch("late-output");
    let output = root.join("image.png");
    let plan =
        pixelcraft::automation::EditorOutputPlan::new(output.clone(), vec![output.clone()], false)
            .unwrap();
    let result = plan.write(false, |staged| {
        std::fs::write(staged, b"encoded").unwrap();
        std::fs::write(&output, b"late foreign output").unwrap();
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"late foreign output");
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn approved_primary_does_not_authorize_a_late_sidecar() {
    let root = scratch("late-sidecar");
    let image = root.join("image.png");
    let meta = image.with_extension("json");
    std::fs::write(&image, b"original image").unwrap();
    let plan = pixelcraft::automation::EditorOutputPlan::new(
        image.clone(),
        vec![image.clone(), meta.clone()],
        false,
    )
    .unwrap();
    assert_eq!(plan.existing(), &[image.canonicalize().unwrap()]);
    let result = plan.write(true, |staged| {
        std::fs::write(staged, b"new image").unwrap();
        std::fs::write(staged.with_extension("json"), b"new metadata").unwrap();
        std::fs::write(&meta, b"foreign late metadata").unwrap();
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&image).unwrap(), b"original image");
    assert_eq!(std::fs::read(&meta).unwrap(), b"foreign late metadata");
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn confirmed_manifest_replaces_only_listed_existing_outputs() {
    let root = scratch("confirmed-outputs");
    let image = root.join("image.png");
    let meta = image.with_extension("json");
    std::fs::write(&meta, b"old metadata").unwrap();
    let plan = pixelcraft::automation::EditorOutputPlan::new(
        image.clone(),
        vec![image.clone(), meta.clone()],
        false,
    )
    .unwrap();
    assert_eq!(plan.existing(), &[meta.canonicalize().unwrap()]);
    let colors = vec![[1, 2, 3, 255]];
    plan.write(true, |staged| {
        formats::export_png(staged, &Document::new(2, 2), 0, false)?;
        save_palette(&staged.with_extension("json"), &colors, "json")
    })
    .unwrap();
    assert_eq!(
        image::open(&image).unwrap().into_rgba8().dimensions(),
        (2, 2)
    );
    assert_eq!(
        std::fs::read(&meta).unwrap(),
        write_palette(&colors, "json").unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn save_as_no_clobber_preserves_an_unapproved_existing_target() {
    let root = scratch("save-no-clobber");
    let output = root.join("document.pixel");
    std::fs::write(&output, b"foreign project").unwrap();
    assert!(formats::save_with_overwrite(&output, &Document::new(2, 2), false).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"foreign project");
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn palette_file_loader_rejects_oversized_input() {
    let root = scratch("bounded-palette");
    let path = root.join("colors.hex");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_PALETTE_BYTES as u64 + 1).unwrap();
    drop(file);
    assert!(load_palette(&path).unwrap_err().contains("limit"));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(any(windows, unix))]
#[test]
fn sequence_destination_must_not_follow_a_directory_link() {
    let root = scratch("linked-sequence");
    let selected = root.join("selected");
    let outside = root.join("outside");
    std::fs::create_dir(&selected).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let link = selected.join("sequence");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let result = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let result = pixelcraft::automation::EditorOutputPlan::new(
        link.clone(),
        vec![link.join("frame-0001.png")],
        true,
    );
    assert!(result.is_err(), "linked sequence folder was accepted");
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
    #[cfg(windows)]
    std::fs::remove_dir(&link).unwrap();
    #[cfg(unix)]
    std::fs::remove_file(&link).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn replaced_staging_is_not_published_or_recursively_deleted() {
    let root = scratch("staging-replacement");
    let output = root.join("image.png");
    let mut replacement = None;
    let plan =
        pixelcraft::automation::EditorOutputPlan::new(output.clone(), vec![output.clone()], false)
            .unwrap();
    let result = plan.write(false, |staged| {
        formats::export_png(staged, &Document::new(2, 2), 0, false)?;
        let stage = staged.parent().unwrap().to_path_buf();
        let moved = root.join("moved-owned-staging");
        match std::fs::rename(&stage, &moved) {
            Ok(()) => {
                std::fs::create_dir(&stage).unwrap();
                let marker = stage.join("foreign.txt");
                std::fs::write(&marker, b"keep unrelated file").unwrap();
                replacement = Some(marker);
            }
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
            Err(error) => return Err(error.to_string()),
        }
        Ok(())
    });
    if let Some(marker) = replacement {
        assert!(result.is_err());
        assert!(!output.exists());
        assert_eq!(std::fs::read(marker).unwrap(), b"keep unrelated file");
    }
    std::fs::remove_dir_all(root).unwrap();
}
