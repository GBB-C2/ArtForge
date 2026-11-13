use poseforge_core::{ArtisticSettings, ArtisticStyle, Editor, Material, MaterialPreset, Project};

#[test]
fn artistic_presets_append_without_reindexing_legacy_presets() {
    let legacy=[MaterialPreset::Matte,MaterialPreset::Plastic,MaterialPreset::Metal,
        MaterialPreset::Ceramic,MaterialPreset::Rubber,MaterialPreset::Glass,MaterialPreset::Water,
        MaterialPreset::BrushedMetal,MaterialPreset::Satin,MaterialPreset::Velvet,
        MaterialPreset::Toon,MaterialPreset::Anime,MaterialPreset::Ink];
    assert_eq!(&MaterialPreset::ALL[..13], &legacy);
    assert_eq!(MaterialPreset::ALL.len(),26);
    let mut names=std::collections::BTreeSet::new();
    for (i,preset) in MaterialPreset::ALL.into_iter().enumerate() {
        let material=preset.create();
        assert!(names.insert(material.name.clone()));
        assert_eq!(material.artistic.enabled,i>=13);
        assert!(material.artistic.valid());
        let p=Project { materials:vec![material.clone()], ..Project::default() };
        p.validate().unwrap();
        let roundtrip:Material=serde_json::from_slice(&serde_json::to_vec(&material).unwrap()).unwrap();
        assert_eq!(roundtrip,material);
        if i>=13 { assert!(!material.category.is_empty()); assert!(!material.tags.is_empty()); }
    }
    assert_ne!(MaterialPreset::PlasticineSoft.create().artistic,MaterialPreset::PlasticineSculpted.create().artistic);
    assert_ne!(MaterialPreset::WatercolorLight.create().artistic,MaterialPreset::WatercolorGranulated.create().artistic);
    assert!(MaterialPreset::ClayWet.create().roughness<MaterialPreset::ClayDry.create().roughness);
}

#[test]
fn artistic_old_and_partial_documents_have_safe_defaults() {
    let original=MaterialPreset::Ceramic.create();
    let mut json=serde_json::to_value(&original).unwrap();
    json.as_object_mut().unwrap().remove("artistic");
    let old:Material=serde_json::from_value(json).unwrap();
    assert_eq!(old,original);
    assert!(!old.artistic.enabled);
    let partial:ArtisticSettings=serde_json::from_str(r#"{"enabled":true,"style":"Watercolor"}"#).unwrap();
    assert!(partial.valid()); assert_eq!(partial.style,ArtisticStyle::Watercolor);
    assert!(serde_json::from_str::<ArtisticSettings>(r#"{"style":"UnknownStyle"}"#).is_err());
}

#[test]
fn artistic_fields_validate_bounds_and_never_commit_nan_or_fractional_seed() {
    let mut settings=ArtisticSettings::default();
    for field in 0..ArtisticSettings::FIELD_COUNT {
        let (min,max)=ArtisticSettings::bounds(field).unwrap();
        for value in [min,max] { assert!(settings.set_number(field,value)); assert_eq!(settings.number(field),value); assert!(settings.valid()); }
        for invalid in [min-1.,max+1.,f32::NAN,f32::INFINITY,f32::NEG_INFINITY] {
            let before=settings; assert!(!settings.set_number(field,invalid)); assert_eq!(settings,before);
        }
    }
    assert!(!settings.set_number(12,0.5));
    assert!(!settings.set_number(13,0.));
    settings.bump=f32::NAN;
    assert!(!settings.valid());
    let p=Project { materials:vec![Material{artistic:settings,..Material::default()}],..Project::default() };
    assert!(p.validate().is_err());
}

#[test]
fn artistic_style_ids_and_cycle_are_stable() {
    let mut style=ArtisticStyle::Plasticine;
    for (index,expected) in ArtisticStyle::ALL.into_iter().enumerate() {
        assert_eq!(style,expected); assert_eq!(style.shader_id(),index as u32+1);
        let defaults=ArtisticSettings::for_style(style);
        assert!(defaults.enabled && defaults.valid()); assert_eq!(defaults.style,style);
        style=style.next();
    }
    assert_eq!(style,ArtisticStyle::Plasticine);
}

#[test]
fn artistic_apply_preserves_authored_color_identity_maps_and_emission() {
    let mut material=Material { id:73,name:"Мой материал".into(),base_color:[0.1,0.3,0.7,0.4],
        base_color_texture:Some(42),normal_texture:Some(43),emissive:[1.,2.,3.],
        double_sided:true,uv_scale:[2.,3.], ..Material::default() };
    let original=material.clone();
    MaterialPreset::WatercolorGranulated.apply(&mut material);
    assert!(material.artistic.enabled);
    assert_eq!(material.id,original.id); assert_eq!(material.name,original.name);
    assert_eq!(material.base_color,original.base_color); assert_eq!(material.emissive,original.emissive);
    assert_eq!(material.base_color_texture,original.base_color_texture);
    assert_eq!(material.normal_texture,original.normal_texture); assert_eq!(material.uv_scale,original.uv_scale);
    assert_eq!(material.double_sided,original.double_sided);
    let settings=material.artistic;
    MaterialPreset::Matte.apply(&mut material);
    assert!(!material.artistic.enabled); assert_eq!(material.artistic.style,settings.style);
}

#[test]
fn artistic_settings_participate_in_editor_undo_redo() {
    let p=Project {materials:vec![Material::default()],..Project::default()};
    let mut editor=Editor::new(p.clone());
    editor.transact("Акварель",|p| MaterialPreset::WatercolorLight.apply(&mut p.materials[0])).unwrap();
    let painted=editor.project.clone();
    assert!(editor.undo()); assert_eq!(editor.project,p);
    assert!(editor.redo()); assert_eq!(editor.project,painted);
}
