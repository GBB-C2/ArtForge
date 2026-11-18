use super::tests::{app, press};
use super::*;

#[test]
fn signed_extrusion_inset_and_bevel_grips_drive_the_panel_preview() {
    for (command, field, targets) in [
        (A::Extrude, 0, [-0.16, 0.24]),
        (A::Inset, 0, [0.06, 0.18]),
        (A::Bevel, 0, [0.12, 0.2]),
        (A::Bevel, 3, [-65., 60.]),
        (A::EdgeBevel, 2, [-65., 60.]),
    ] {
        let mut a = app(command);
        if command == A::Bevel && field == 3 {
            a.modeling_live_confirm(&Action::Author(A::LiveField(1)), "0.2")
                .unwrap();
        }
        let baseline = a.editor.project.clone();
        let initial = a.modeling_live.as_ref().unwrap().values;
        for target in targets {
            let op = a.modeling_live.as_ref().unwrap();
            let (dir, step) = a.live_parameter_basis(op, field);
            let old = op.values[field];
            press(&mut a, A::LiveDrag(field as u8));
            let next = Vec2::from_array(a.mouse) + Vec2::from_array(dir) * (target - old) / step;
            a.mouse_move(next.to_array());
            let op = a.modeling_live.as_ref().unwrap();
            assert!(
                (op.values[field] - target).abs() < 0.001,
                "{command:?}: {}",
                a.status
            );
            assert!(op
                .values
                .iter()
                .enumerate()
                .all(|(i, v)| i == field || *v == initial[i]));
            assert_ne!(a.editor.project, baseline);
            let mut expected = Editor::new(baseline.clone());
            expected.begin_edit("Expected");
            expected
                .preview_mesh(1, &op.original, op.edit(op.values))
                .unwrap();
            assert_eq!(a.editor.project.meshes, expected.project.meshes);
            a.cancel_drag();
            assert_eq!(a.editor.project, baseline);
            assert_eq!(a.editor.undo_len(), 0);
        }
    }
}
#[test]
fn array_spacing_can_resume_after_zero_without_losing_its_direction() {
    let mut a = app(A::Extrude);
    a.cancel_modeling_live();
    a.authoring.workflow.values[3..6].copy_from_slice(&[-0.2, 0.3, 0.1]);
    a.action(Action::ModelingTool(ui::ModelingTool::Array));
    let op = a.modeling_live.as_ref().unwrap();
    let direction = op.offset().normalize();
    let (axis, step) = a.spacing_basis(op);
    let distance = op.offset().length() / step + 4.;
    press(&mut a, A::ArraySpacing);
    a.mouse_move((Vec2::from_array(a.mouse) - axis * distance).to_array());
    a.mouse_up(MouseButton::Left);
    assert_eq!(a.modeling_live.as_ref().unwrap().offset(), Vec3::ZERO);
    let (axis, step) = a.spacing_basis(a.modeling_live.as_ref().unwrap());
    press(&mut a, A::ArraySpacing);
    a.mouse_move((Vec2::from_array(a.mouse) + axis * 16.).to_array());
    let offset = a.modeling_live.as_ref().unwrap().offset();
    assert!((offset - direction * step * 16.).length() < 1e-4);
    a.mouse_up(MouseButton::Left);
    a.action(Action::Author(A::LiveAccept));
    assert_eq!(a.editor.undo_len(), 1);
}
#[test]
fn dragging_bridge_seam_across_zero_selects_the_adjacent_vertex() {
    let mut a = app(A::Extrude);
    a.cancel_modeling_live();
    a.preferences.right_panel = false;
    crate::app::modeling_workflow_tests::select_bridge_caps(&mut a);
    let source = a.editor.project.clone();
    a.action(Action::ModelingTool(ui::ModelingTool::Bridge));
    a.modeling_live_confirm(&Action::Author(A::LiveField(0)), "3")
        .unwrap();
    let op = a.modeling_live.as_ref().unwrap();
    let rim = a.projected_rim(op).unwrap();
    assert!(grip_geometry::rim_draggable(&rim));
    let preview = a.editor.project.clone();
    press(&mut a, A::LiveDrag(1));
    let mouse = Vec2::from_array(a.mouse);
    let end = rim.len() - 1;
    for (index, expected) in [(end, -1.), (0, 0.), (1, 1.)] {
        a.mouse_move((mouse + rim[index] - rim[0]).to_array());
        let op = a.modeling_live.as_ref().unwrap();
        assert_eq!(op.values[1], expected, "{}", a.status);
        assert_eq!(a.editor.project == preview, index == 0);
    }
    a.mouse_up(MouseButton::Left);
    a.action(Action::Author(A::LiveAccept));
    assert_eq!(a.editor.undo_len(), 1);
    a.action(Action::Undo);
    assert_eq!(a.editor.project, source);
}

#[test]
fn hover_readout_preserves_the_complete_name_and_unit_in_both_languages() {
    let mut a = app(A::EdgeBevel);
    press(&mut a, A::LiveDrag(0));
    for (language, caption) in [
        (crate::localization::Language::Russian, "Ширина"),
        (crate::localization::Language::English, "Width"),
    ] {
        let mut ui = Ui::new(1.);
        ui.language = language;
        let op = a.modeling_live.as_ref().unwrap();
        a.draw_live_handles(&mut ui, op);
        let unit = if language == crate::localization::Language::English {
            "m"
        } else {
            "м"
        };
        let label = format!(
            "{caption} · {} {unit}",
            crate::ui::number_caption(op.values[0] as f64)
        );
        assert!(ui.texts.iter().any(|t| t.text == label), "{label}");
        assert!(ui.texts.iter().all(|t| !t.text.ends_with('…')));
    }
}
