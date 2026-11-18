//! X11 CLIPBOARD selection with a packed tilemap and standard PNG.
use super::Clipboard;
use super::tilemap_wire::{Flavor, decode, encode, preview_png};
use std::collections::HashMap;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, CreateWindowAux, EventMask, PropMode,
    Property, SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Window,
    WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

const TIMEOUT: Duration = Duration::from_millis(1200);
const MAX_TRANSFERS: usize = 16;
const CHUNK_BYTES: usize = 64 * 1024;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        CLIPBOARD,
        TARGETS,
        TIMESTAMP,
        MULTIPLE,
        ATOM_PAIR,
        INCR,
        PIXELCRAFT_CLIPBOARD,
        TILEMAP: b"application/vnd.pixelcraft.tilemap",
        INDEXED: b"application/vnd.pixelcraft.indexed-raster",
        PNG: b"image/png",
    }
}

struct Selection {
    conn: RustConnection,
    window: Window,
    atoms: Atoms,
}

impl Selection {
    fn target(&self, flavor: Flavor) -> Atom {
        match flavor {
            Flavor::Tilemap => self.atoms.TILEMAP,
            Flavor::Indexed => self.atoms.INDEXED,
        }
    }
    fn connect() -> Result<Self> {
        let (conn, screen) = x11rb::connect(None)?;
        let window = conn.generate_id()?;
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            conn.setup().roots[screen].root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?
        .check()?;
        let atoms = Atoms::new(&conn)?.reply()?;
        Ok(Self {
            conn,
            window,
            atoms,
        })
    }

    fn next_event(&self, deadline: Instant) -> Result<Event> {
        let mut descriptor = poll_descriptor(self.conn.stream().as_raw_fd());
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("X11 clipboard request timed out".into());
            }
            if let Some(event) = self.conn.poll_for_event()? {
                return Ok(event);
            }
            poll(std::slice::from_mut(&mut descriptor), remaining)?;
        }
    }

    fn claim(&self, deadline: Instant) -> Result<u32> {
        // A PropertyNotify supplies a current timestamp from the X server.
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.window,
                self.atoms.PIXELCRAFT_CLIPBOARD,
                AtomEnum::STRING,
                b"",
            )?
            .check()?;
        let timestamp = loop {
            if let Event::PropertyNotify(event) = self.next_event(deadline)?
                && event.window == self.window
                && event.atom == self.atoms.PIXELCRAFT_CLIPBOARD
            {
                break event.time;
            }
        };
        self.conn
            .set_selection_owner(self.window, self.atoms.CLIPBOARD, timestamp)?
            .check()?;
        if self
            .conn
            .get_selection_owner(self.atoms.CLIPBOARD)?
            .reply()?
            .owner
            != self.window
        {
            return Err("Could not acquire the X11 clipboard".into());
        }
        Ok(timestamp)
    }

    fn receive(&self, target: Atom, limit: usize) -> Result<Option<Vec<u8>>> {
        let property = self.atoms.PIXELCRAFT_CLIPBOARD;
        let deadline = Instant::now() + TIMEOUT;
        self.conn.convert_selection(
            self.window,
            self.atoms.CLIPBOARD,
            target,
            property,
            x11rb::CURRENT_TIME,
        )?;
        self.conn.flush()?;
        let mut incremental = false;
        let mut bytes = Vec::new();
        loop {
            match self.next_event(deadline)? {
                Event::SelectionNotify(event)
                    if !incremental
                        && event.requestor == self.window
                        && event.selection == self.atoms.CLIPBOARD
                        && event.target == target =>
                {
                    if event.property != property {
                        return Ok(None);
                    }
                    let reply = self
                        .conn
                        .get_property(
                            true,
                            self.window,
                            property,
                            AtomEnum::ANY,
                            0,
                            limit.div_ceil(4).max(1) as u32,
                        )?
                        .reply()?;
                    if reply.type_ == self.atoms.INCR
                        && reply.format == 32
                        && reply.value_len <= 1
                        && reply.bytes_after == 0
                    {
                        let minimum = reply
                            .value32()
                            .and_then(|mut values| values.next())
                            .unwrap_or(0);
                        if minimum as usize > limit {
                            return Err("Clipboard data exceeds limits".into());
                        }
                        incremental = true;
                    } else {
                        validate_property(&reply, target, limit)?;
                        return Ok(Some(reply.value));
                    }
                }
                Event::PropertyNotify(event)
                    if incremental
                        && event.window == self.window
                        && event.atom == property
                        && event.state == Property::NEW_VALUE =>
                {
                    let remaining = limit - bytes.len();
                    let reply = self
                        .conn
                        .get_property(
                            true,
                            self.window,
                            property,
                            target,
                            0,
                            remaining.div_ceil(4).max(1) as u32,
                        )?
                        .reply()?;
                    validate_property(&reply, target, remaining)?;
                    if reply.value.is_empty() {
                        return Ok(Some(bytes));
                    }
                    bytes.extend_from_slice(&reply.value);
                }
                _ => {}
            }
        }
    }
}

