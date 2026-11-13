use poseforge_core::{Mat4, Project, Vec3};
use poseforge_io::{import_gltf, load_project, merge_project, save_project, IoError};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fixture(dir: &Path, count: usize, extra_bind: bool) -> Value {
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut append = |bytes: Vec<u8>, component: u32, kind: &str, count: usize| {
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        views.push(json!({"buffer":0,"byteOffset":bin.len(),"byteLength":bytes.len()}));
        bin.extend(bytes);
        accessors.push(
            json!({"bufferView":views.len()-1,"componentType":component,"type":kind,"count":count}),
        );
        accessors.len() - 1
    };
    let position = append(
        [0f32, 0., 0., 1., 0., 0., 0., 1., 0.]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
        5126,
        "VEC3",
        3,
    );
    let joints = append(
        (0..3)
            .flat_map(|_| [(count - 1) as u16, 0, 0, 0])
            .flat_map(u16::to_le_bytes)
            .collect(),
        5123,
        "VEC4",
        3,
    );
    let weights = append(
        (0..3)
            .flat_map(|_| [1f32, 0., 0., 0.])
            .flat_map(f32::to_le_bytes)
            .collect(),
        5126,
        "VEC4",
        3,
    );
    let bind_count = count + usize::from(extra_bind);
    let ibm = append(
        (0..bind_count)
            .flat_map(|_| Mat4::from_translation(-Vec3::Y).to_cols_array())
            .flat_map(f32::to_le_bytes)
            .collect(),
        5126,
        "MAT4",
        bind_count,
    );
    let time = append(
        [0f32, 1.].into_iter().flat_map(f32::to_le_bytes).collect(),
        5126,
        "SCALAR",
        2,
    );
    let motion = append(
        [0f32, 1., 0., 0., 3., 0.]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
        5126,
        "VEC3",
        2,
    );
    accessors[position]["min"] = json!([0, 0, 0]);
    accessors[position]["max"] = json!([1, 1, 0]);
    accessors[time]["min"] = json!([0]);
    accessors[time]["max"] = json!([1]);
    let mut nodes = vec![json!({"name":"Rig","children":(1..=count).collect::<Vec<_>>()})];
    nodes.extend((1..=count).map(|i| json!({"name":format!("Joint {i}"),"translation":[0,1,0]})));
    nodes.push(json!({"name":"Mesh","mesh":0,"skin":0}));
    let doc = json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0,count+1]}],
        "nodes":nodes,"skins":[{"joints":(1..=count).rev().collect::<Vec<_>>(),"inverseBindMatrices":ibm,"skeleton":0}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":position,"JOINTS_0":joints,"WEIGHTS_0":weights}}]}],
        "buffers":[{"uri":"rig.bin","byteLength":bin.len()}],"bufferViews":views,"accessors":accessors,
        "animations":[{"samplers":[{"input":time,"output":motion,"interpolation":"LINEAR"}],
            "channels":[{"sampler":0,"target":{"node":1,"path":"translation"}}]}]});
    fs::write(dir.join("rig.bin"), bin).unwrap();
    write(dir, &doc);
    doc
}

fn write(dir: &Path, doc: &Value) {
    fs::write(dir.join("rig.gltf"), serde_json::to_vec(doc).unwrap()).unwrap();
}

fn check_pose(project: &Project, count: usize) {
    let node = project.nodes.iter().find(|n| n.skin.is_some()).unwrap();
    let skin = node.skin.as_ref().unwrap();
    assert_eq!(skin.joints.len(), count);
    assert_eq!(skin.inverse_bind_matrices.len(), count);
    let mesh = project.mesh(node.mesh.unwrap()).unwrap();
    assert!(mesh
        .vertices
        .iter()
        .all(|v| v.joints[0] == (count - 1) as u32 && v.weights == [1., 0., 0., 0.]));
    for (time, offset) in [(0., 0.), (0.5, 1.), (1., 2.)] {
        let scene = project.evaluated(Some(0), time).unwrap();
        let globals = &scene.global;
        let palette_index = count - 1;
        let matrix =
            globals[&skin.joints[palette_index]] * skin.inverse_bind_matrices[palette_index];
        for v in &mesh.vertices {
            let p = Vec3::from_array(v.position);
            assert!((matrix.transform_point3(p) - p - Vec3::Y * offset).length() < 1e-5);
        }
    }
}

