use poseforge_core::{Editor, Node, PreparedProjectEdit, Project};

#[test]
fn prepared_import_is_one_atomic_undo_and_redo() {
    let mut editor = Editor::new(Project::default());
    let before = editor.project.clone();
    let mut after = before.clone();
    after.nodes.push(Node::new(500, "Imported"));
    let edit = PreparedProjectEdit::new(before.clone(), after).unwrap();
    editor.commit_prepared("Импорт", edit).unwrap();
    let imported = editor.project.clone();
    assert!(imported.node(500).unwrap().rest_transform.is_some());
    assert_eq!(editor.undo_len(), 1);
    assert!(editor.undo());
    assert_eq!(editor.project, before);
    assert!(editor.redo());
    assert_eq!(editor.project, imported);
}

#[test]
fn invalid_stale_and_active_edits_never_replace_the_document() {
    let mut editor = Editor::new(Project::default());
    let before = editor.project.clone();
    let mut invalid = before.clone();
    let mut node = Node::new(501, "Invalid");
    node.parent = Some(999);
    invalid.nodes.push(node);
    assert!(PreparedProjectEdit::new(before.clone(), invalid).is_err());
    let mut after = before.clone();
    after.nodes.push(Node::new(500, "Imported"));
    let edit = PreparedProjectEdit::new(before.clone(), after.clone()).unwrap();
    editor
        .transact("Rename", |p| p.title = "New scene".into())
        .unwrap();
    let newer = editor.project.clone();
    assert!(editor.commit_prepared("Import", edit).is_err());
    assert_eq!(editor.project, newer);
    assert_eq!(editor.undo_len(), 1);
    let edit = PreparedProjectEdit::new(newer.clone(), after).unwrap();
    editor.begin_edit("Drag");
    assert!(editor.commit_prepared("Import", edit).is_err());
    assert_eq!(editor.project, newer);
    assert_eq!(editor.undo_len(), 1);
}