fn validate_property(
    reply: &x11rb::protocol::xproto::GetPropertyReply,
    target: Atom,
    limit: usize,
) -> Result<()> {
    if reply.bytes_after != 0 || reply.value.len() > limit {
        return Err("Clipboard data exceeds limits".into());
    }
    if reply.type_ != target || reply.format != 8 {
        return Err("Invalid X11 clipboard property type".into());
    }
    Ok(())
}

fn poll_descriptor(fd: i32) -> libc::pollfd {
    libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    }
}

fn poll(descriptors: &mut [libc::pollfd], duration: Duration) -> io::Result<()> {
    // Every descriptor is borrowed from an object that stays alive during the wait.
    let ready = unsafe {
        libc::poll(
            descriptors.as_mut_ptr(),
            descriptors.len() as libc::nfds_t,
            (duration.as_millis() as i32).max(1),
        )
    };
    if ready < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    Ok(())
}

struct Transfer {
    target: Atom,
    offset: usize,
    expires: Instant,
}

struct Server {
    selection: Selection,
    timestamp: u32,
    data: [(Atom, Vec<u8>); 2],
    pending: HashMap<(Window, Atom), Transfer>,
    chunk_bytes: usize,
}

impl Server {
    fn new(
        selection: Selection,
        timestamp: u32,
        flavor: Flavor,
        packed: Vec<u8>,
        png: Vec<u8>,
    ) -> Self {
        let data = [
            (selection.target(flavor), packed),
            (selection.atoms.PNG, png),
        ];
        let max_request = usize::from(selection.conn.setup().maximum_request_length) * 4;
        Self {
            selection,
            timestamp,
            data,
            pending: HashMap::new(),
            chunk_bytes: CHUNK_BYTES.min(max_request.saturating_sub(24)).max(1),
        }
    }

    fn offer(&mut self, requestor: Window, target: Atom, property: Atom) -> Result<()> {
        let Selection { conn, atoms, .. } = &self.selection;
        if self.pending.contains_key(&(requestor, property)) {
            return Err("X11 clipboard property is already in use".into());
        }
        if target == atoms.TARGETS {
            let mut targets = vec![
                atoms.TARGETS,
                atoms.TIMESTAMP,
                atoms.MULTIPLE,
                self.data[0].0,
            ];
            if !self.data[1].1.is_empty() {
                targets.push(atoms.PNG);
            }
            conn.change_property32(
                PropMode::REPLACE,
                requestor,
                property,
                AtomEnum::ATOM,
                &targets,
            )?
            .check()?;
        } else if target == atoms.TIMESTAMP {
            conn.change_property32(
                PropMode::REPLACE,
                requestor,
                property,
                AtomEnum::INTEGER,
                &[self.timestamp],
            )?
            .check()?;
        } else if let Some((_, bytes)) = self
            .data
            .iter()
            .find(|(kind, bytes)| *kind == target && !bytes.is_empty())
        {
            if bytes.len() <= self.chunk_bytes {
                conn.change_property8(PropMode::REPLACE, requestor, property, target, bytes)?
                    .check()?;
            } else {
                if self.pending.len() >= MAX_TRANSFERS {
                    return Err("Too many X11 clipboard transfers".into());
                }
                conn.change_window_attributes(
                    requestor,
                    &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
                )?
                .check()?;
                let start = (|| -> Result<()> {
                    conn.change_property32(
                        PropMode::REPLACE,
                        requestor,
                        property,
                        atoms.INCR,
                        &[bytes.len() as u32],
                    )?
                    .check()?;
                    Ok(())
                })();
                if let Err(error) = start {
                    self.unwatch(requestor);
                    return Err(error);
                }
                self.pending.insert(
                    (requestor, property),
                    Transfer {
                        target,
                        offset: 0,
                        expires: Instant::now() + TIMEOUT,
                    },
                );
            }
        } else {
            return Err("Unsupported X11 clipboard target".into());
        }
        Ok(())
    }

