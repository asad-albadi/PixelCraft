//! Native RGBA clipboard access. X11/Windows/macOS use arboard; Wayland
//! uses the desktop's wl-clipboard helpers when available, with XWayland fallback.
use pixelcraft::model::{MAX_DIMENSION, MAX_PIXELS, Pixel};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(2);
type ClipboardImage = (usize, usize, Vec<Pixel>);
pub enum Completion {
    Copy(Result<(), String>),
    Paste(Result<ClipboardImage, String>),
}
enum Operation {
    Copy(usize, usize, Vec<Pixel>),
    Paste,
}
struct Job {
    started: Instant,
    response: Receiver<Completion>,
    paste: bool,
    expired: bool,
}
/// One native owner and at most one in-flight request, even after a timeout.
#[derive(Default)]
pub struct SystemClipboard {
    worker: Option<SyncSender<(Operation, SyncSender<Completion>)>>,
    job: Option<Job>,
}
impl SystemClipboard {
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
                        let result = match operation {
                            Operation::Copy(w, h, pixels) => {
                                Completion::Copy(native.set(w, h, &pixels))
                            }
                            Operation::Paste => Completion::Paste(native.get()),
                        };
                        let _ = response.send(result);
                    }
                })
                .map_err(|e| e.to_string())?;
            self.worker = Some(tx);
        }
        let paste = matches!(operation, Operation::Paste);
        let (tx, rx) = mpsc::sync_channel(1);
        self.worker
            .as_ref()
            .unwrap()
            .try_send((operation, tx))
            .map_err(|e| e.to_string())?;
        self.job = Some(Job {
            started: Instant::now(),
            response: rx,
            paste,
            expired: false,
        });
        Ok(())
    }
    pub fn copy(&mut self, w: usize, h: usize, pixels: Vec<Pixel>) -> Result<(), String> {
        self.start(Operation::Copy(w, h, pixels))
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
            let error = Err("System clipboard timed out".into());
            return Some(if job.paste {
                Completion::Paste(error)
            } else {
                Completion::Copy(error.map(|_| ()))
            });
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
                let paste = job.paste;
                self.job = None;
                self.worker = None;
                if expired {
                    None
                } else if paste {
                    Some(Completion::Paste(Err("Clipboard worker stopped".into())))
                } else {
                    Some(Completion::Copy(Err("Clipboard worker stopped".into())))
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn run_helper(
    mut command: std::process::Command,
    input: Option<Vec<u8>>,
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
                let limit = (MAX_PIXELS * 4 + 1024 * 1024) as u64;
                let mut bytes = vec![];
                if let Some(stdout) = stdout {
                    stdout
                        .take(limit + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                }
                if bytes.len() as u64 > limit {
                    return Err("Clipboard image exceeds limits".into());
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
}
impl NativeClipboard {
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
            match run_helper(command, Some(bytes.into_inner())) {
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
            match run_helper(command, None) {
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
                paste: true,
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
                run_helper(command, input)
                    .unwrap_err()
                    .contains("timed out")
            );
            assert!(start.elapsed() < CLIPBOARD_TIMEOUT);
        }
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
