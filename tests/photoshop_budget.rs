use pixelcraft::formats;

// Invalid three-byte channel bodies keep these tests cheap: the work budget
// must reject their declared dimensions before attempting to decode any body.
fn photoshop_layers(records: &[(i32, i32, u8, Vec<i16>)]) -> Vec<u8> {
    let mut layers = (records.len() as i16).to_be_bytes().to_vec();
    for (layer_size, mask_size, mask_flags, channels) in records {
        for value in [0i32, 0, *layer_size, *layer_size] {
            layers.extend_from_slice(&value.to_be_bytes());
        }
        layers.extend_from_slice(&(channels.len() as u16).to_be_bytes());
        for id in channels {
            layers.extend_from_slice(&id.to_be_bytes());
            layers.extend_from_slice(&3u32.to_be_bytes());
        }
        layers.extend_from_slice(b"8BIMnorm\xff\0\0\0");
        let mut extra = 20u32.to_be_bytes().to_vec();
        for value in [0i32, 0, *mask_size, *mask_size] {
            extra.extend_from_slice(&value.to_be_bytes());
        }
        extra.extend_from_slice(&[0, *mask_flags, 0, 0]);
        extra.extend_from_slice(&[0; 8]); // Empty blend ranges and layer name.
        layers.extend_from_slice(&(extra.len() as u32).to_be_bytes());
        layers.extend(extra);
    }
    for (_, _, _, channels) in records {
        for _ in channels {
            layers.extend_from_slice(&[0xff, 0xff, 0]);
        }
    }
    layers.resize(layers.len().next_multiple_of(2), 0);
    let mut section = (layers.len() as u32).to_be_bytes().to_vec();
    section.extend(layers);
    section.extend_from_slice(&[0; 4]);
    let mut bytes = b"8BPS\0\x01".to_vec();
    bytes.extend_from_slice(&[0; 6]);
    bytes.extend_from_slice(b"\0\x03\0\0\0\x01\0\0\0\x01\0\x08\0\x03");
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&(section.len() as u32).to_be_bytes());
    bytes.extend(section);
    bytes
}

fn load_error(records: &[(i32, i32, u8, Vec<i16>)], case: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "pixelcraft-photoshop-budget-{}-{case}.psd",
        std::process::id()
    ));
    std::fs::write(&path, photoshop_layers(records)).unwrap();
    let result = formats::load(&path);
    std::fs::remove_file(path).unwrap();
    result.unwrap_err()
}

#[test]
fn photoshop_decode_budget_counts_independent_and_repeated_mask_channels() {
    for (case, records) in [
        ("independent-masks", vec![(1, 8000, 0, vec![-2]); 9]),
        ("repeated-masks", vec![(1, 8000, 0, vec![-2; 9])]),
        ("repeated-colors", vec![(8000, 1, 0, vec![0; 9])]),
        ("repeated-alpha", vec![(8000, 1, 0, vec![-1; 9])]),
    ] {
        let error = load_error(&records, case);
        assert!(error.contains("pixel work limit"), "{case}: {error}");
    }
}

#[test]
fn photoshop_decode_budget_ignores_disabled_and_unsupported_channels() {
    for (case, layer_size, mask_size, channels, flags) in [
        ("disabled-masks", 1, 8000, vec![-2; 9], 2),
        ("unsupported-channels", 8000, 1, vec![8; 9], 0),
        ("at-limit", 8000, 1, vec![0; 7], 0),
    ] {
        let mut channels = channels;
        channels.push(0);
        let error = load_error(&[(layer_size, mask_size, flags, channels)], case);
        assert!(
            error.contains("Unsupported Photoshop channel compression"),
            "{case}: {error}"
        );
    }
}
