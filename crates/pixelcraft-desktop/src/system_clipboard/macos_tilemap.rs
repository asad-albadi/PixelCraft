//! Native editable tilemaps and their flattened preview share a pasteboard item.
use super::Clipboard;
use super::tilemap_wire::{Flavor, decode, encode, preview_png};
use image::{ExtendedColorType, ImageEncoder, codecs::tiff::TiffEncoder};
use objc2::{rc::autoreleasepool, runtime::ProtocolObject};
use objc2_app_kit::{NSPasteboard, NSPasteboardItem};
use objc2_foundation::{NSArray, NSData, NSString};
use std::io::Cursor;

fn type_name(flavor: Flavor) -> &'static str {
    match flavor {
        Flavor::Tilemap => "com.asad-albadi.pixelcraft.tilemap",
        Flavor::Indexed => "com.asad-albadi.pixelcraft.indexed-raster",
    }
}
const PNG_TYPE: &str = "public.png";
const TIFF_TYPE: &str = "public.tiff";

pub(super) fn publish(clip: &Clipboard) -> Result<(), String> {
    autoreleasepool(|_| publish_to(&NSPasteboard::generalPasteboard(), clip))
}

pub(super) fn read() -> Option<Clipboard> {
    autoreleasepool(|_| read_from(&NSPasteboard::generalPasteboard()))
}

fn publish_to(board: &NSPasteboard, clip: &Clipboard) -> Result<(), String> {
    let flavor = Flavor::of(clip).ok_or("Image has no packed clipboard format")?;
    let item = NSPasteboardItem::new();
    if !item.setData_forType(
        &NSData::with_bytes(&encode(clip)?),
        &NSString::from_str(type_name(flavor)),
    ) {
        return Err("Could not prepare the tilemap clipboard data".into());
    }
    if let Ok((width, height, pixels)) = clip.native_bitmap() {
        let mut tiff = Cursor::new(Vec::new());
        TiffEncoder::new(&mut tiff)
            .write_image(
                pixels.as_flattened(),
                width as u32,
                height as u32,
                ExtendedColorType::Rgba8,
            )
            .map_err(|error| error.to_string())?;
        for (format, preview) in [
            (TIFF_TYPE, tiff.into_inner()),
            (PNG_TYPE, preview_png(width, height, &pixels)?),
        ] {
            if !item.setData_forType(&NSData::with_bytes(&preview), &NSString::from_str(format)) {
                return Err(format!("Could not prepare the {format} clipboard preview"));
            }
        }
    }

    board.clearContents();
    let objects = NSArray::from_retained_slice(&[ProtocolObject::from_retained(item)]);
    if !board.writeObjects(&objects) {
        return Err("Could not write the tilemap to the pasteboard".into());
    }
    Ok(())
}

fn read_from(board: &NSPasteboard) -> Option<Clipboard> {
    Flavor::ALL.into_iter().find_map(|flavor| {
        let data = board.dataForType(&NSString::from_str(type_name(flavor)))?;
        if data.length() > flavor.limit() {
            return None;
        }
        decode(&data.to_vec(), flavor)
    })
}

#[cfg(test)]
mod tests {
    use super::super::tilemap_wire::tests::{indexed_sample, sample};
    use super::*;

    #[test]
    fn packed_tilemap_named_pasteboard_roundtrip() {
        const BOARD_ENV: &str = "PIXELCRAFT_CLIPBOARD_TEST_BOARD";
        const KIND_ENV: &str = "PIXELCRAFT_CLIPBOARD_TEST_KIND";
        autoreleasepool(|_| {
            if let Ok(name) = std::env::var(BOARD_ENV) {
                let expected = match std::env::var(KIND_ENV).as_deref() {
                    Ok("tilemap") => sample(),
                    Ok("indexed") => indexed_sample(),
                    _ => panic!("missing clipboard test payload"),
                };
                let board = NSPasteboard::pasteboardWithName(&NSString::from_str(&name));
                let actual = read_from(&board).expect("missing packed data in another process");
                assert_eq!(
                    serde_json::to_value(actual).unwrap(),
                    serde_json::to_value(&expected).unwrap()
                );

                let (w, h, pixels) = expected.native_bitmap().unwrap();
                for (format, decoder) in [
                    (PNG_TYPE, image::ImageFormat::Png),
                    (TIFF_TYPE, image::ImageFormat::Tiff),
                ] {
                    let data = board
                        .dataForType(&NSString::from_str(format))
                        .unwrap_or_else(|| panic!("missing standard {format} preview"));
                    let preview = image::load_from_memory_with_format(&data.to_vec(), decoder)
                        .unwrap()
                        .into_rgba8();
                    assert_eq!(preview.dimensions(), (w as u32, h as u32));
                    assert_eq!(preview.as_raw().as_slice(), pixels.as_flattened());
                }
                println!("Verified packed clipboard and preview.");
                return;
            }

            for (kind, expected) in [("tilemap", sample()), ("indexed", indexed_sample())] {
                let board = NSPasteboard::pasteboardWithUniqueName();
                publish_to(&board, &expected).unwrap();
                let result = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "system_clipboard::macos_tilemap::tests::packed_tilemap_named_pasteboard_roundtrip",
                        "--nocapture",
                    ])
                    .env(BOARD_ENV, board.name().to_string())
                    .env(KIND_ENV, kind)
                    .output();
                board.clearContents();
                let result = result.unwrap();
                let stdout = String::from_utf8_lossy(&result.stdout);
                assert!(
                    result.status.success()
                        && stdout.contains("Verified packed clipboard and preview."),
                    "{stdout}\n{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                println!("{stdout}");
            }
        });
    }
}
