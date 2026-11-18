//! The Wayland data-control offer holds the packed map and its bitmap preview.
use super::Clipboard;
use super::tilemap_wire::{Flavor, decode, encode, preview_png};
use super::wayland_offer::{self, Owner};
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};
use wl_clipboard_rs::paste;

pub(super) const PNG_MIME: &str = "image/png";
const READ_TIMEOUT: Duration = Duration::from_millis(750);

pub(super) fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

pub(super) fn publish(clip: &Clipboard) -> Result<Option<Owner>, String> {
    let packed = encode(clip)?;
    let png = match clip.native_bitmap() {
        Ok((w, h, pixels)) => Some(preview_png(w, h, &pixels)?),
        Err(_) => None,
    };
    let flavor = Flavor::of(clip).ok_or("Image has no packed clipboard format")?;
    wayland_offer::publish(flavor.mime(), packed, png)
}

pub(super) fn read() -> Result<Option<Clipboard>, paste::Error> {
    for flavor in Flavor::ALL {
        if let Some(clip) =
            receive(flavor.mime(), flavor.limit())?.and_then(|data| decode(&data, flavor))
        {
            return Ok(Some(clip));
        }
    }
    Ok(None)
}

pub(super) fn receive(mime: &str, limit: usize) -> Result<Option<Vec<u8>>, paste::Error> {
    let pipe = match paste::get_contents(
        paste::ClipboardType::Regular,
        paste::Seat::Unspecified,
        paste::MimeType::Specific(mime),
    ) {
        Ok((pipe, _)) => pipe,
        Err(paste::Error::NoMimeType | paste::Error::ClipboardEmpty) => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(read_with_deadline(pipe, limit, READ_TIMEOUT).ok())
}

fn read_with_deadline(
    mut pipe: impl Read + AsRawFd,
    limit: usize,
    timeout: Duration,
) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut descriptor = libc::pollfd {
        fd: pipe.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let mut bytes = Vec::new();
    let mut chunk = [0; 16 * 1024];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Clipboard transfer timed out",
            ));
        }
        // The file descriptor stays owned by pipe during this call.
        let readiness = unsafe { libc::poll(&mut descriptor, 1, remaining.as_millis() as i32 + 1) };
        match readiness {
            -1 => {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            0 => continue,
            _ => {}
        }
        let read = pipe.read(&mut chunk)?;
        if read == 0 {
            return Ok(bytes);
        }
        if read > limit.saturating_sub(bytes.len()) {
            return Err(io::Error::other("Clipboard data exceeds limits"));
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    #[test]
    fn clipboard_pipe_rejects_oversized_and_stalled_transfers() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(b"12345").unwrap();
        assert_eq!(
            read_with_deadline(reader, 4, READ_TIMEOUT)
                .unwrap_err()
                .to_string(),
            "Clipboard data exceeds limits"
        );
        let (reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(b"1234").unwrap();
        let timeout = Duration::from_millis(50);
        assert_eq!(
            read_with_deadline(reader, 4, timeout).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        let (reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(b"1234").unwrap();
        drop(writer);
        assert_eq!(
            read_with_deadline(reader, 4, READ_TIMEOUT).unwrap(),
            b"1234"
        );
    }
}
