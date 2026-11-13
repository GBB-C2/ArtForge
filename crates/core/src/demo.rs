use crate::*;

impl Project {
    /// Articulated mannequin in a T pose, with rigid shells and a wave clip.
    pub fn demo() -> Self {
        #[derive(serde::Deserialize)]
        struct StarterBone {
            id: u64,
            name: String,
            parent: u64,
            translation: [f32; 3],
        }
        #[derive(serde::Deserialize)]
        struct Starter {
            meshes: Vec<Mesh>,
            bones: Vec<StarterBone>,
            materials: Vec<Material>,
        }
        // Geometry and its bind skeleton share a single authored source.
        // New scenes share immutable buffers until an edit detaches them.
        static STARTER: std::sync::OnceLock<Starter> = std::sync::OnceLock::new();
        let starter = STARTER.get_or_init(|| {
            serde_json::from_str(include_str!("../assets/starter_character.json"))
                .expect("validated bundled starter character")
        });
        let mut p = Project {
            title: "Шарнирный манекен".into(),
            ..Project::default()
        };
        p.nodes.push(Node::new(1, "Персонаж"));
        for bone in &starter.bones {
            let mut node = Node::new(bone.id, bone.name.clone());
            node.parent = Some(bone.parent);
            node.transform = Transform::from_translation(Vec3::from(bone.translation));
            node.is_joint = true;
            p.nodes.push(node);
        }
        let globals = p.global_transforms().unwrap();
        let joints: Vec<_> = starter.bones.iter().map(|bone| bone.id).collect();
        let skin = Skin {
            inverse_bind_matrices: joints.iter().map(|j| globals[j].inverse()).collect(),
            joints,
        };
        p.materials.extend(starter.materials.iter().cloned());
        p.meshes.extend(starter.meshes.iter().cloned());
        for (id, mesh, name) in [(20, 100, "Корпус и шарниры"), (21, 102, "Голова"), (22, 103, "Кисти и стопы")] {
            let mut node = Node::new(id, name);
            node.parent = Some(1);
            node.mesh = Some(mesh);
            node.skin = Some(skin.clone());
            p.nodes.push(node);
        }
        let mut wave = Clip::new("Приветствие", 4.);
        for (t, shoulder, elbow) in [
            (0., 0., 0.),
            (0.7, 0.30, 1.05),
            (1.2, 0.42, 0.70),
            (1.7, 0.30, 1.05),
            (2.2, 0.42, 0.70),
            (2.7, 0.30, 1.05),
            (3.4, 0., 0.),
            (4., 0., 0.),
        ] {
            let mut a = p.node(10).unwrap().transform;
            a.rotation = Quat::from_rotation_z(shoulder);
            wave.key_transform(10, t, a);
            let mut b = p.node(11).unwrap().transform;
            b.rotation = Quat::from_rotation_z(elbow);
            wave.key_transform(11, t, b);
        }
        for (t, angle) in [(0., 0.), (1.3, -0.12), (2.6, 0.12), (4., 0.)] {
            let mut h = p.node(6).unwrap().transform;
            h.rotation = Quat::from_rotation_y(angle);
            wave.key_transform(6, t, h);
        }
        p.clips.push(wave);
        p.camera = Camera {
            eye: Vec3::new(0., 1.05, 3.4),
            target: Vec3::new(0., 1.02, 0.),
            ..Camera::default()
        };
        p
    }
}
/// Public procedural primitive for editor-created props and deterministic fixtures.
pub fn box_mesh(
    id: u64,
    name: impl Into<String>,
    half_extents: Vec3,
    material: Option<u64>,
) -> Mesh {
    let mut mesh = Mesh {
        id,
        name: name.into(),
        vertices: vec![].into(),
        indices: vec![].into(),
        material,
        topology: None,
        morph_targets: vec![],
    };
    append_box(&mut mesh, Vec3::ZERO, half_extents, None);
    mesh
}
fn append_box(mesh: &mut Mesh, center: Vec3, h: Vec3, joint: Option<u32>) {
    let faces = [
        (
            Vec3::X,
            [
                Vec3::new(1., -1., -1.),
                Vec3::new(1., 1., -1.),
                Vec3::new(1., 1., 1.),
                Vec3::new(1., -1., 1.),
            ],
        ),
        (
            -Vec3::X,
            [
                Vec3::new(-1., -1., 1.),
                Vec3::new(-1., 1., 1.),
                Vec3::new(-1., 1., -1.),
                Vec3::new(-1., -1., -1.),
            ],
        ),
        (
            Vec3::Y,
            [
                Vec3::new(-1., 1., -1.),
                Vec3::new(-1., 1., 1.),
                Vec3::new(1., 1., 1.),
                Vec3::new(1., 1., -1.),
            ],
        ),
        (
            -Vec3::Y,
            [
                Vec3::new(-1., -1., 1.),
                Vec3::new(-1., -1., -1.),
                Vec3::new(1., -1., -1.),
                Vec3::new(1., -1., 1.),
            ],
        ),
        (
            Vec3::Z,
            [
                Vec3::new(1., -1., 1.),
                Vec3::new(1., 1., 1.),
                Vec3::new(-1., 1., 1.),
                Vec3::new(-1., -1., 1.),
            ],
        ),
        (
            -Vec3::Z,
            [
                Vec3::new(-1., -1., -1.),
                Vec3::new(-1., 1., -1.),
                Vec3::new(1., 1., -1.),
                Vec3::new(1., -1., -1.),
            ],
        ),
    ];
    for (normal, corners) in faces {
        let start = mesh.vertices.len() as u32;
        for (i, v) in corners.into_iter().enumerate() {
            mesh.vertices.push(Vertex {
                color: [1.; 4],
                position: (center + v * h).to_array(),
                normal: normal.to_array(),
                uv: [[0., 1.], [0., 0.], [1., 0.], [1., 1.]][i],
                joints: [joint.unwrap_or(0), 0, 0, 0],
                weights: if joint.is_some() {
                    [1., 0., 0., 0.]
                } else {
                    [0.; 4]
                },
            });
        }
        mesh.indices
            .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }
}
