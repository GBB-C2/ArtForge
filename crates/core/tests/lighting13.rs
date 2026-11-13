use poseforge_core::*;

#[test]
fn graph_compiles_reordered_ids_and_keeps_four_outputs() {
    let mut color = MaterialNode::new(7, MaterialNodeKind::Color, [0., 0.]);
    color.value = [0.2, 0.4, 0.8, 1.];
    let mut multiply = MaterialNode::new(100, MaterialNodeKind::Multiply, [200., 0.]);
    multiply.inputs[0] = Some(7);
    let mut graph = MaterialGraph {
        nodes: vec![multiply, color],
        ..MaterialGraph::default()
    };
    graph.outputs = [Some(100), None, None, Some(7), None, None, None, None];
    let program = graph.compile().unwrap();
    assert_eq!(program.nodes.len(), 2);
    assert_eq!(program.nodes[0].value, [0.2, 0.4, 0.8, 1.]);
    assert_eq!(program.nodes[1].op_inputs[1], 0);
    assert_eq!(program.outputs, [1, -1, -1, 0, -1, -1, -1, -1]);
}
#[test]
fn graph_rejects_cycles_missing_ids_and_invalid_values_without_losing_old_link() {
    let mut graph = MaterialGraph {
        nodes: vec![
            MaterialNode::new(1, MaterialNodeKind::Mix, [0., 0.]),
            MaterialNode::new(2, MaterialNodeKind::Multiply, [0., 0.]),
        ],
        ..MaterialGraph::default()
    };
    graph.connect(2, 0, Some(1)).unwrap();
    let before = graph.clone();
    assert!(graph.connect(1, 1, Some(2)).is_err());
    assert_eq!(graph, before);
    assert!(graph.connect(0, 0, Some(77)).is_err());
    assert_eq!(graph, before);
    graph.nodes[0].value[0] = f32::NAN;
    assert!(graph.compile().is_err());
    graph = before.clone();
    graph.nodes[0].id = 2;
    assert!(graph.compile().is_err());
    graph = before;
    graph.nodes[0].position[0] = f32::INFINITY;
    assert!(graph.compile().is_err());
}
#[test]
fn deleting_graph_node_detaches_dependents_and_material_presets_preserve_graph() {
    let mut graph = MaterialGraph {
        nodes: vec![
            MaterialNode::new(1, MaterialNodeKind::Color, [0., 0.]),
            MaterialNode::new(2, MaterialNodeKind::Mix, [0., 0.]),
        ],
        ..MaterialGraph::default()
    };
    graph.connect(2, 0, Some(1)).unwrap();
    graph.connect(0, 3, Some(1)).unwrap();
    let mut material = Material {
        graph: Some(graph.clone()),
        ..Material::default()
    };
    MaterialPreset::Glass.apply(&mut material);
    assert_eq!(material.graph, Some(graph.clone()));
    graph.remove(1);
    assert_eq!(graph.outputs[3], None);
    assert_eq!(graph.nodes[0].inputs[0], None);
    graph.compile().unwrap();
}
#[test]
fn hdri_radiance_bounds_and_ray_sample_budgets_are_validated() {
    let mut map = EnvironmentMap {
        name: "HDR".into(),
        width: 2,
        height: 1,
        rgb: vec![[8., 0.4, 1.], [0.; 3]],
    };
    map.validate().unwrap();
    map.rgb[0][0] = f32::INFINITY;
    assert!(map.validate().is_err());
    map.rgb[0][0] = 1.;
    map.width = 4097;
    assert!(map.validate().is_err());
    let mut s = RenderSettings {
        ray_reflections: true,
        ray_shadows: true,
        reflection_samples: 16,
        area_shadow_samples: 64,
        ..RenderSettings::default()
    };
    s.validate().unwrap();
    s.area_shadow_samples = 65;
    assert!(s.validate().is_err());
    s.area_shadow_samples = 16;
    s.hdri_rotation = 181.;
    assert!(s.validate().is_err());
}
