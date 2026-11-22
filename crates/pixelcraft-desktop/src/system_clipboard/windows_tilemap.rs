//! Publish a registered tilemap format alongside standard image formats.
use super::Clipboard;
use super::tilemap_wire::{Flavor, decode, encode, preview_png};
use crate::api::model::Pixel;
use clipboard_win::{Clipboard as WindowsClipboard, formats::CF_DIBV5, raw};
use std::{ptr, thread, time::Duration};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE},
};

fn format_name(flavor: Flavor) -> &'static str {
    match flavor {
        Flavor::Tilemap => "PixelCraft Tilemap",
        Flavor::Indexed => "PixelCraft Indexed Raster",
    }
}
struct ClipboardOwner(HWND);

impl ClipboardOwner {
    fn new() -> Result<Self, String> {
        let class = [
            b'S' as u16,
            b'T' as u16,
            b'A' as u16,
            b'T' as u16,
            b'I' as u16,
            b'C' as u16,
            0,
        ];
        // The predefined STATIC class and HWND_MESSAGE create a nonvisual owner
        // on this worker thread. Clipboard data is fully rendered before closing.
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if window.is_null() {
            Err(format!(
                "Could not create the clipboard owner. {}",
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(Self(window))
        }
    }
}

impl Drop for ClipboardOwner {
    fn drop(&mut self) {
        // Created on this thread and only used while the clipboard is open.
        unsafe { DestroyWindow(self.0) };
    }
}

fn open(owner: HWND) -> Result<WindowsClipboard, String> {
    for attempt in 0..6 {
        match WindowsClipboard::new_for(owner) {
            Ok(clipboard) => return Ok(clipboard),
            Err(error) if attempt == 5 => return Err(error.to_string()),
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    unreachable!()
}

pub(super) fn publish(clip: &Clipboard) -> Result<(), String> {
    let data = encode(clip)?;
    // Large maps can still transfer as cells even if their rendered bitmap
    // exceeds the ordinary image cap.
    let bitmap = if let Ok((width, height, pixels)) = clip.native_bitmap() {
        Some((
            preview_png(width, height, &pixels)?,
            dibv5(width, height, &pixels),
        ))
    } else {
        None
    };
    let flavor = Flavor::of(clip).ok_or("Image has no packed clipboard format")?;
    let format = raw::register_format(format_name(flavor))
        .ok_or("Could not register the packed clipboard format")?;
    let png_format = bitmap.as_ref().and_then(|_| raw::register_format("PNG"));
    let owner = ClipboardOwner::new()?;
    let _clipboard = open(owner.0)?;
    raw::empty().map_err(|error| error.to_string())?;
    raw::set_without_clear(format.get(), &data).map_err(|error| error.to_string())?;
    if let Some((png, dib)) = bitmap {
        if let Some(format) = png_format {
            raw::set_without_clear(format.get(), &png).map_err(|error| error.to_string())?;
        }
        raw::set_without_clear(CF_DIBV5, &dib).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) fn read() -> Option<Clipboard> {
    let clipboard = open(ptr::null_mut()).ok()?;
    let (flavor, data) = Flavor::ALL.into_iter().find_map(|flavor| {
        let format = raw::register_format(format_name(flavor))?.get();
        if !raw::is_format_avail(format) || raw::size(format)?.get() > flavor.limit() {
            return None;
        }
        let mut data = Vec::new();
        raw::get_vec(format, &mut data).ok()?;
        Some((flavor, data))
    })?;
    drop(clipboard);
    decode(&data, flavor)
}

fn dibv5(width: usize, height: usize, pixels: &[Pixel]) -> Vec<u8> {
    use windows_sys::Win32::Graphics::Gdi::{BI_BITFIELDS, LCS_GM_IMAGES};

    // BITMAPV5HEADER followed by bottom-up BGRA scanlines.
    const DIB_HEADER: usize = 124;
    let mut data = vec![0; DIB_HEADER];
    for (offset, value) in [
        (0, DIB_HEADER as u32),
        (4, width as u32),
        (8, height as u32),
        (12, 1 | (32 << 16)),
        (16, BI_BITFIELDS),
        (20, (pixels.len() * 4) as u32),
        (40, 0x00ff_0000),
        (44, 0x0000_ff00),
        (48, 0x0000_00ff),
        (52, 0xff00_0000),
        (56, u32::from_be_bytes(*b"sRGB")),
        (108, LCS_GM_IMAGES as u32),
    ] {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    data.reserve_exact(pixels.len() * 4);
    for row in pixels.chunks_exact(width).rev() {
        for &[r, g, b, a] in row {
            data.extend_from_slice(&[b, g, r, a]);
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::super::tilemap_wire::tests::{indexed_sample, sample};
    use super::*;
    use crate::system_clipboard::{Completion, SystemClipboard};

    #[test]
    fn dibv5_preview_stores_bottom_up_bgra_with_alpha() {
        let dib = dibv5(1, 2, &[[1, 2, 3, 4], [5, 6, 7, 8]]);
        assert_eq!(&dib[124..], &[7, 6, 5, 8, 3, 2, 1, 4]);
    }

    #[test]
    #[ignore = "Writes the Windows clipboard. Run inside an isolated window station."]
    fn packed_tilemap_cross_process_roundtrip() {
        let mut worker = SystemClipboard::default();
        for (kind, clip) in [("indexed", indexed_sample()), ("tilemap", sample())] {
            worker.copy_content(&clip).unwrap();
            assert!(matches!(completion(&mut worker), Completion::Copy(Ok(()))));
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "system_clipboard::windows_tilemap::tests::packed_tilemap_reader",
                    "--ignored",
                    "--nocapture",
                ])
                .env("PIXELCRAFT_CLIPBOARD_PROBE_CHILD", kind)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&result.stdout);
            assert!(
                result.status.success()
                    && stdout.contains("Verified packed clipboard and preview."),
                "{stdout}\n{}",
                String::from_utf8_lossy(&result.stderr)
            );
            println!("{stdout}");
        }
    }

    #[test]
    #[ignore = "Child process for the isolated Windows clipboard round trip."]
    fn packed_tilemap_reader() {
        let expected = match std::env::var("PIXELCRAFT_CLIPBOARD_PROBE_CHILD").as_deref() {
            Ok("tilemap") => sample(),
            Ok("indexed") => indexed_sample(),
            _ => panic!("missing clipboard test payload"),
        };
        let mut worker = SystemClipboard::default();
        worker.paste().unwrap();
        let Completion::Packed(actual) = completion(&mut worker) else {
            panic!("missing packed tilemap in another process");
        };
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let (w, h, pixels) = expected.native_bitmap().unwrap();
        let preview = arboard::Clipboard::new().unwrap().get_image().unwrap();
        assert_eq!((preview.width, preview.height), (w, h));
        assert_eq!(preview.bytes.as_ref(), pixels.as_flattened());
        assert!(raw::is_format_avail(CF_DIBV5));
        println!("Verified packed clipboard and preview.");
    }

    fn completion(worker: &mut SystemClipboard) -> Completion {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = worker.poll() {
                return result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "clipboard worker response timed out"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }
}