    fn multiple(&mut self, requestor: Window, property: Atom) -> Result<()> {
        let atoms = &self.selection.atoms;
        let pair = atoms.ATOM_PAIR;
        let multiple = atoms.MULTIPLE;
        let reply = self
            .selection
            .conn
            .get_property(false, requestor, property, pair, 0, 256)?
            .reply()?;
        let values = reply.value32().ok_or("Invalid X11 clipboard pair list")?;
        let mut pairs: Vec<_> = values.collect();
        if reply.type_ != pair || reply.bytes_after != 0 || pairs.len() % 2 != 0 {
            return Err("Invalid X11 clipboard pair list".into());
        }
        for entry in pairs.chunks_exact_mut(2) {
            let [target, output] = [entry[0], entry[1]];
            if output == x11rb::NONE
                || output == property
                || target == multiple
                || self.offer(requestor, target, output).is_err()
            {
                entry[1] = x11rb::NONE;
            }
        }
        self.selection
            .conn
            .change_property32(PropMode::REPLACE, requestor, property, pair, &pairs)?
            .check()?;
        Ok(())
    }

    fn request(&mut self, event: SelectionRequestEvent, accepting: bool) -> Result<()> {
        let property = if event.property == x11rb::NONE {
            event.target
        } else {
            event.property
        };
        let mut reply = x11rb::NONE;
        if accepting
            && event.selection == self.selection.atoms.CLIPBOARD
            && event.owner == self.selection.window
            && (event.time == x11rb::CURRENT_TIME
                || event.time.wrapping_sub(self.timestamp) <= i32::MAX as u32)
        {
            let offered = if event.target == self.selection.atoms.MULTIPLE {
                self.multiple(event.requestor, property)
            } else {
                self.offer(event.requestor, event.target, property)
            };
            if offered.is_ok() {
                reply = property;
            }
        }
        self.selection.conn.send_event(
            false,
            event.requestor,
            EventMask::NO_EVENT,
            SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: event.time,
                requestor: event.requestor,
                selection: event.selection,
                target: event.target,
                property: reply,
            },
        )?;
        self.selection.conn.flush()?;
        Ok(())
    }

    fn unwatch(&self, window: Window) {
        if !self
            .pending
            .keys()
            .any(|(requestor, _)| *requestor == window)
        {
            let _ = self.selection.conn.change_window_attributes(
                window,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
            );
        }
    }

    fn advance(&mut self, window: Window, property: Atom) -> Result<()> {
        let Some(mut state) = self.pending.remove(&(window, property)) else {
            return Ok(());
        };
        let bytes = &self.data[usize::from(state.target == self.selection.atoms.PNG)].1;
        let end = (state.offset + self.chunk_bytes).min(bytes.len());
        let chunk = &bytes[state.offset..end];
        let sent = self
            .selection
            .conn
            .change_property8(PropMode::REPLACE, window, property, state.target, chunk)?
            .check();
        if sent.is_ok() && !chunk.is_empty() {
            state.offset = end;
            state.expires = Instant::now() + TIMEOUT;
            self.pending.insert((window, property), state);
        } else {
            self.unwatch(window);
        }
        self.selection.conn.flush()?;
        sent?;
        Ok(())
    }

    fn serve(mut self, stop: UnixStream) -> Result<()> {
        let mut descriptors = [
            poll_descriptor(self.selection.conn.stream().as_raw_fd()),
            poll_descriptor(stop.as_raw_fd()),
        ];
        let mut accepting = true;
        loop {
            let now = Instant::now();
            let expired: Vec<_> = self
                .pending
                .iter()
                .filter_map(|(key, transfer)| (transfer.expires <= now).then_some(*key))
                .collect();
            for key in expired {
                self.pending.remove(&key);
                self.unwatch(key.0);
            }
            if !accepting && self.pending.is_empty() {
                return Ok(());
            }
            self.selection.conn.flush()?;
            // Bounded batches also give cancellation and expiry a chance during event floods.
            let mut wait = Duration::ZERO;
            for _ in 0..64 {
                let Some(event) = self.selection.conn.poll_for_event()? else {
                    wait = self
                        .pending
                        .values()
                        .map(|transfer| transfer.expires.saturating_duration_since(Instant::now()))
                        .min()
                        .unwrap_or(TIMEOUT);
                    break;
                };
                match event {
                    Event::SelectionClear(event)
                        if event.selection == self.selection.atoms.CLIPBOARD =>
                    {
                        // Previously accepted INCR requests can finish after ownership changes.
                        accepting = false;
                    }
                    Event::SelectionRequest(event) => {
                        let _ = self.request(event, accepting);
                    }
                    Event::PropertyNotify(event) if event.state == Property::DELETE => {
                        let _ = self.advance(event.window, event.atom);
                    }
                    _ => {}
                }
            }
            if !accepting && self.pending.is_empty() {
                return Ok(());
            }
            poll(&mut descriptors, wait)?;
            if descriptors[1].revents != 0 {
                accepting = false;
                descriptors[1].fd = -1;
            }
        }
    }
}

