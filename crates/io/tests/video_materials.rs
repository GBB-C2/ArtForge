use poseforge_core::*;
use poseforge_io::{decode_video_frame,load_project,save_project,merge_project};
use std::{io::Cursor,sync::atomic::AtomicBool};
fn scene() -> Project {
    let mut bytes=Vec::new(); let mut offsets=vec![0];
    for rgb in [[255,0,0],[0,0,255]] {
        let mut cursor=Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(4,4,image::Rgb(rgb))).write_to(&mut cursor,image::ImageFormat::Jpeg).unwrap();
        bytes.extend(cursor.into_inner()); offsets.push(bytes.len() as u32);
    }
    let video=VideoTexture {texture:4,width:4,height:4,fps:24,offsets,data:bytes.into(),playback:Default::default()};
    let poster=decode_video_frame(&video,0).unwrap();
    let mut n=Node::new(1,"Screen"); n.mesh=Some(2);
    Project {nodes:vec![n],meshes:vec![box_mesh(2,"Cube",Vec3::ONE,Some(3))],materials:vec![Material {id:3,base_color_texture:Some(4),..Default::default()}],textures:vec![poster],video_textures:vec![video],..Default::default()}
}
#[test]
fn portable_frames_and_playback_roundtrip_without_source_video() {
    let mut p=scene(); p.video_textures[0].playback.speed=0.5; p.materials[0].normal_y_flip=true;
    let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("video.artforge"); save_project(&p,&path).unwrap();
    let loaded=load_project(&path).unwrap(); assert_eq!(loaded,p); assert!(decode_video_frame(&loaded.video_textures[0],1).unwrap().rgba8[2]>220);
}
#[test]
fn export_uses_exact_frame_and_reuses_a_decoded_frame_without_mutating_authoring() {
    let original=scene(); let mut p=original.clone(); let mut cache=poseforge_io::video::VideoFrameCache::default(); let cancel=AtomicBool::new(false);
    cache.apply(&mut p,1./24.,true,&cancel).unwrap(); assert!(p.textures[0].rgba8[2]>220); assert!(original.textures[0].rgba8[0]>220);
    let frame=p.textures[0].rgba8.clone(); cache.apply(&mut p,1./24.,true,&cancel).unwrap(); assert!(frame.shares_storage(&p.textures[0].rgba8));
    cache.apply(&mut p,0.,true,&cancel).unwrap(); assert!(p.textures[0].rgba8[0]>220);
    assert!(cache.apply(&mut p,0.,true,&AtomicBool::new(true)).is_err());
}
#[test]
fn merged_videos_remap_the_poster_link_and_material_reference_together() {
    let mut a=scene(); let incoming=scene(); merge_project(&mut a,incoming).unwrap(); a.validate().unwrap();
    assert_eq!(a.video_textures.len(),2); assert_ne!(a.video_textures[0].texture,a.video_textures[1].texture);
    assert_eq!(a.materials[1].base_color_texture,Some(a.video_textures[1].texture));
    let id=a.video_textures[1].texture; assert!(a.textures.iter().any(|t|t.id==id));
}
#[test]
fn schema27_gets_disabled_new_effects_and_never_rewrites_the_input() {
    let mut p=scene(); p.video_textures.clear();
    let mut data=serde_json::to_value(&p).unwrap(); data["version"]=27.into(); data.as_object_mut().unwrap().remove("video_textures");
    let m=data["materials"][0].as_object_mut().unwrap(); m.remove("normal_y_flip");
    let n=m.get_mut("npr").unwrap().as_object_mut().unwrap(); for key in ["light_wrap","light_color","halftone_strength","halftone_density"] {n.remove(key);}
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("old.artforge");let bytes=serde_json::to_vec(&data).unwrap();std::fs::write(&path,&bytes).unwrap();
    let loaded=load_project(&path).unwrap();assert_eq!(loaded.version,SCHEMA_VERSION);assert!(loaded.video_textures.is_empty());assert!(!loaded.materials[0].normal_y_flip);assert_eq!(loaded.materials[0].npr.halftone_strength,0.);assert_eq!(std::fs::read(path).unwrap(),bytes);
}
#[test]
fn malformed_frames_bad_dimensions_and_cancelled_import_return_errors() {
    let mut v=scene().video_textures.remove(0); v.width=2; assert!(decode_video_frame(&v,0).is_err());
    v.width=4;v.data=vec![0xff,0xd8,0xff,0xd9].into();v.offsets=vec![0,4];assert!(decode_video_frame(&v,0).is_err());assert!(decode_video_frame(&v,1).is_err());
    let cancelled=poseforge_io::load_video(std::path::Path::new("not-present.mp4"),&AtomicBool::new(true),|_|{});assert!(matches!(cancelled,Err(poseforge_io::IoError::Cancelled)));
}
