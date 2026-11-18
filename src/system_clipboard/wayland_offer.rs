//! Publish Wayland data-control offers without blocking on clipboard recipients.
use super::wayland_tilemap::PNG_MIME;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop};
use wayland_protocols::ext::data_control::v1::client as ext;
use wayland_protocols_wlr::data_control::v1::client as wlr;

const OWNER_TIMEOUT: Duration = Duration::from_millis(1200);
const SEND_TIMEOUT: Duration = Duration::from_millis(1000);
const MAX_TRANSFERS: usize = 8;
static ACTIVE_TRANSFERS: AtomicUsize = AtomicUsize::new(0);
static INITIALIZING_OFFER: AtomicBool = AtomicBool::new(false);

struct Initialization;

impl Initialization {
    fn acquire() -> Result<Self, String> {
        INITIALIZING_OFFER
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| "Waiting for the Wayland compositor to respond".into())
    }
}

impl Drop for Initialization {
    fn drop(&mut self) {
        INITIALIZING_OFFER.store(false, Ordering::Release);
    }
}

struct TransferSlot;

impl TransferSlot {
    fn acquire() -> Option<Self> {
        ACTIVE_TRANSFERS
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |active| {
                (active < MAX_TRANSFERS).then_some(active + 1)
            })
            .ok()?;
        Some(Self)
    }
}

impl Drop for TransferSlot {
    fn drop(&mut self) {
        ACTIVE_TRANSFERS.fetch_sub(1, Ordering::Relaxed);
    }
}

struct Offer {
    packed_mime: &'static str,
    packed: Arc<[u8]>,
    png: Option<Arc<[u8]>>,
    sources: usize,
}

impl Offer {
    fn send(&self, mime_type: &str, fd: OwnedFd) {
        let bytes = match mime_type {
            kind if kind == self.packed_mime => &self.packed,
            PNG_MIME => {
                let Some(png) = &self.png else { return };
                png
            }
            _ => return,
        };
        let Some(slot) = TransferSlot::acquire() else {
            return;
        };
        let bytes = Arc::clone(bytes);
        // Each worker has a deadline and a slot shared by all offers in this process.
        let _ = std::thread::Builder::new()
            .name("wayland-clipboard-send".into())
            .spawn(move || {
                let _slot = slot;
                let _ = write_payload(File::from(fd), &bytes, SEND_TIMEOUT);
            });
    }
}

