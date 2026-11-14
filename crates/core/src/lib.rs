//! Authoring data, evaluated scene state, animation, and reversible editing.
//! This crate deliberately has no window or GPU handles.
pub mod animation;
pub mod authoring;
pub use authoring::*;
pub mod camera;
pub mod studio;
pub use studio::*;
mod demo;
mod bone_display;
mod modifiers;
mod modifier_geometry;
mod advanced_modifiers;
mod advanced_bridge;
mod knife_stroke;
mod knife_caps;
mod knife_interior;
mod extrude_manifold;
mod modeling_workflow;
pub use modeling_workflow::loop_cut_path;
pub use knife_stroke::KnifeCut;
mod mesh_cleanup;
pub use mesh_cleanup::{MeshRegion,TopologyEdit,RepairKind};
pub use modifiers::*;
pub use bone_display::{BoneDisplay, BoneDisplayPreset, BoneShape, JointShape};
mod humanoid_template;
pub use humanoid_template::HUMANOID_JOINT_COUNT;
mod curve_binding;
mod collision_targets;
pub use collision_targets::{CollisionKind, CollisionTarget, CollisionTargets};
pub use curve_binding::{CurveBinding, CurveTranslation};
pub mod document;
pub mod editor;
pub mod humanoid;
pub mod lighting;
mod outliner;
pub use outliner::{Outliner, SceneItem};
pub mod material;
mod artistic;
pub use artistic::{ArtisticSettings, ArtisticStyle};
pub use lighting::*;
pub mod rig;
pub use animation::*;
pub use camera::*;
pub use demo::box_mesh;
pub use document::*;
pub use editor::*;
pub use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
pub use humanoid::*;
pub use material::*;
mod texture_sampler;
pub use texture_sampler::*;
mod texture_uv;
pub use texture_uv::TextureUvTransform;
mod npr;
pub use npr::{NprContours, NprSettings};
pub use rig::*;

mod environment;
mod material_graph;
pub use environment::*;
pub use material_graph::*;
mod shared_buffer;
pub use shared_buffer::SharedBuffer;

mod collections;
pub use collections::*;
mod topology;
pub use topology::*;
mod fox_primitive;
mod primitive;
mod advanced_primitives;
mod helpers;
pub use helpers::*;
mod curve_primitive;
mod curves;
pub use curves::*;
pub use primitive::*;
mod connectivity;
mod mesh_modeling;
mod component_mesh;
mod polygon_edit;
pub use connectivity::MeshConnectivity;

mod triangulate;
pub use triangulate::MAX_FACE_CORNERS;
mod mesh_topology_tools;
pub use mesh_topology_tools::{inspect_topology, select_edge_path, TopologyReport};
mod edge_bevel;
mod face_bevel;
mod vertex_bevel;

mod uv_edit;
pub use uv_edit::{edit_uv, uv_islands, uv_seams, uv_stretch, UvEdit};

mod mesh_advanced;
pub use mesh_advanced::{proportional_weights, EdgePoint};

mod mesh_normals;
pub use mesh_normals::NormalEdit;
mod mesh_shape;
pub use mesh_shape::ShapeEdit;

pub mod fracture;
pub mod fx;
mod fx_profiles;
pub use fx_profiles::{FxQuality,FxEnvelope,FxKey};
mod fx_pyro;
pub use fx_pyro::{FxPyro,FxEmission};
pub use fx::{FxKind, FxSettings, MAX_FX};
mod material_layers;
pub use material_layers::*;
mod destruction;
pub use destruction::*;
pub use fracture::{fracture_mesh, fracture_mesh_with_cuts, fracture_mesh_with_material, Fragment};

pub mod video;
pub use video::{VideoPlayback, VideoTexture};

pub mod morph;
pub use morph::morph_compatible;

mod text_object;
mod text_path;
pub use text_object::{TextSettings,TextFont};
