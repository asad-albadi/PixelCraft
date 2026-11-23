//! Bounded private recovery envelope. Native `.pixel` files remain unchanged.
use super::*;
const MAGIC: &[u8; 8] = b"PCREC1\0\0";
pub(super) fn encode(editor: &EditorSession) -> Result<Vec<u8>, String> {
    let artwork = encode_mobile(&editor.doc)?;
    let saved = encode_mobile(&editor.saved)?;
    let size = 24usize
        .saturating_add(artwork.len())
        .saturating_add(saved.len());
    if size > MAX_BUFFER {
        return Err("Recovery exceeds the mobile file budget".into());
    }
    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(MAGIC);
    for value in [artwork.len(), saved.len(), editor.frame, editor.layer] {
        bytes.extend_from_slice(&(value as u32).to_le_bytes());
    }
    bytes.extend(artwork);
    bytes.extend(saved);
    Ok(bytes)
}
pub(super) fn decode(bytes: &[u8]) -> Result<EditorSession, String> {
    if bytes.len() < 24 || bytes.len() > MAX_BUFFER || &bytes[..8] != MAGIC {
        return Err("Unsupported or damaged recovery snapshot".into());
    }
    let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    let (current, baseline, frame, layer) = (word(8), word(12), word(16), word(20));
    if current == 0
        || baseline == 0
        || 24usize
            .checked_add(current)
            .and_then(|n| n.checked_add(baseline))
            != Some(bytes.len())
    {
        return Err("Invalid recovery snapshot lengths".into());
    }
    let doc = document(6, 0, 0, &bytes[24..24 + current])?;
    let saved = document(6, 0, 0, &bytes[24 + current..])?;
    if frame >= doc.durations.len() || layer >= doc.layers.len() {
        return Err("Invalid recovery editor target".into());
    }
    Ok(EditorSession {
        saved,
        frame,
        layer,
        ..EditorSession::new(doc)
    })
}
