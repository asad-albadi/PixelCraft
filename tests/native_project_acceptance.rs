//! Opt-in real-project acceptance. Inputs are always opened read-only.
use pixelcraft::formats;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Instant};

#[test]
#[ignore = "requires local Autumn Breeze projects; writes disposable acceptance outputs"]
fn saved_day_and_night_projects_render_save_reopen_and_export() {
    let paths: Vec<PathBuf> = serde_json::from_str(
        &std::env::var("PIXELCRAFT_ACCEPT_PROJECTS").expect("JSON input paths"),
    )
    .unwrap();
    let root = PathBuf::from(
        std::env::var("PIXELCRAFT_ACCEPT_OUTPUT").expect("disposable output directory"),
    );
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(paths.len(), 2, "exercise both day and night");
    for (index, path) in paths.iter().enumerate() {
        let hash = Sha256::digest(std::fs::read(path).unwrap());
        let start = Instant::now();
        let doc = formats::load(path).unwrap();
        let load = start.elapsed();
        doc.validate().unwrap();
        assert_eq!((doc.width, doc.height, doc.durations.len()), (480, 270, 24));
        let start = Instant::now();
        let composites: Vec<_> = (0..doc.durations.len())
            .map(|frame| doc.composite_view(frame))
            .collect();
        let render = start.elapsed();
        assert!(
            composites.windows(2).any(|frames| frames[0] != frames[1]),
            "animation must retain distinct frames"
        );
        let destination = root.join(format!("project-{index}.pixel"));
        assert!(!paths.contains(&destination));
        let start = Instant::now();
        formats::save(&destination, &doc).unwrap();
        let save = start.elapsed();
        let start = Instant::now();
        let reopened = formats::load(&destination).unwrap();
        let reopen = start.elapsed();
        assert_eq!(
            doc, reopened,
            "all native metadata, frames, layers and samples must survive"
        );
        for (frame, pixels) in composites.iter().enumerate() {
            assert_eq!(*pixels, reopened.composite_view(frame), "frame {frame}");
        }
        let start = Instant::now();
        let png = root.join(format!("project-{index}.png"));
        formats::export_png(&png, &doc, 0, false).unwrap();
        assert_eq!(formats::load(&png).unwrap().composite(0), doc.composite(0));
        formats::export_gif(&root.join(format!("project-{index}.gif")), &doc).unwrap();
        formats::export_mp4(&root.join(format!("project-{index}.mp4")), &doc, 1).unwrap();
        let export = start.elapsed();
        let start = Instant::now();
        let ase = root.join(format!("project-{index}.aseprite"));
        formats::save(&ase, &doc).unwrap();
        let ase_save = start.elapsed();
        let ase_read = formats::load(&ase).unwrap();
        assert_eq!(ase_read.durations, doc.durations);
        assert_eq!(ase_read.layers.len(), doc.layers.len());
        for frame in 0..doc.durations.len() {
            assert_eq!(
                ase_read.composite(frame),
                doc.composite(frame),
                "ASE frame {frame}"
            );
        }
        assert_eq!(
            hash,
            Sha256::digest(std::fs::read(path).unwrap()),
            "source file changed"
        );
        eprintln!(
            "ACCEPT {}: {}x{}, {} layers, {} frames; load={load:?}, render24={render:?}, save={save:?}, reopen={reopen:?}, exports={export:?}, ase_save={ase_save:?}; source_sha256={hash:x}",
            path.display(),
            doc.width,
            doc.height,
            doc.layers.len(),
            doc.durations.len()
        );
    }
}
