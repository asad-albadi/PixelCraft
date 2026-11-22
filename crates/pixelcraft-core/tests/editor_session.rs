use pixelcraft_core::{
    editor::{EditorSession, SelectionMask, StrokeStabilizer},
    model::Document,
};

#[test]
fn history_restores_artwork_selection_and_frame_without_gui() {
    let mut session = EditorSession::new(Document::new(3, 2));
    session.selection = Some((0, 0, 1, 1));
    session.mask = Some(SelectionMask::from(vec![
        true, false, false, false, false, false,
    ]));
    session.polygon = vec![(0, 0), (1, 0), (0, 1)];
    session.remember_action("Draw");
    session.doc.set_pixel(0, 0, 0, [1, 2, 3, 255]);
    session.doc.add_frame(0, false);
    session.frame = 1;
    session.selection = None;
    session.mask = None;
    session.polygon.clear();
    let drawn = session.doc.clone();
    assert!(session.jump_history(0));
    assert_eq!(session.doc, session.saved);
    assert_eq!(session.frame, 0);
    assert_eq!(session.selection, Some((0, 0, 1, 1)));
    assert!(session.mask.as_ref().unwrap()[0]);
    assert_eq!(session.polygon.len(), 3);
    assert!(session.jump_history(1));
    assert_eq!(session.doc, drawn);
    assert_eq!(session.frame, 1);
    assert!(session.selection.is_none());
    assert!(session.mask.is_none());
    assert!(session.polygon.is_empty());
}

#[test]
fn new_edits_drop_redo_and_invalid_history_targets_preserve_state() {
    let mut session = EditorSession::new(Document::new(2, 2));
    session.remember_action("First");
    session.doc.set_pixel(0, 0, 0, [1, 2, 3, 255]);
    assert!(session.jump_history(0));
    assert_eq!(session.redo.len(), 1);
    let before = session.doc.clone();
    assert!(!session.jump_history(9));
    assert_eq!(session.doc, before);
    assert_eq!(session.redo.len(), 1);
    session.remember_action("Second");
    assert!(session.redo.is_empty());
    for _ in 0..80 {
        session.remember_action("Edit");
    }
    assert_eq!(session.undo.len(), 64);
}

#[test]
fn stroke_release_reaches_pointer_and_zero_radius_is_exact() {
    let mut smooth = StrokeStabilizer::new((0, 0), 4);
    assert_eq!(smooth.advance(Some((12, 0)), false), (8, 0));
    assert_eq!(smooth.advance(None, true), (12, 0));
    let mut exact = StrokeStabilizer::new((0, 0), 0);
    assert_eq!(exact.advance(Some((-8, 7)), false), (-8, 7));
}
