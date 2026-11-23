use super::*;

pub(super) fn import(session: &mut MobileSession, format: u32, bytes: &[u8]) -> Result<(), String> {
    let doc = formats::decode_mobile_format(format, bytes)?;
    check_document(&doc)?;
    encode_mobile(&doc)?;
    let revision = session.next_revision()?;
    session.editor = EditorSession::new(doc);
    session.revision = revision;
    Ok(())
}

/// Keep a single validated candidate detached from active artwork and history.
pub(super) fn prepare(
    session: &mut MobileSession,
    format: u32,
    bytes: &[u8],
) -> Result<(), String> {
    if session.prepared_import.is_some() {
        return Err("An import is already prepared; commit or cancel it first".into());
    }
    let doc = if format == 0 {
        document(6, 0, 0, bytes)?
    } else {
        formats::decode_mobile_format(format, bytes)?
    };
    check_document(&doc)?;
    encode_mobile(&doc)?;
    session.prepared_import = Some((session.revision, doc));
    Ok(())
}

pub(super) fn commit_prepared(session: &mut MobileSession, expected: u32) -> Result<(), String> {
    let (revision, _) = session
        .prepared_import
        .as_ref()
        .ok_or("No prepared import")?;
    if *revision != expected || session.revision != expected {
        return Err("Artwork changed; cancel and retry import".into());
    }
    let next = session.next_revision()?;
    let (_, doc) = session
        .prepared_import
        .take()
        .expect("validated prepared import");
    session.editor = EditorSession::new(doc);
    session.revision = next;
    Ok(())
}
