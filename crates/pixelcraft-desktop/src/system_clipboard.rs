//! Native image and text clipboard access. X11/Windows/macOS use arboard; Wayland
//! uses native data control for tilemaps and has desktop-helper and XWayland fallbacks.
#[cfg(target_os = "macos")]
mod macos_tilemap;
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
mod tilemap_wire;
#[cfg(target_os = "linux")]
mod wayland_offer;
#[cfg(target_os = "linux")]
mod wayland_tilemap;
#[cfg(windows)]
mod windows_tilemap;
#[cfg(target_os = "linux")]
mod x11_tilemap;

#[cfg(target_os = "macos")]
use macos_tilemap as native_tilemap;
#[cfg(windows)]
use windows_tilemap as native_tilemap;

use super::Clipboard;
use crate::api::model::{MAX_DIMENSION, MAX_PIXELS, Pixel};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(2);

type ClipboardImage = (usize, usize, Vec<Pixel>);
pub enum Completion {
    Copy(Result<(), String>),
    Paste(Result<ClipboardImage, String>),
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    Packed(Box<Clipboard>),
}
enum Operation {
    Copy(usize, usize, Vec<Pixel>),
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    CopyPacked(Box<Clipboard>),
    Paste,
}

#[derive(Clone, Copy)]
enum JobKind {
    Write,
    ReadImage,
}
impl JobKind {
    fn error(self, reason: &str) -> Completion {
        let reason = reason.to_owned();
        match self {
            Self::Write => Completion::Copy(Err(reason)),
            Self::ReadImage => Completion::Paste(Err(reason)),
        }
    }
}

