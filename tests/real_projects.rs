use pixelcraft::{formats::*, model::*};
use std::path::Path;
#[test]
fn real_aseprite_projects_preserve_frames_and_metadata() {
    for item in
        std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")).unwrap()
    {
        let path = item.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("aseprite") {
            continue;
        }
        let doc = load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let sheet = image::open(
            path.parent()
                .unwrap()
                .join("rendered")
                .join(path.file_stem().unwrap())
                .with_extension("png"),
        )
        .unwrap()
        .to_rgba8();
        assert_eq!(
            sheet.dimensions(),
            ((doc.width * doc.durations.len()) as u32, doc.height as u32)
        );
        for f in 0..doc.durations.len() {
            let actual = doc.composite(f);
            for y in 0..doc.height {
                for x in 0..doc.width {
                    let expected = sheet.get_pixel((f * doc.width + x) as u32, y as u32).0;
                    let got = actual[y * doc.width + x];
                    assert!(
                        got == expected || got[3] == 0 && expected[3] == 0,
                        "{} frame {f} ({x},{y}): got {got:?} expected {expected:?}",
                        path.display()
                    );
                }
            }
        }
        let saved = encode_ase(&doc).unwrap();
        let read = decode_ase(&saved).unwrap_or_else(|e| panic!("{} reread: {e}", path.display()));
        for f in 0..doc.durations.len() {
            assert_eq!(
                doc.composite(f),
                read.composite(f),
                "{} frame {f}",
                path.display()
            );
        }
        assert_eq!(doc.tags, read.tags, "{} tags", path.display());
        assert_eq!(doc.color_profile, read.color_profile);
        let opaque = |d: &Document| {
            let mut values: Vec<_> = d
                .metadata
                .iter()
                .filter(|m| m.kind != 0)
                .map(|m| (m.frame, m.kind, m.owner.clone(), m.data.clone()))
                .collect();
            values.sort();
            values
        };
        assert_eq!(opaque(&doc), opaque(&read), "{} metadata", path.display());
        assert_eq!(decode_project(&encode_project(&doc).unwrap()).unwrap(), doc);
    }
}