delegate_noop!(Offer: ignore wl_seat::WlSeat);

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Offer {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

macro_rules! implement_control {
    ($manager:ident :: $Manager:ident, $device:ident :: $Device:ident,
     $source:ident :: $Source:ident, $offer:ident :: $Offer:ident) => {
        delegate_noop!(Offer: ignore $manager::$Manager);
        delegate_noop!(Offer: ignore $offer::$Offer);

        impl Dispatch<$device::$Device, ()> for Offer {
            fn event(
                _: &mut Self, device: &$device::$Device, event: $device::Event, _: &(),
                _: &Connection, _: &QueueHandle<Self>,
            ) {
                match event {
                    $device::Event::Selection { id: Some(offer) }
                    | $device::Event::PrimarySelection { id: Some(offer) } => offer.destroy(),
                    $device::Event::Finished => device.destroy(),
                    _ => {}
                }
            }

            wayland_client::event_created_child!(Offer, $device::$Device, [
                $device::EVT_DATA_OFFER_OPCODE => ($offer::$Offer, ())
            ]);
        }

        impl Dispatch<$source::$Source, ()> for Offer {
            fn event(
                state: &mut Self, source: &$source::$Source, event: $source::Event, _: &(),
                _: &Connection, _: &QueueHandle<Self>,
            ) {
                match event {
                    $source::Event::Send { mime_type, fd } => state.send(&mime_type, fd),
                    $source::Event::Cancelled => {
                        source.destroy();
                        state.sources = state.sources.saturating_sub(1);
                    }
                    _ => {}
                }
            }
        }
    }
}

use ext::{
    ext_data_control_device_v1, ext_data_control_manager_v1, ext_data_control_offer_v1,
    ext_data_control_source_v1,
};
use wlr::{
    zwlr_data_control_device_v1, zwlr_data_control_manager_v1, zwlr_data_control_offer_v1,
    zwlr_data_control_source_v1,
};

implement_control!(
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_source_v1::ExtDataControlSourceV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1
);
implement_control!(
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1
);

fn prepare(state: &mut Offer) -> Result<Option<EventQueue<Offer>>, Box<dyn std::error::Error>> {
    let Ok(connection) = Connection::connect_to_env() else {
        return Ok(None);
    };
    let (globals, mut queue) = registry_queue_init::<Offer>(&connection)?;
    let handle = queue.handle();
    let seats: Vec<wl_seat::WlSeat> = globals.contents().with_list(|list| {
        list.iter()
            .filter(|global| global.interface == wl_seat::WlSeat::interface().name)
            .map(|global| globals.registry().bind(global.name, 1, &handle, ()))
            .collect()
    });
    if seats.is_empty() {
        return Ok(None);
    }

    macro_rules! offer_on_seats {
        ($manager:expr) => {
            for seat in &seats {
                let device = $manager.get_data_device(seat, &handle, ());
                let source = $manager.create_data_source(&handle, ());
                source.offer(state.packed_mime.into());
                if state.png.is_some() {
                    source.offer(PNG_MIME.into());
                }
                device.set_selection(Some(&source));
                state.sources += 1;
            }
        };
    }

    if let Ok(manager) = globals.bind::<ext_data_control_manager_v1::ExtDataControlManagerV1, _, _>(
        &handle,
        1..=1,
        (),
    ) {
        offer_on_seats!(manager);
    } else if let Ok(manager) = globals
        .bind::<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, _, _>(&handle, 1..=2, ())
    {
        offer_on_seats!(manager);
    } else {
        return Ok(None);
    }
    queue.roundtrip(state)?;
    if state.sources == 0 {
        return Err("The compositor declined the clipboard offer".into());
    }
    Ok(Some(queue))
}

fn serve(mut queue: EventQueue<Offer>, mut state: Offer, stop: UnixStream) {
    while state.sources != 0 {
        if queue.dispatch_pending(&mut state).is_err() || state.sources == 0 {
            break;
        }
        let Some(read) = queue.prepare_read() else {
            continue;
        };
        let mut events = libc::POLLIN;
        match queue.flush() {
            Ok(()) => {}
            Err(wayland_client::backend::WaylandError::Io(error))
                if error.kind() == io::ErrorKind::WouldBlock =>
            {
                events |= libc::POLLOUT
            }
            Err(_) => break,
        }
        let mut descriptors = [
            libc::pollfd {
                fd: read.connection_fd().as_raw_fd(),
                events,
                revents: 0,
            },
            libc::pollfd {
                fd: stop.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // The read guard and stop stream own both descriptors throughout the wait.
        let ready = unsafe { libc::poll(descriptors.as_mut_ptr(), 2, -1) };
        if ready < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        if descriptors[1].revents != 0 {
            break;
        }
        if descriptors[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            match read.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(error))
                    if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => break,
            }
        }
    }
}

pub(super) struct Owner {
    _stop: UnixStream,
}

pub(super) fn publish(
    packed_mime: &'static str,
    packed: Vec<u8>,
    png: Option<Vec<u8>>,
) -> Result<Option<Owner>, String> {
    let initialization = Initialization::acquire()?;
    let (stop, listener) = UnixStream::pair().map_err(|error| error.to_string())?;
    let (ready, response) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("wayland-clipboard-owner".into())
        .spawn(move || {
            let mut state = Offer {
                packed_mime,
                packed: packed.into(),
                png: png.map(Into::into),
                sources: 0,
            };
            let prepared = prepare(&mut state);
            drop(initialization);
            match prepared {
                Ok(Some(queue)) => {
                    if ready.send(Ok(true)).is_ok() {
                        serve(queue, state, listener);
                    }
                }
                Ok(None) => {
                    let _ = ready.send(Ok(false));
                }
                Err(error) => {
                    let _ = ready.send(Err(error.to_string()));
                }
            }
        })
        .map_err(|error| error.to_string())?;
    let published = response
        .recv_timeout(OWNER_TIMEOUT)
        .map_err(|error| error.to_string())??;
    Ok(published.then_some(Owner { _stop: stop }))
}

fn write_payload(mut pipe: File, mut bytes: &[u8], timeout: Duration) -> io::Result<()> {
    let fd = pipe.as_fd().as_raw_fd();
    // The pipe's write end belongs to this transfer. Preserve its existing status flags.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let deadline = Instant::now() + timeout;
    let mut descriptor = libc::pollfd {
        fd,
        events: libc::POLLOUT,
        revents: 0,
    };
    while !bytes.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Wayland clipboard recipient stopped reading",
            ));
        }
        match pipe.write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                // pipe keeps the descriptor alive for the wait.
                let ready =
                    unsafe { libc::poll(&mut descriptor, 1, remaining.as_millis() as i32 + 1) };
                if ready < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonreading_wayland_recipient_releases_its_worker() {
        let (_reader, writer) = UnixStream::pair().unwrap();
        let writer = File::from(OwnedFd::from(writer));
        let timeout = Duration::from_millis(50);
        let data = vec![7; 2 * 1024 * 1024];
        assert_eq!(
            write_payload(writer, &data, timeout).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }

    #[test]
    fn wayland_writer_delivers_full_payload_across_partial_writes() {
        use std::io::Read;
        let (mut reader, writer) = UnixStream::pair().unwrap();
        let read = std::thread::spawn(move || {
            let mut received = Vec::new();
            reader.read_to_end(&mut received).unwrap();
            received
        });
        let writer = File::from(OwnedFd::from(writer));
        let data = vec![9; 2 * 1024 * 1024];
        write_payload(writer, &data, SEND_TIMEOUT).unwrap();
        assert_eq!(read.join().unwrap(), data);
    }
}