struct Job {
    started: Instant,
    response: Receiver<Completion>,
    kind: JobKind,
    expired: bool,
}
/// One native owner and at most one in-flight request, even after a timeout.
#[derive(Default)]
pub struct SystemClipboard {
    worker: Option<SyncSender<(Operation, SyncSender<Completion>)>>,
    job: Option<Job>,
}
impl SystemClipboard {
    #[cfg(test)]
    pub(super) fn inject_completion_for_test(&mut self, completion: Completion) {
        let (tx, response) = mpsc::channel();
        tx.send(completion).unwrap();
        self.job = Some(Job {
            started: Instant::now(),
            response,
            kind: JobKind::ReadImage,
            expired: false,
        });
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
    fn start(&mut self, operation: Operation) -> Result<(), String> {
        if self.busy() {
            return Err("System clipboard is still busy".into());
        }
        if self.worker.is_none() {
            let (tx, rx) = mpsc::sync_channel::<(Operation, SyncSender<Completion>)>(1);
            std::thread::Builder::new()
                .name("clipboard".into())
                .spawn(move || {
                    let mut native = NativeClipboard::default();
                    while let Ok((operation, response)) = rx.recv() {
                        #[cfg(target_os = "linux")]
                        let replaces_packed_offer = matches!(operation, Operation::Copy(..));
                        let result = match operation {
                            Operation::Copy(w, h, pixels) => {
                                Completion::Copy(native.set(w, h, &pixels))
                            }
                            #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
                            Operation::CopyPacked(clip) => {
                                Completion::Copy(native.copy_packed(&clip))
                            }
                            Operation::Paste => native.read_content(),
                        };
                        #[cfg(target_os = "linux")]
                        if replaces_packed_offer && matches!(result, Completion::Copy(Ok(()))) {
                            native.x11_offer = None;
                            native.wayland_offer = None;
                        }
                        let _ = response.send(result);
                    }
                })
                .map_err(|e| e.to_string())?;
            self.worker = Some(tx);
        }
        let kind = match operation {
            Operation::Paste => JobKind::ReadImage,

            _ => JobKind::Write,
        };
        let (tx, rx) = mpsc::sync_channel(1);
        self.worker
            .as_ref()
            .unwrap()
            .try_send((operation, tx))
            .map_err(|e| e.to_string())?;
        self.job = Some(Job {
            started: Instant::now(),
            response: rx,
            kind,
            expired: false,
        });
        Ok(())
    }
    pub fn copy(&mut self, w: usize, h: usize, pixels: Vec<Pixel>) -> Result<(), String> {
        self.start(Operation::Copy(w, h, pixels))
    }
    pub(super) fn copy_content(&mut self, clip: &Clipboard) -> Result<(), String> {
        #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
        if tilemap_wire::Flavor::of(clip).is_some() {
            return self.start(Operation::CopyPacked(Box::new(clip.clone())));
        }
        let (w, h, pixels) = clip.native_bitmap()?;
        self.copy(w, h, pixels)
    }
    pub fn paste(&mut self) -> Result<(), String> {
        self.start(Operation::Paste)
    }

    pub fn poll(&mut self) -> Option<Completion> {
        self.poll_at(Instant::now())
    }
    fn poll_at(&mut self, now: Instant) -> Option<Completion> {
        let job = self.job.as_mut()?;
        // A result observed after the deadline is never applied to the document.
        if !job.expired && now.duration_since(job.started) >= CLIPBOARD_TIMEOUT {
            job.expired = true;
            return Some(job.kind.error("System clipboard timed out"));
        }
        match job.response.try_recv() {
            Ok(result) => {
                let expired = job.expired;
                self.job = None;
                if expired { None } else { Some(result) }
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                let expired = job.expired;
                let kind = job.kind;
                self.job = None;
                self.worker = None;
                (!expired).then(|| kind.error("Clipboard worker stopped"))
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn run_helper(
    mut command: std::process::Command,
    input: Option<Vec<u8>>,
    limit: usize,
) -> Result<Vec<u8>, String> {
    use std::io::{Read, Write};
    use std::process::Stdio;
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        // wl-copy may fork a clipboard owner; it must not inherit a pipe we await.
        .stdout(if input.is_some() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let (tx, rx) = mpsc::sync_channel(1);
    let io_thread = std::thread::Builder::new()
        .name("clipboard-pipe".into())
        .spawn(move || {
            let result = (|| {
                if let (Some(mut stdin), Some(bytes)) = (stdin, input) {
                    stdin.write_all(&bytes).map_err(|e| e.to_string())?;
                }
                let mut bytes = vec![];
                if let Some(stdout) = stdout {
                    stdout
                        .take(limit as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                }
                if bytes.len() > limit {
                    return Err("Clipboard data exceeds limits".into());
                }
                Ok(bytes)
            })();
            let _ = tx.send(result);
        });
    if let Err(e) = io_thread {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e.to_string());
    }
    let started = Instant::now();
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => break Err("Clipboard helper failed".into()),
            Err(e) => break Err(e.to_string()),
            _ => {}
        }
        match rx.try_recv() {
            Ok(result) => {
                if let Err(e) = result {
                    break Err(e);
                }
                if let Ok(Some(status)) = child.try_wait() {
                    break if status.success() {
                        result
                    } else {
                        Err("Clipboard helper failed".into())
                    };
                }
                // Keep the IO result while waiting for the process, under the same deadline.
                while started.elapsed() < Duration::from_millis(1500) {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            return if status.success() {
                                result
                            } else {
                                Err("Clipboard helper failed".into())
                            };
                        }
                        Err(e) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err(e.to_string());
                        }
                        _ => std::thread::sleep(Duration::from_millis(5)),
                    }
                }
                break Err("Clipboard helper timed out".into());
            }
            Err(mpsc::TryRecvError::Disconnected) => break Err("Clipboard pipe stopped".into()),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if started.elapsed() >= Duration::from_millis(1500) {
            break Err("Clipboard helper timed out".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[derive(Default)]
struct NativeClipboard {
    native: Option<arboard::Clipboard>,
    #[cfg(target_os = "linux")]
    x11_offer: Option<x11_tilemap::Owner>,
    #[cfg(target_os = "linux")]
    wayland_offer: Option<wayland_offer::Owner>,
}
impl NativeClipboard {
    fn read_content(&mut self) -> Completion {
        #[cfg(any(windows, target_os = "macos"))]
        let packed = native_tilemap::read();
        #[cfg(target_os = "linux")]
        let packed = if wayland_tilemap::is_wayland() {
            match wayland_tilemap::read() {
                Ok(clip) => clip,
                Err(
                    wl_clipboard_rs::paste::Error::MissingProtocol { .. }
                    | wl_clipboard_rs::paste::Error::NoSeats
                    | wl_clipboard_rs::paste::Error::WaylandConnection(_),
                ) => x11_tilemap::read(),
                Err(_) => None,
            }
        } else {
            x11_tilemap::read()
        };
        #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
        if let Some(clip) = packed {
            return Completion::Packed(Box::new(clip));
        }
        Completion::Paste(self.get())
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn copy_packed(&mut self, clip: &Clipboard) -> Result<(), String> {
        native_tilemap::publish(clip)
    }

    #[cfg(target_os = "linux")]
    fn copy_packed(&mut self, clip: &Clipboard) -> Result<(), String> {
        let wayland = wayland_tilemap::is_wayland();
        if wayland && let Some(offer) = wayland_tilemap::publish(clip)? {
            self.x11_offer = None;
            self.wayland_offer = Some(offer);
            return Ok(());
        }
        match x11_tilemap::publish(clip) {
            Ok(offer) => self.x11_offer = Some(offer),
            Err(_) if wayland => {
                let (w, h, pixels) = clip.native_bitmap()?;
                self.set(w, h, &pixels)?;
                self.x11_offer = None;
            }
            Err(error) => return Err(error),
        }
        self.wayland_offer = None;
        Ok(())
    }

    fn native(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.native.is_none() {
            self.native = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        Ok(self.native.as_mut().unwrap())
    }
    pub fn set(&mut self, w: usize, h: usize, pixels: &[Pixel]) -> Result<(), String> {
        if w == 0
            || h == 0
            || w > MAX_DIMENSION
            || h > MAX_DIMENSION
            || w.saturating_mul(h) > MAX_PIXELS
            || pixels.len() != w * h
        {
            return Err("Invalid clipboard image".into());
        }
        #[cfg(target_os = "linux")]
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            let im = image::RgbaImage::from_raw(
                w as u32,
                h as u32,
                pixels.iter().flatten().copied().collect(),
            )
            .unwrap();
            let mut bytes = std::io::Cursor::new(Vec::new());
            im.write_to(&mut bytes, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            let mut command = std::process::Command::new("wl-copy");
            command.args(["--type", "image/png"]);
            match run_helper(command, Some(bytes.into_inner()), 0) {
                Ok(_) => return Ok(()),
                Err(e) if e.contains("timed out") => return Err(e),
                Err(_) => {}
            }
        }
        self.native()?
            .set_image(arboard::ImageData {
                width: w,
                height: h,
                bytes: std::borrow::Cow::Owned(pixels.iter().flatten().copied().collect()),
            })
            .map_err(|e| e.to_string())
    }
    pub fn get(&mut self) -> Result<(usize, usize, Vec<Pixel>), String> {
        #[cfg(target_os = "linux")]
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            let mut command = std::process::Command::new("wl-paste");
            command.args(["--no-newline", "--type", "image/png"]);
            match run_helper(command, None, MAX_PIXELS * 4 + 1024 * 1024) {
                Ok(bytes) => return decode_png(&bytes),
                Err(e) if e.contains("timed out") || e.contains("exceeds limits") => return Err(e),
                Err(_) => {}
            }
        }
        let im = self.native()?.get_image().map_err(|e| e.to_string())?;
        if im.width == 0
            || im.height == 0
            || im.width > MAX_DIMENSION
            || im.height > MAX_DIMENSION
            || im.width.saturating_mul(im.height) > MAX_PIXELS
            || im.bytes.len() != im.width * im.height * 4
        {
            return Err("Clipboard image exceeds limits".into());
        }
        Ok((im.width, im.height, im.bytes.as_chunks::<4>().0.to_vec()))
    }
}
#[cfg(any(target_os = "linux", test))]
fn decode_png(bytes: &[u8]) -> Result<(usize, usize, Vec<Pixel>), String> {
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION as u32);
    limits.max_image_height = Some(MAX_DIMENSION as u32);
    limits.max_alloc = Some((MAX_PIXELS * 4) as u64);
    reader.limits(limits);
    let im = reader.decode().map_err(|e| e.to_string())?.to_rgba8();
    if (im.width() as usize).saturating_mul(im.height() as usize) > MAX_PIXELS {
        return Err("Clipboard image exceeds limits".into());
    }
    Ok((
        im.width() as usize,
        im.height() as usize,
        im.pixels().map(|p| p.0).collect(),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_png_keeps_transparent_and_partial_alpha() {
        let pixels = vec![[7, 8, 9, 0], [20, 40, 60, 127], [255, 0, 0, 255]];
        let im =
            image::RgbaImage::from_raw(3, 1, pixels.iter().flatten().copied().collect()).unwrap();
        let mut bytes = std::io::Cursor::new(Vec::new());
        im.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        assert_eq!(decode_png(bytes.get_ref()).unwrap(), (3, 1, pixels));
    }
}

#[cfg(test)]
mod worker_regressions {
    use super::*;
    #[test]
    fn blocked_native_request_times_out_once_without_accepting_more_work() {
        let (tx, rx) = mpsc::sync_channel(1);
        let start = Instant::now();
        let mut clipboard = SystemClipboard {
            worker: None,
            job: Some(Job {
                started: start,
                response: rx,
                kind: JobKind::ReadImage,
                expired: false,
            }),
        };
        assert!(clipboard.poll_at(start).is_none());
        assert!(
            matches!(clipboard.poll_at(start + CLIPBOARD_TIMEOUT), Some(Completion::Paste(Err(e))) if e.contains("timed out"))
        );
        assert!(clipboard.paste().is_err());
        assert!(clipboard.poll_at(start + CLIPBOARD_TIMEOUT).is_none());
        tx.send(Completion::Paste(Ok((1, 1, vec![[1, 2, 3, 255]]))))
            .unwrap();
        assert!(clipboard.poll_at(start + CLIPBOARD_TIMEOUT).is_none());
        assert!(!clipboard.busy());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stalled_helper_is_killed_before_ui_deadline() {
        for input in [None, Some(vec![0; 1024 * 1024])] {
            let mut command = std::process::Command::new("sh");
            command.args(["-c", "exec sleep 30"]);
            let start = Instant::now();
            assert!(
                run_helper(command, input, 16)
                    .unwrap_err()
                    .contains("timed out")
            );
            assert!(start.elapsed() < CLIPBOARD_TIMEOUT);
        }
    }

    #[cfg(target_os = "linux")]
    const CHILD_VERIFIED: &str = "Verified packed clipboard and PNG in another process.";

    #[cfg(target_os = "linux")]
    fn run_clipboard_child(test: &str, child: &str, kind: &str) {
        let (_, module) = module_path!().split_once("::").unwrap();
        let test = format!("{module}::{test}");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--ignored", "--nocapture"])
            .env(child, kind)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains(CHILD_VERIFIED),
            "Clipboard child {test} did not verify both formats:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{stdout}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "Writes the clipboard. Run against an isolated Wayland compositor with data control."]
    fn wayland_packed_tilemap_cross_process_roundtrip() {
        use super::tilemap_wire::tests::{indexed_sample, sample};
        const CHILD: &str = "PIXELCRAFT_WAYLAND_CLIPBOARD_TEST_CHILD";
        assert!(std::env::var_os("WAYLAND_DISPLAY").is_some());
        assert!(std::env::var_os("DISPLAY").is_none());
        let mut expected = sample();
        expected.tileset.as_mut().unwrap().user_data[2].extend(vec![0x77; 200_000]);
        let mut worker = SystemClipboard::default();
        let complete = |worker: &mut SystemClipboard| {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(result) = worker.poll() {
                    return result;
                }
                assert!(
                    Instant::now() < deadline,
                    "clipboard worker response timed out"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        if let Ok(kind) = std::env::var(CHILD) {
            let expected = if kind == "indexed" {
                indexed_sample()
            } else {
                expected
            };
            assert!(kind == "indexed" || kind == "tilemap");
            worker.paste().unwrap();
            let Completion::Packed(actual) = complete(&mut worker) else {
                panic!("missing packed tilemap in another process");
            };
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
            let bytes = wayland_tilemap::receive("image/png", 1024 * 1024)
                .unwrap()
                .unwrap();
            assert_eq!(
                decode_png(&bytes).unwrap(),
                expected.native_bitmap().unwrap()
            );
            println!("{CHILD_VERIFIED}");
            return;
        }
        worker.copy_content(&indexed_sample()).unwrap();
        assert!(matches!(complete(&mut worker), Completion::Copy(Ok(()))));
        run_clipboard_child(
            "wayland_packed_tilemap_cross_process_roundtrip",
            CHILD,
            "indexed",
        );
        worker.copy_content(&expected).unwrap();
        let Completion::Copy(result) = complete(&mut worker) else {
            panic!("missing copy result")
        };
        result.unwrap();
        // This recipient requests a payload larger than a pipe buffer and never reads.
        let (stalled, _) = wl_clipboard_rs::paste::get_contents(
            wl_clipboard_rs::paste::ClipboardType::Regular,
            wl_clipboard_rs::paste::Seat::Unspecified,
            wl_clipboard_rs::paste::MimeType::Specific(tilemap_wire::Flavor::Tilemap.mime()),
        )
        .unwrap();
        run_clipboard_child(
            "wayland_packed_tilemap_cross_process_roundtrip",
            CHILD,
            "tilemap",
        );
        drop(stalled);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "Writes the clipboard. Run against an isolated X11 display."]
    fn x11_packed_tilemap_cross_process_roundtrip() {
        use super::tilemap_wire::tests::{indexed_sample, sample};
        const CHILD: &str = "PIXELCRAFT_X11_CLIPBOARD_TEST_CHILD";
        assert!(std::env::var_os("WAYLAND_DISPLAY").is_none());
        assert!(std::env::var_os("DISPLAY").is_some());
        let mut expected = sample();
        // The opaque ASE user data makes this larger than one X11 property chunk.
        expected.tileset.as_mut().unwrap().user_data[2].extend(vec![0x77; 300_000]);
        let wait = |worker: &mut SystemClipboard| {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(result) = worker.poll() {
                    return result;
                }
                assert!(
                    Instant::now() < deadline,
                    "clipboard worker response timed out"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        let mut worker = SystemClipboard::default();
        if let Ok(kind) = std::env::var(CHILD) {
            let expected = if kind == "indexed" {
                indexed_sample()
            } else {
                expected
            };
            assert!(kind == "indexed" || kind == "tilemap");
            worker.paste().unwrap();
            let Completion::Packed(actual) = wait(&mut worker) else {
                panic!("missing packed tilemap in another process");
            };
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
            let (w, h, pixels) = expected.native_bitmap().unwrap();
            let bitmap = arboard::Clipboard::new().unwrap().get_image().unwrap();
            assert_eq!((bitmap.width, bitmap.height), (w, h));
            assert_eq!(bitmap.bytes.as_ref(), pixels.as_flattened());
            println!("{CHILD_VERIFIED}");
            return;
        }
        worker.copy_content(&indexed_sample()).unwrap();
        assert!(matches!(wait(&mut worker), Completion::Copy(Ok(()))));
        run_clipboard_child(
            "x11_packed_tilemap_cross_process_roundtrip",
            CHILD,
            "indexed",
        );
        worker.copy_content(&expected).unwrap();
        assert!(matches!(wait(&mut worker), Completion::Copy(Ok(()))));
        run_clipboard_child(
            "x11_packed_tilemap_cross_process_roundtrip",
            CHILD,
            "tilemap",
        );
        let (w, h, pixels) = expected.native_bitmap().unwrap();
        let mut external = arboard::Clipboard::new().unwrap();
        external
            .set_image(arboard::ImageData {
                width: w,
                height: h,
                bytes: std::borrow::Cow::Borrowed(pixels.as_flattened()),
            })
            .unwrap();
        worker.paste().unwrap();
        let Completion::Paste(Ok(actual)) = wait(&mut worker) else {
            panic!("ordinary PNG must replace the packed offer");
        };
        assert_eq!(actual, (w, h, pixels));
    }

    #[test]
    #[ignore = "Requires a native desktop clipboard; run explicitly on Linux and Windows"]
    fn native_clipboard_roundtrip() {
        let pixels = vec![[255, 0, 0, 255], [0, 255, 0, 255]];
        let mut clipboard = NativeClipboard::default();
        clipboard.set(2, 1, &pixels).unwrap();
        assert_eq!(clipboard.get().unwrap(), (2, 1, pixels));
    }
}