#[test]
fn rigs_above_256_keep_high_indices_bind_order_animation_merge_and_save() {
    for count in [257, 1024, 8192] {
        let dir = tempfile::tempdir().unwrap();
        let doc = fixture(dir.path(), count, true);
        let imported = import_gltf(dir.path().join("rig.gltf")).unwrap();
        assert_eq!(imported.report.joints, count);
        assert!(imported.report.warnings.is_empty());
        assert_eq!(
            imported
                .project
                .nodes
                .iter()
                .find_map(|n| n.skin.as_ref())
                .unwrap()
                .joints,
            (2..=count as u64 + 1).rev().collect::<Vec<_>>()
        );
        check_pose(&imported.project, count);
        let mut project = Project::default();
        merge_project(&mut project, imported.project).unwrap();
        check_pose(&project, count);
        let saved = dir.path().join("rig.poseforge");
        save_project(&project, &saved).unwrap();
        assert_eq!(load_project(&saved).unwrap(), project);
        // The same skin also works without an explicit bind matrix accessor.
        if count == 257 {
            let mut implicit = doc;
            implicit["skins"][0]
                .as_object_mut()
                .unwrap()
                .remove("inverseBindMatrices");
            write(dir.path(), &implicit);
            let p = import_gltf(dir.path().join("rig.gltf")).unwrap().project;
            assert!(p
                .nodes
                .iter()
                .find_map(|n| n.skin.as_ref())
                .unwrap()
                .inverse_bind_matrices
                .iter()
                .all(|m| *m == Mat4::IDENTITY));
        }
    }
}

#[test]
fn auto_rig_pro_flat_export_keeps_skin_and_animation_and_maps_deform_roles() {
    use poseforge_core::{detect_humanoid, BoneRole};
    let dir = tempfile::tempdir().unwrap();
    let mut doc = fixture(dir.path(), 17, false);
    let names = [
        "root.x",
        "spine_01.x",
        "spine_02.x",
        "neck.x",
        "head.x",
        "arm_stretch.l",
        "forearm_stretch.l",
        "hand.l",
        "arm_stretch.r",
        "forearm_stretch.r",
        "hand.r",
        "thigh_stretch.l",
        "leg_stretch.l",
        "foot.l",
        "thigh_stretch.r",
        "leg_stretch.r",
        "foot.r",
    ];
    for (i, name) in names.iter().enumerate() {
        doc["nodes"][i + 1]["name"] = json!(format!("Rig:{name}"));
    }
    write(dir.path(), &doc);
    let p = import_gltf(dir.path().join("rig.gltf")).unwrap().project;
    check_pose(&p, 17);
    let report = detect_humanoid(&p, None);
    assert_eq!(report.resolved.assignments.len(), 17);
    assert_eq!(report.resolved.assignments[&BoneRole::LeftUpperArm], 7);
    assert!(
        report.resolved.chain(BoneRole::LeftHand, &p).is_err(),
        "No Parents export must not gain invented IK links"
    );
    let saved = dir.path().join("arp.artforge");
    save_project(&p, &saved).unwrap();
    let loaded = load_project(saved).unwrap();
    assert_eq!(loaded, p);
    check_pose(&loaded, 17);
}

#[test]
fn expanded_skin_budget_and_short_bind_accessors_fail_before_buffer_reads() {
    let dir = tempfile::tempdir().unwrap();
    let doc = fixture(dir.path(), 1024, false);
    fs::remove_file(dir.path().join("rig.bin")).unwrap();
    let mut short = doc.clone();
    short["accessors"][3]["count"] = json!(1023);
    write(dir.path(), &short);
    let error = import_gltf(dir.path().join("rig.gltf")).unwrap_err();
    assert!(
        error.to_string().contains("меньше числа суставов"),
        "{error}"
    );
    let mut expanded = doc;
    let primitive = expanded["meshes"][0]["primitives"][0].clone();
    expanded["meshes"][0]["primitives"] = json!(vec![primitive; 4]);
    let nodes = expanded["nodes"].as_array_mut().unwrap();
    nodes.extend((0..256).map(|_| json!({"mesh":0,"skin":0})));
    write(dir.path(), &expanded);
    let error = import_gltf(dir.path().join("rig.gltf")).unwrap_err();
    assert!(matches!(error, IoError::Limit(_)), "{error}");
    assert!(error.to_string().contains("64 МиБ"), "{error}");
}

#[test]
fn high_joint_outside_skin_is_still_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let doc = fixture(dir.path(), 257, false);
    let offset = doc["bufferViews"][1]["byteOffset"].as_u64().unwrap() as usize;
    let path = dir.path().join("rig.bin");
    let mut bin = fs::read(&path).unwrap();
    bin[offset..offset + 2].copy_from_slice(&257u16.to_le_bytes());
    fs::write(path, bin).unwrap();
    let error = import_gltf(dir.path().join("rig.gltf")).unwrap_err();
    assert!(
        error.to_string().contains("индекс сустава вне skin"),
        "{error}"
    );
}

