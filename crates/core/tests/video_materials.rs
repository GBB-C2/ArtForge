use poseforge_core::*;
fn clip() -> VideoTexture { VideoTexture {texture:1,width:2,height:2,fps:24,offsets:(0..=24).map(|i|i*4).collect(),data:[0xff,0xd8,0xff,0xd9].repeat(24).into(),playback:Default::default()} }
#[test]
fn timeline_start_offset_speed_loop_hold_and_disabled_are_deterministic() {
    let mut v=clip(); v.validate().unwrap(); assert_eq!(v.frame_at(0.5),12); assert_eq!(v.frame_at(1.),0);
    v.playback.start=2.; v.playback.offset=0.25; v.playback.speed=2.; assert_eq!(v.frame_at(0.),6); assert_eq!(v.frame_at(2.25),18); assert_eq!(v.frame_at(2.5),6);
    v.playback.looped=false; assert_eq!(v.frame_at(10.),23); v.playback.enabled=false; assert_eq!(v.frame_at(10.),6); assert_eq!(v.frame_at(f64::NAN),0);
}
#[test]
fn encoded_frames_roundtrip_and_metadata_validation_rejects_corruption() {
    let v=clip(); let json=serde_json::to_value(&v).unwrap(); assert!(json["data"].is_string());
    let restored:VideoTexture=serde_json::from_value(json).unwrap(); assert_eq!(restored,v); restored.validate().unwrap();
    for change in 0..7 { let mut bad=v.clone(); match change {0=>bad.offsets[1]=0,1=>bad.width=0,2=>bad.width=1025,3=>bad.fps=0,4=>bad.playback.speed=f64::NAN,5=>bad.data[0]=0,_=>bad.offsets.pop().map(|_|()).unwrap()}; assert!(bad.validate().is_err()); }
}
#[test]
fn project_requires_unique_linked_video_and_consistent_dimensions() {
    let mut p=Project::default(); let v=clip(); p.video_textures.push(v.clone()); assert!(p.validate().is_err());
    p.textures.push(Texture { id:1,name:"Clip".into(),width:2,height:2,rgba8:vec![255;16].into() }); p.validate().unwrap();
    p.video_textures.push(v); assert!(p.validate().is_err()); p.video_textures.pop(); p.textures[0].width=1; p.textures[0].rgba8=vec![255;8].into(); assert!(p.validate().is_err());
}
#[test]
fn npr_presets_and_new_controls_validate_and_legacy_defaults_preserve_appearance() {
    for i in 0..4 { let n=NprSettings::preset(i).unwrap(); assert!(n.enabled && n.valid()); }
    assert!(NprSettings::preset(4).is_none());
    let legacy:NprSettings=serde_json::from_str("{\"enabled\":true}").unwrap(); assert_eq!(legacy.light_wrap,0.); assert_eq!(legacy.light_color,1.); assert_eq!(legacy.halftone_strength,0.);
    for value in [f32::NAN,-0.1,1.1] {assert!(!NprSettings {halftone_strength:value,..Default::default()}.valid());}
}
