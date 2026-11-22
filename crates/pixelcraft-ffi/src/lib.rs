//! Bounded tablet proof-of-concept ABI. See include/pixelcraft.h for ownership.
//! All calls and buffer access for an owner must be serialized. No Rust object
//! layout or foreign input pointer crosses the ABI.
use pixelcraft_core::{editor::EditorSession, formats, model::Document};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, OnceLock},
};

mod commands;
mod recovery;
mod selection;

const MAX_BUFFER: usize = 8 * 1024 * 1024;
const MAX_POOL: usize = 32 * 1024 * 1024;
const MAX_SESSIONS: usize = 2;
const MAX_SIDE: usize = 256;
const RASTER_BUDGET: usize = 16 * 1024 * 1024;
struct Buffer {
    bytes: Box<[u8]>,
    kind: u32,
}
struct Stroke {
    base: Document,
    last: (i32, i32),
    color: [u8; 4],
    size: i32,
}
struct MobileSession {
    editor: EditorSession,
    stroke: Option<Stroke>,
    revision: u32,
    raster_pixels: usize,
    clipboard: Option<selection::RasterClipboard>,
}
#[derive(Default)]
struct Registry {
    next: u64,
    sessions: HashMap<u64, MobileSession>,
    buffers: HashMap<u64, Buffer>,
    bytes: usize,
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
fn guarded<T: Default>(f: impl FnOnce(&mut Registry) -> T) -> T {
    catch_unwind(AssertUnwindSafe(|| {
        f(&mut registry().lock().unwrap_or_else(|p| p.into_inner()))
    }))
    .unwrap_or_default()
}
impl Registry {
    fn id(&mut self) -> Result<u64, String> {
        self.next = self.next.checked_add(1).ok_or("Handle space exhausted")?;
        Ok(self.next)
    }
    fn buffer(&mut self, bytes: Vec<u8>, kind: u32) -> u64 {
        if bytes.len() > MAX_BUFFER
            || self.buffers.len() >= 64
            || self.bytes.saturating_add(bytes.len()) > MAX_POOL
        {
            return 0;
        }
        let Ok(id) = self.id() else {
            return 0;
        };
        self.bytes += bytes.len();
        self.buffers.insert(
            id,
            Buffer {
                bytes: bytes.into_boxed_slice(),
                kind,
            },
        );
        id
    }
    fn request(
        &mut self,
        h: u64,
        op: u32,
        a: u32,
        b: u32,
        input: u64,
    ) -> Result<(Vec<u8>, u32), String> {
        let input = if input == 0 {
            vec![]
        } else {
            let buf = self.buffers.get(&input).ok_or("Invalid input buffer")?;
            if buf.kind != 4 {
                return Err("Expected a caller-owned input buffer".into());
            }
            buf.bytes.to_vec()
        };
        let h = if h == 0 {
            if !matches!(op, 0 | 1 | 6) {
                return Err("Missing editor session".into());
            }
            if self.sessions.len() >= MAX_SESSIONS {
                return Err("Too many editor sessions".into());
            }
            let doc = document(op, a, b, &input)?;
            let layer = if op == 1 { 1 } else { 0 };
            let id = self.id()?;
            self.sessions.insert(
                id,
                MobileSession {
                    editor: EditorSession {
                        layer,
                        ..EditorSession::new(doc)
                    },
                    stroke: None,
                    revision: 1,
                    raster_pixels: RASTER_BUDGET / 4,
                    clipboard: None,
                },
            );
            return Ok((
                serde_json::to_vec(&self.sessions[&id].state(id)).map_err(|e| e.to_string())?,
                1,
            ));
        } else {
            h
        };
        let session = self
            .sessions
            .get_mut(&h)
            .ok_or("Editor session is closed")?;
        if session.stroke.is_some() && !matches!(op, 2 | 7 | 21) {
            return Err("Finish the active stroke first".into());
        }
        match op {
            0 | 1 | 6 => {
                let doc = document(op, a, b, &input)?;
                let revision = session.next_revision()?;
                session.editor = EditorSession::new(doc);
                if op == 1 {
                    session.editor.layer = 1;
                }
                session.revision = revision;
            }
            2 => {}
            3 => {
                let next = session.next_revision()?;
                session
                    .editor
                    .jump_history(session.editor.undo.len().saturating_sub(1));
                session.revision = next;
            }
            4 => {
                let next = session.next_revision()?;
                session.editor.jump_history(session.editor.undo.len() + 1);
                session.revision = next;
            }
            5 => return Ok((encode_mobile(&session.editor.doc)?, 3)),
            20 => selection::apply(session, &input)?,
            17 => commands::apply(session, &input)?,
            18 => {
                use pixelcraft_core::model::BrushShape;
                let shape = match a {
                    0 => BrushShape::Round,
                    1 => BrushShape::Square,
                    2 => BrushShape::Diamond,
                    3 => BrushShape::Horizontal,
                    4 => BrushShape::Vertical,
                    _ => return Err("Unknown brush shape".into()),
                };
                session.editor.doc.brush_shape = shape;
                session.editor.saved.brush_shape = shape;
                for entry in session
                    .editor
                    .undo
                    .iter_mut()
                    .chain(session.editor.redo.iter_mut())
                {
                    entry.doc.brush_shape = shape;
                }
            }
            19 => {
                if a as usize >= session.editor.doc.width || b as usize >= session.editor.doc.height
                {
                    return Err("Sample outside canvas".into());
                }
                let color = session.editor.doc.composite(session.editor.frame)
                    [b as usize * session.editor.doc.width + a as usize];
                return Ok((
                    serde_json::to_vec(&json!({"color":color})).map_err(|e| e.to_string())?,
                    6,
                ));
            }
            15 => return Ok((recovery::encode(&session.editor)?, 5)),
            16 => {
                let recovered = recovery::decode(&input)?;
                let revision = session.next_revision()?;
                session.editor = recovered;
                session.revision = revision;
            }
            7 => {
                let frame = if b == 1 {
                    a as usize
                } else {
                    session.editor.frame
                };
                if frame >= session.editor.doc.durations.len() {
                    return Err("Invalid preview frame".into());
                }
                return Ok((
                    session
                        .editor
                        .doc
                        .composite(frame)
                        .into_iter()
                        .flatten()
                        .collect(),
                    2,
                ));
            }
            21 => {
                if a > 8 || b > 8 {
                    return Err("Choose at most8 neighboring onion frames".into());
                }
                let options = pixelcraft_core::productivity::OnionOptions {
                    previous: a as usize,
                    next: b as usize,
                    ..Default::default()
                };
                return Ok((
                    pixelcraft_core::productivity::onion_pixels(
                        &session.editor.doc,
                        session.editor.frame,
                        &options,
                    )
                    .into_iter()
                    .flatten()
                    .collect(),
                    2,
                ));
            }
            22 => {
                let (sequence, repeat) = if b == 1 {
                    let tag = session
                        .editor
                        .doc
                        .tags
                        .get(a as usize)
                        .ok_or("Unknown animation tag")?;
                    (pixelcraft_core::productivity::tag_frames(tag), tag.repeat)
                } else {
                    ((0..session.editor.doc.durations.len()).collect(), 0)
                };
                let durations = sequence
                    .iter()
                    .map(|f| session.editor.doc.durations[*f])
                    .collect::<Vec<_>>();
                return Ok((
                    serde_json::to_vec(
                        &json!({"sequence":sequence,"durations":durations,"repeat":repeat}),
                    )
                    .map_err(|e| e.to_string())?,
                    6,
                ));
            }
            9 => {
                if a as usize >= session.editor.doc.durations.len() {
                    return Err("Invalid frame".into());
                }
                session.editor.frame = a as usize;
            }
            10 => {
                if a as usize >= session.editor.doc.layers.len() {
                    return Err("Choose an image layer".into());
                }
                session.editor.layer = a as usize;
            }
            12 => {
                if a != session.revision {
                    return Err("Artwork changed while saving".into());
                }
                session.editor.saved = session.editor.doc.clone();
            }
            _ => return Err("Unsupported editor operation".into()),
        }
        Ok((
            serde_json::to_vec(&session.state(h)).map_err(|e| e.to_string())?,
            1,
        ))
    }
}
fn encode_mobile(doc: &Document) -> Result<Vec<u8>, String> {
    let bytes = formats::encode_project(doc)?;
    if bytes.len() > MAX_BUFFER {
        return Err("Project exceeds the tablet preview save budget".into());
    }
    // Never publish a project that this same preview cannot reopen.
    formats::decode_project_bounded(&bytes, 2 * 1024 * 1024, RASTER_BUDGET)?;
    Ok(bytes)
}

fn document(op: u32, width: u32, height: u32, bytes: &[u8]) -> Result<Document, String> {
    let doc = match op {
        0 => {
            if width == 0 || height == 0 || width as usize > MAX_SIDE || height as usize > MAX_SIDE
            {
                return Err(
                    "Choose dimensions from 1 to 256 pixels for this tablet preview".into(),
                );
            }
            Document::new(width as usize, height as usize)
        }
        1 => Document::demo(),
        6 => formats::decode_project_bounded(bytes, 2 * 1024 * 1024, RASTER_BUDGET)?,
        _ => return Err("Unsupported document operation".into()),
    };
    check_document(&doc)?;
    Ok(doc)
}
fn check_document(doc: &Document) -> Result<(), String> {
    if doc.width > MAX_SIDE
        || doc.height > MAX_SIDE
        || doc.layers.len() > 32
        || doc.durations.len() > 64
        || doc.stored_pixels() > RASTER_BUDGET / 4
    {
        return Err(
            "Project exceeds the tablet preview limits (256 px, 32 layers, 64 frames)".into(),
        );
    }
    if doc.layers.iter().any(|layer| layer.name.len() > 256) {
        return Err("Layer names exceed the tablet preview metadata budget".into());
    }
    if doc.palette.len() > 256
        || doc.palette_names.len() > 256
        || doc.palette_names.iter().any(|s| s.len() > 128)
        || doc.tags.len() > 64
        || doc.tags.iter().any(|t| t.name.len() > 128)
        || (0..doc.durations.len()).any(|f| doc.frame_name(f).is_some_and(|n| n.len() > 128))
    {
        return Err("Project exceeds the mobile palette/tag/name budget".into());
    }
    doc.validate()?;
    Ok(())
}
impl MobileSession {
    fn commit(&mut self, doc: Document, label: &'static str) -> Result<(), String> {
        check_document(&doc)?;
        encode_mobile(&doc)?;
        if doc == self.editor.doc {
            return Ok(());
        }
        let revision = self.next_revision()?;
        self.editor.redo.clear();
        self.editor.push_undo(self.editor.doc.clone(), label);
        self.editor.doc = doc;
        self.editor.layer = self.editor.layer.min(self.editor.doc.layers.len() - 1);
        self.editor.frame = self.editor.frame.min(self.editor.doc.durations.len() - 1);
        self.revision = revision;
        while self.editor.undo.len() > 1
            && (self.editor.undo.len() > 32 || self.editor.history_bytes() > 32 * 1024 * 1024)
        {
            self.editor.undo.remove(0);
        }
        Ok(())
    }
    fn next_revision(&self) -> Result<u32, String> {
        self.revision
            .checked_add(1)
            .ok_or_else(|| "Editor revision exhausted; save and reopen".into())
    }
    fn state(&self, h: u64) -> Value {
        json!({"session":h,"revision":self.revision,"width":self.editor.doc.width,"height":self.editor.doc.height,"frame":self.editor.frame,"frames":self.editor.doc.durations.len(),"durations":self.editor.doc.durations,"tags":self.editor.doc.tags,"mode":self.editor.doc.color_mode,"palette":self.editor.doc.palette,"palette_names":self.editor.doc.palette_names,"frame_names":(0..self.editor.doc.durations.len()).map(|f|self.editor.doc.frame_name(f).unwrap_or("")).collect::<Vec<_>>(),"layer":self.editor.layer,"layers":self.editor.doc.layers.iter().map(|l| json!({"name":l.name,"visible":l.visible,"locked":l.locked,"group":l.group,"opacity":l.opacity,"linked":l.cel_meta[self.editor.frame].artwork_link.and_then(|id|l.cel_meta.iter().enumerate().find(|(f,m)|*f!=self.editor.frame&&m.artwork_link==Some(id)).map(|(f,_)|f)),"blend":l.blend})).collect::<Vec<_>>(),"undo":self.editor.undo.len(),"redo":self.editor.redo.len(),"brush_shape":self.editor.doc.brush_shape,"selection":self.editor.selection,"has_clipboard":self.clipboard.is_some(),"dirty":self.editor.doc != self.editor.saved})
    }
    fn stroke(
        &mut self,
        phase: u32,
        points: &[(i32, i32)],
        color: u32,
        size: u32,
    ) -> Result<(), String> {
        if phase > 3 {
            return Err("Invalid stroke phase".into());
        }
        if phase == 3 {
            let next = self.next_revision()?;
            if let Some(stroke) = self.stroke.take() {
                self.editor.doc = stroke.base;
                self.revision = next;
            }
            return Ok(());
        }
        if points.iter().any(|&(x, y)| {
            x < 0
                || y < 0
                || x as usize >= self.editor.doc.width
                || y as usize >= self.editor.doc.height
        }) {
            return Err("Stroke is outside the canvas".into());
        }
        if phase == 0 {
            if self.stroke.is_some() || points.is_empty() || size == 0 || size > 32 {
                return Err("Invalid stroke start".into());
            }
            let l = self.editor.layer;
            if !self.editor.doc.editable(l)
                || self.editor.doc.layers[l].group
                || self.editor.doc.layers[l].tileset.is_some()
            {
                return Err("Choose an unlocked image layer".into());
            }
        } else if self.stroke.is_none() {
            return Err("No active stroke".into());
        }
        let next = self.next_revision()?;
        let checkpoint = self.editor.doc.clone();
        if phase == 0 {
            self.stroke = Some(Stroke {
                base: self.editor.doc.clone(),
                last: points[0],
                color: color.to_le_bytes(),
                size: size as i32,
            });
        }
        let mask = selection::brush_mask(&self.editor);
        let stroke = self.stroke.as_mut().expect("validated stroke");
        let previous_point = stroke.last;
        let options = pixelcraft_core::productivity::BrushOptions {
            mask,
            ..Default::default()
        };
        for &point in points {
            if options.mask.is_some() {
                let _: Result<(), std::convert::Infallible> =
                    pixelcraft_core::model::line_pixels(stroke.last, point, |p| {
                        pixelcraft_core::productivity::paint_brush(
                            &mut self.editor.doc,
                            self.editor.layer,
                            self.editor.frame,
                            p,
                            stroke.color,
                            stroke.size,
                            &options,
                        );
                        Ok(())
                    });
            } else {
                self.editor.doc.line(
                    self.editor.layer,
                    self.editor.frame,
                    stroke.last,
                    point,
                    stroke.color,
                    stroke.size,
                );
            }
            stroke.last = point;
        }
        if self.editor.doc.stored_pixels() > self.raster_pixels {
            self.editor.doc = checkpoint;
            stroke.last = previous_point;
            if phase == 0 {
                self.stroke = None;
            }
            return Err("Stroke exceeds the tablet preview memory budget".into());
        }
        self.revision = next;
        if phase == 2 {
            let stroke = self.stroke.take().expect("validated stroke");
            if self.editor.doc != stroke.base {
                if let Err(error) = encode_mobile(&self.editor.doc) {
                    self.editor.doc = stroke.base;
                    return Err(error);
                }
                self.editor.redo.clear();
                self.editor.push_undo(stroke.base, "Draw");
                while self.editor.undo.len() > 1
                    && (self.editor.undo.len() > 32
                        || self.editor.history_bytes() > 32 * 1024 * 1024)
                {
                    self.editor.undo.remove(0);
                }
            }
        }
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pc_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_buffer_alloc(size: usize) -> u64 {
    guarded(|r| {
        if size <= MAX_BUFFER && r.buffers.len() < 64 && r.bytes.saturating_add(size) <= MAX_POOL {
            r.buffer(vec![0; size], 4)
        } else {
            0
        }
    })
}
/// Borrowed bytes; valid until free. The owner must serialize reads/writes/free.
#[unsafe(no_mangle)]
pub extern "C" fn pc_buffer_data(id: u64) -> *const u8 {
    guarded(|r| {
        r.buffers
            .get(&id)
            .map_or(std::ptr::null(), |b| b.bytes.as_ptr())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_buffer_len(id: u64) -> usize {
    guarded(|r| r.buffers.get(&id).map_or(0, |b| b.bytes.len()))
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_buffer_kind(id: u64) -> u32 {
    guarded(|r| r.buffers.get(&id).map_or(u32::MAX, |b| b.kind))
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_buffer_free(id: u64) -> u32 {
    guarded(|r| match r.buffers.remove(&id) {
        Some(b) => {
            r.bytes -= b.bytes.len();
            1
        }
        None => 0,
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_session_close(id: u64) -> u32 {
    guarded(|r| u32::from(r.sessions.remove(&id).is_some()))
}
/// Operations: new=0, demo=1, state=2, undo=3, redo=4, encode=5, decode=6,
/// RGBA=7, frame=9, layer=10, acknowledge durable save=12 (a=revision).
#[unsafe(no_mangle)]
pub extern "C" fn pc_request(h: u64, op: u32, a: u32, b: u32, input: u64) -> u64 {
    guarded(|r| {
        if r.buffers.len() >= 64 || r.bytes.saturating_add(MAX_BUFFER) > MAX_POOL {
            return 0;
        }
        match r.request(h, op, a, b, input) {
            Ok((bytes, kind)) => r.buffer(bytes, kind),
            Err(e) => r.buffer(e.into_bytes(), 0),
        }
    })
}
/// Phase begin=0, extend=1, finish=2, cancel=3. Input: ≤256 little-endian i32 x/y pairs.
#[unsafe(no_mangle)]
pub extern "C" fn pc_stroke(h: u64, phase: u32, buffer: u64, color: u32, size: u32) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let r = &mut *registry().lock().unwrap_or_else(|p| p.into_inner());
        let points = if buffer == 0 {
            vec![]
        } else {
            let Some(b) = r.buffers.get(&buffer) else {
                return -2;
            };
            if b.kind != 4 || b.bytes.len() > 256 * 8 || b.bytes.len() % 8 != 0 {
                return -2;
            }
            b.bytes
                .chunks_exact(8)
                .map(|p| {
                    (
                        i32::from_le_bytes(p[..4].try_into().unwrap()),
                        i32::from_le_bytes(p[4..].try_into().unwrap()),
                    )
                })
                .collect::<Vec<_>>()
        };
        let Some(s) = r.sessions.get_mut(&h) else {
            return -1;
        };
        if s.stroke(phase, &points, color, size).is_err() {
            -3
        } else {
            0
        }
    }))
    .unwrap_or(-99)
}

#[cfg(test)]
mod quota_tests {
    use super::*;
    #[test]
    fn failed_finish_restores_artwork_history_and_accepts_a_fresh_stroke() {
        let mut session = MobileSession {
            editor: EditorSession::new(Document::new(16, 16)),
            stroke: None,
            revision: 1,
            raster_pixels: RASTER_BUDGET / 4,
            clipboard: None,
        };
        let original = session.editor.doc.clone();
        session.stroke(0, &[(1, 1)], u32::MAX, 1).unwrap();
        // Exercise the finish rollback when bounded serialization rejects an edit.
        session.editor.doc.color_profile = vec![0; 2 * 1024 * 1024];
        assert!(session.stroke(2, &[], 0, 0).is_err());
        assert_eq!(session.editor.doc, original);
        assert!(session.editor.undo.is_empty());
        assert!(session.editor.redo.is_empty());
        assert!(session.stroke.is_none());
        session.stroke(0, &[(2, 2)], u32::MAX, 1).unwrap();
        session.stroke(2, &[], 0, 0).unwrap();
        assert_eq!(session.editor.undo.len(), 1);
        assert!(encode_mobile(&session.editor.doc).is_ok());
    }
    #[test]
    fn a_new_sparse_block_cannot_bypass_the_runtime_raster_budget() {
        let mut session = MobileSession {
            editor: EditorSession::new(Document::new(64, 64)),
            stroke: None,
            revision: 1,
            raster_pixels: 32,
            clipboard: None,
        };
        session.stroke(0, &[(0, 0)], u32::MAX, 1).unwrap();
        let before = session.editor.doc.clone();
        assert!(session.stroke(1, &[(40, 40)], u32::MAX, 1).is_err());
        assert_eq!(session.editor.doc, before);
        assert!(session.editor.undo.is_empty());
        session.stroke(3, &[], 0, 0).unwrap();
        assert_eq!(session.editor.doc, session.editor.saved);
    }
}