fn eight_influences(dir: &Path) -> Value {
    let mut doc = fixture(dir, 8, false);
    let path = dir.join("rig.bin");
    let mut bin = fs::read(&path).unwrap();
    let joints = doc["bufferViews"][1]["byteOffset"].as_u64().unwrap() as usize;
    let weights = doc["bufferViews"][2]["byteOffset"].as_u64().unwrap() as usize;
    for v in 0..3 {
        for k in 0..4 {
            bin[joints + v * 8 + k * 2..joints + v * 8 + k * 2 + 2]
                .copy_from_slice(&(k as u16).to_le_bytes());
            bin[weights + v * 16 + k * 4..weights + v * 16 + k * 4 + 4]
                .copy_from_slice(&[0.02f32, 0.03, 0.04, 0.01][k].to_le_bytes());
        }
    }
    for (semantic, bytes, component) in [
        (
            "JOINTS_1",
            (0..3)
                .flat_map(|_| [4u16, 5, 6, 7])
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
            5123,
        ),
        (
            "WEIGHTS_1",
            (0..3)
                .flat_map(|_| [0.1f32, 0.2, 0.25, 0.35])
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
            5126,
        ),
    ] {
        let view = doc["bufferViews"].as_array().unwrap().len();
        doc["bufferViews"]
            .as_array_mut()
            .unwrap()
            .push(json!({"buffer":0,"byteOffset":bin.len(),"byteLength":bytes.len()}));
        let accessor = doc["accessors"].as_array().unwrap().len();
        doc["accessors"]
            .as_array_mut()
            .unwrap()
            .push(json!({"bufferView":view,"componentType":component,"type":"VEC4","count":3}));
        doc["meshes"][0]["primitives"][0]["attributes"][semantic] = json!(accessor);
        bin.extend(bytes);
    }
    doc["buffers"][0]["byteLength"] = json!(bin.len());
    for i in 1..=8 {
        doc["nodes"][i]["translation"] = json!([i, 1, 0]);
    }
    fs::write(path, bin).unwrap();
    write(dir, &doc);
    doc
}

#[test]
fn strongest_four_across_all_sets_keep_the_rig_pose_and_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    eight_influences(dir.path());
    let imported = import_gltf(dir.path().join("rig.gltf")).unwrap();
    assert_eq!(imported.report.joints, 8);
    assert_eq!(imported.report.clips, 1);
    assert_eq!(imported.report.warnings.len(), 1);
    assert!(imported.report.warnings[0].contains("для 3 вершин"));
    assert!(imported.report.warnings[0].contains("деформация может отличаться"));
    let mut project = Project::default();
    merge_project(&mut project, imported.project).unwrap();
    let saved = dir.path().join("converted.artforge");
    save_project(&project, &saved).unwrap();
    let project = load_project(saved).unwrap();
    let node = project.nodes.iter().find(|n| n.skin.is_some()).unwrap();
    let skin = node.skin.as_ref().unwrap();
    let globals = project.global_transforms().unwrap();
    for v in &project.mesh(node.mesh.unwrap()).unwrap().vertices {
        assert_eq!(v.joints, [4, 5, 6, 7]);
        for (actual, expected) in v.weights.iter().zip([0.1f32, 0.2, 0.25, 0.35]) {
            assert!((actual - expected / 0.9).abs() < 1e-6);
        }
        let mut posed = Vec3::ZERO;
        for (&joint, &weight) in v.joints.iter().zip(&v.weights) {
            let m =
                globals[&skin.joints[joint as usize]] * skin.inverse_bind_matrices[joint as usize];
            posed += m.transform_point3(Vec3::from_array(v.position)) * weight;
        }
        let expected = Vec3::from_array(v.position) + Vec3::X * (1.85 / 0.9);
        assert!((posed - expected).length() < 1e-5);
    }
}

#[test]
fn invalid_discarded_influence_is_not_hidden_by_reduction() {
    let dir = tempfile::tempdir().unwrap();
    let doc = eight_influences(dir.path());
    let offset = doc["bufferViews"][1]["byteOffset"].as_u64().unwrap() as usize;
    let path = dir.path().join("rig.bin");
    let mut bin = fs::read(&path).unwrap();
    bin[offset..offset + 2].copy_from_slice(&100u16.to_le_bytes());
    fs::write(path, bin).unwrap();
    assert!(import_gltf(dir.path().join("rig.gltf"))
        .unwrap_err()
        .to_string()
        .contains("индекс сустава вне skin"));
}

#[test]
fn missing_skin_attributes_do_not_bind_every_vertex_to_the_first_joint() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = fixture(dir.path(), 8, false);
    doc["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .unwrap()
        .retain(|name, _| name == "POSITION");
    write(dir.path(), &doc);
    assert!(import_gltf(dir.path().join("rig.gltf"))
        .unwrap_err()
        .to_string()
        .contains("отсутствуют JOINTS/WEIGHTS"));
}

#[test]
fn too_many_weight_sets_are_bounded_before_buffer_io() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = fixture(dir.path(), 8, false);
    for set in 1..33 {
        doc["meshes"][0]["primitives"][0]["attributes"][format!("JOINTS_{set}")] = json!(1);
        doc["meshes"][0]["primitives"][0]["attributes"][format!("WEIGHTS_{set}")] = json!(2);
    }
    write(dir.path(), &doc);
    fs::remove_file(dir.path().join("rig.bin")).unwrap();
    let error = import_gltf(dir.path().join("rig.gltf")).unwrap_err();
    assert!(matches!(error, IoError::Limit(_)), "{error}");
}
