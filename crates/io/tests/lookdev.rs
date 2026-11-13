use poseforge_core::*;
use poseforge_io::{load_project, save_project};

#[test]
fn schema_seven_defaults_keep_the_old_look_and_schema_eight_roundtrips() {
    let mut p = Project::default();
    p.materials.push(Material {
        id: 1,
        ..Material::default()
    });
    p.lights.push(Light::default());
    let mut json = serde_json::to_value(&p).unwrap();
    json["version"] = 7.into();
    for key in ["uv_scale", "uv_offset", "uv_rotation"] {
        json["materials"][0].as_object_mut().unwrap().remove(key);
    }
    for key in ["use_temperature", "temperature_kelvin", "area_rotation"] {
        json["lights"][0].as_object_mut().unwrap().remove(key);
    }
    for key in ["ambient_occlusion", "ambient_occlusion_radius"] {
        json["render_settings"].as_object_mut().unwrap().remove(key);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("look.artforge");
    let bytes = serde_json::to_vec(&json).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let mut loaded = load_project(&path).unwrap();
    assert_eq!(loaded, p);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    loaded.materials[0].uv_scale = [2., 3.];
    loaded.materials[0].uv_rotation = 30.;
    loaded.lights[0].use_temperature = true;
    loaded.lights[0].temperature_kelvin = 3200.;
    loaded.lights[0].area_rotation = 70.;
    loaded.render_settings.ambient_occlusion = 0.8;
    loaded.render_settings.ambient_occlusion_radius = 0.5;
    save_project(&loaded, &path).unwrap();
    assert_eq!(load_project(&path).unwrap(), loaded);
}