pub(super) struct Owner {
    _stop: UnixStream,
}

pub(super) fn publish(clip: &Clipboard) -> std::result::Result<Owner, String> {
    let packed = encode(clip)?;
    let flavor = Flavor::of(clip).ok_or("Image has no packed clipboard format")?;
    let png = match clip.native_bitmap() {
        Ok((w, h, pixels)) => preview_png(w, h, &pixels)?,
        Err(_) => Vec::new(),
    };
    let (stop, listener) = UnixStream::pair().map_err(|error| error.to_string())?;
    let (ready, response) = mpsc::sync_channel(1);
    let deadline = Instant::now() + TIMEOUT;
    std::thread::Builder::new()
        .name("x11-clipboard-owner".into())
        .spawn(move || {
            let server = (|| -> Result<Server> {
                let selection = Selection::connect()?;
                let timestamp = selection.claim(deadline)?;
                Ok(Server::new(selection, timestamp, flavor, packed, png))
            })();
            match server {
                Ok(server) if ready.send(Ok(())).is_ok() => {
                    let _ = server.serve(listener);
                }
                Ok(_) => {}
                Err(error) => {
                    let _ = ready.send(Err(error.to_string()));
                }
            }
        })
        .map_err(|error| error.to_string())?;
    response
        .recv_timeout(TIMEOUT)
        .map_err(|error| error.to_string())??;
    Ok(Owner { _stop: stop })
}

pub(super) fn read() -> Option<Clipboard> {
    let selection = Selection::connect().ok()?;
    for flavor in Flavor::ALL {
        let bytes = selection
            .receive(selection.target(flavor), flavor.limit())
            .ok()?;
        if let Some(clip) = bytes.and_then(|bytes| decode(&bytes, flavor)) {
            return Some(clip);
        }
    }
    None
}
