//! Reversible material authoring and explicit ownership of mesh assignments.
use crate::{AssetId, CoreError, Editor, Material, NodeId, Project};
use std::collections::BTreeSet;

/// Starting points for layered opaque and transmissive materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialPreset {
    Matte,
    Plastic,
    Metal,
    Ceramic,
    Rubber,
    Glass,
    Water,
    BrushedMetal,
    Satin,
    Velvet,
    Toon,
    Anime,
    Ink,
    PlasticineSoft,
    PlasticineSculpted,
    ClayWet,
    ClayDry,
    Terracotta,
    WatercolorLight,
    WatercolorGranulated,
    Gouache,
    Wax,
    CeramicGlazed,
    Paper,
    Cardboard,
    Pastel,
}

impl MaterialPreset {
    pub const ALL: [Self; 26] = [
        Self::Matte,
        Self::Plastic,
        Self::Metal,
        Self::Ceramic,
        Self::Rubber,
        Self::Glass,
        Self::Water,
        Self::BrushedMetal,
        Self::Satin,
        Self::Velvet,
        Self::Toon,
        Self::Anime,
        Self::Ink,
        Self::PlasticineSoft,
        Self::PlasticineSculpted,
        Self::ClayWet,
        Self::ClayDry,
        Self::Terracotta,
        Self::WatercolorLight,
        Self::WatercolorGranulated,
        Self::Gouache,
        Self::Wax,
        Self::CeramicGlazed,
        Self::Paper,
        Self::Cardboard,
        Self::Pastel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Matte => "Матовый",
            Self::Plastic => "Пластик",
            Self::Metal => "Металл",
            Self::Ceramic => "Керамика",
            Self::Rubber => "Резина",
            Self::Glass => "Стекло",
            Self::Water => "Вода",
            Self::BrushedMetal => "Шлифованный металл",
            Self::Satin => "Сатин",
            Self::Velvet => "Бархат",
            Self::Toon => "NPR · Toon",
            Self::Anime => "NPR · Anime",
            Self::Ink => "NPR · Тушь",
            Self::PlasticineSoft => "Пластилин · мягкий",
            Self::PlasticineSculpted => "Пластилин · следы лепки",
            Self::ClayWet => "Глина · влажная",
            Self::ClayDry => "Глина · сухая",
            Self::Terracotta => "Терракота",
            Self::WatercolorLight => "Акварель · лёгкая",
            Self::WatercolorGranulated => "Акварель · грануляция",
            Self::Gouache => "Гуашь · матовая",
            Self::Wax => "Воск · мягкий",
            Self::CeramicGlazed => "Керамика · глазурь",
            Self::Paper => "Бумага · волокна",
            Self::Cardboard => "Картон · крафт",
            Self::Pastel => "Пастель · сухая",

        }
    }

    pub fn category(self) -> &'static str {
        match self {
            Self::Matte | Self::Plastic | Self::Ceramic | Self::Rubber => "Базовые",
            Self::Metal | Self::BrushedMetal => "Металлы",
            Self::Glass | Self::Water => "Стекло и жидкость",
            Self::Satin | Self::Velvet => "Ткани",
            Self::Toon | Self::Anime | Self::Ink => "NPR",
            Self::PlasticineSoft | Self::PlasticineSculpted | Self::ClayWet | Self::ClayDry | Self::Terracotta => "Лепка",
            Self::WatercolorLight | Self::WatercolorGranulated | Self::Gouache | Self::Pastel => "Живопись",
            Self::Wax | Self::CeramicGlazed | Self::Paper | Self::Cardboard => "Ремесло",
        }
    }
    fn artistic_settings(self) -> Option<crate::ArtisticSettings> {
        use crate::{ArtisticSettings, ArtisticStyle};
        Some(match self {
            Self::PlasticineSoft => ArtisticSettings { bump: 0.22, tool_marks: 0.18, subsurface: 0.2, ..ArtisticSettings::for_style(ArtisticStyle::Plasticine) },
            Self::PlasticineSculpted => ArtisticSettings { bump: 0.65, tool_marks: 0.85, variation: 0.22, grain: 0.35, ..ArtisticSettings::for_style(ArtisticStyle::Plasticine) },
            Self::ClayWet => ArtisticSettings { bump: 0.35, grain: 0.3, tool_marks: 0.6, ..ArtisticSettings::for_style(ArtisticStyle::Clay) },
            Self::ClayDry => ArtisticSettings { bump: 0.7, grain: 0.85, cracks: 0.5, ..ArtisticSettings::for_style(ArtisticStyle::Clay) },
            Self::Terracotta => ArtisticSettings { bump: 0.5, grain: 0.8, cracks: 0.22, tool_marks: 0.15, ..ArtisticSettings::for_style(ArtisticStyle::Clay) },
            Self::WatercolorLight => ArtisticSettings { pigment: 0.52, wash: 0.7, grain: 0.25, ..ArtisticSettings::for_style(ArtisticStyle::Watercolor) },
            Self::WatercolorGranulated => ArtisticSettings { pigment: 0.78, grain: 0.95, paper: 0.85, dry_brush: 0.25, edge_darkening: 0.7, ..ArtisticSettings::for_style(ArtisticStyle::Watercolor) },
            Self::Gouache => ArtisticSettings { ..ArtisticSettings::for_style(ArtisticStyle::Gouache) },
            Self::Wax => ArtisticSettings { ..ArtisticSettings::for_style(ArtisticStyle::Wax) },
            Self::CeramicGlazed => ArtisticSettings { ..ArtisticSettings::for_style(ArtisticStyle::Ceramic) },
            Self::Paper => ArtisticSettings { ..ArtisticSettings::for_style(ArtisticStyle::Paper) },
            Self::Cardboard => ArtisticSettings { scale: 4., bump: 0.35, grain: 0.9, variation: 0.3, subsurface: 0., ..ArtisticSettings::for_style(ArtisticStyle::Paper) },
            Self::Pastel => ArtisticSettings { ..ArtisticSettings::for_style(ArtisticStyle::Pastel) },
            _ => return None,
        })
    }

    /// A fresh material starts without map links; colors are scene-linear RGB.
    pub fn create(self) -> Material {
        let mut material = Material {
            name: self.label().into(),
            base_color: match self {
                Self::Matte => [0.36, 0.36, 0.36, 1.],
                Self::Plastic => [0.045, 0.18, 0.52, 1.],
                Self::Metal => [0.91, 0.92, 0.93, 1.],
                Self::Ceramic => [0.75, 0.72, 0.64, 1.],
                Self::Rubber => [0.015, 0.02, 0.025, 1.],
                Self::Glass => [0.94, 0.98, 1., 1.],
                Self::Water => [0.6, 0.88, 0.96, 1.],
                Self::BrushedMetal => [0.72, 0.55, 0.27, 1.],
                Self::Satin => [0.08, 0.22, 0.48, 1.],
                Self::Velvet => [0.26, 0.018, 0.045, 1.],
                Self::Toon => [0.055, 0.38, 0.65, 1.],
                Self::Anime => [0.7, 0.18, 0.08, 1.],
                Self::Ink => [0.88, 0.84, 0.72, 1.],
                Self::PlasticineSoft => [0.48, 0.065, 0.032, 1.],
                Self::PlasticineSculpted => [0.035, 0.22, 0.36, 1.],
                Self::ClayWet => [0.25, 0.12, 0.065, 1.],
                Self::ClayDry => [0.52, 0.36, 0.23, 1.],
                Self::Terracotta => [0.46, 0.115, 0.04, 1.],
                Self::WatercolorLight => [0.035, 0.23, 0.48, 1.],
                Self::WatercolorGranulated => [0.24, 0.055, 0.35, 1.],
                Self::Gouache => [0.5, 0.28, 0.035, 1.],
                Self::Wax => [0.76, 0.51, 0.23, 1.],
                Self::CeramicGlazed => [0.035, 0.32, 0.22, 1.],
                Self::Paper => [0.8, 0.74, 0.62, 1.],
                Self::Cardboard => [0.32, 0.19, 0.09, 1.],
                Self::Pastel => [0.44, 0.09, 0.16, 1.],

            },
            ..Material::default()
        };
        self.apply(&mut material);
        if material.artistic.enabled {
            material.category = self.category().into();
            material.tags = vec![material.artistic.style.label().into(), "художественный".into(), "процедурный".into()];
        }
        if material.npr.enabled {
            material.category = "NPR".into();
            material.tags = vec!["NPR".into(), "стилизация".into()];
        }
        material
    }

    /// Change only the surface response. Authored color, maps, emission, alpha,
    /// identity, name and double-sided behavior remain under the user's control.
    pub fn apply(self, material: &mut Material) {
        let (metallic, roughness, ior, coat_weight, coat_roughness) = match self {
            Self::Matte => (0., 0.8, 1.5, 0., 0.1),
            Self::Plastic => (0., 0.3, 1.5, 0.2, 0.18),
            Self::Metal => (1., 0.22, 1.5, 0., 0.1),
            Self::Ceramic => (0., 0.26, 1.5, 0.75, 0.12),
            Self::Rubber => (0., 0.92, 1.45, 0., 0.1),
            Self::Glass => (0., 0.06, 1.5, 0., 0.1),
            Self::Water => (0., 0.1, 1.333, 0., 0.1),
            Self::BrushedMetal => (1., 0.34, 1.5, 0.1, 0.18),
            Self::Satin => (0., 0.42, 1.48, 0., 0.1),
            Self::Velvet => (0., 0.9, 1.45, 0., 0.1),
            Self::Toon | Self::Anime | Self::Ink => (0., 0.8, 1.5, 0., 0.1),
            Self::PlasticineSoft => (0., 0.52, 1.46, 0.08, 0.4),
            Self::PlasticineSculpted => (0., 0.6, 1.46, 0.06, 0.45),
            Self::ClayWet => (0., 0.34, 1.48, 0.35, 0.24),
            Self::ClayDry => (0., 0.94, 1.45, 0., 0.1),
            Self::Terracotta => (0., 0.87, 1.5, 0., 0.1),
            Self::WatercolorLight => (0., 0.96, 1.33, 0., 0.1),
            Self::WatercolorGranulated => (0., 0.98, 1.33, 0., 0.1),
            Self::Gouache => (0., 0.96, 1.4, 0., 0.1),
            Self::Wax => (0., 0.4, 1.44, 0.1, 0.32),
            Self::CeramicGlazed => (0., 0.28, 1.5, 0.9, 0.1),
            Self::Paper => (0., 0.98, 1.4, 0., 0.1),
            Self::Cardboard => (0., 1., 1.4, 0., 0.1),
            Self::Pastel => (0., 1., 1.35, 0., 0.1),

        };
        if matches!(self, Self::Toon | Self::Anime | Self::Ink) {
            material.npr = crate::NprSettings {
                enabled: true,
                ..crate::NprSettings::default()
            };
            match self {
                Self::Anime => {
                    material.npr.bands = 2;
                    material.npr.softness = 0.;
                    material.npr.shadow_color = [0.22, 0.09, 0.19];
                    material.npr.specular_size = 0.045;
                    material.npr.specular_strength = 0.65;
                    material.npr.rim_strength = 0.3;
                    material.npr.ink_width = 0.06;
                }
                Self::Ink => {
                    material.npr.bands = 2;
                    material.npr.softness = 0.;
                    material.npr.threshold = 0.6;
                    material.npr.shadow_color = [0.012, 0.014, 0.02];
                    material.npr.specular_strength = 0.;
                    material.npr.rim_strength = 0.;
                    material.npr.ink_width = 0.16;
                }
                _ => {}
            }
        } else {
            // Switching to PBR retains the authored NPR settings for later reuse.
            material.npr.enabled = false;
        }
        material.anisotropy = match self {
            Self::BrushedMetal => 0.8,
            Self::Satin => 0.65,
            _ => 0.,
        };
        material.anisotropy_rotation = 0.;
        material.sheen_weight = match self {
            Self::Satin => 0.45,
            Self::Velvet => 1.,
            _ => 0.,
        };
        material.sheen_roughness = match self {
            Self::Satin => 0.3,
            Self::Velvet => 0.8,
            _ => 0.5,
        };
        material.transmission = if matches!(self, Self::Glass | Self::Water) {
            1.
        } else {
            0.
        };
        material.metallic = metallic;
        material.roughness = roughness;
        material.ior = ior;
        material.coat_weight = coat_weight;
        material.coat_roughness = coat_roughness;
        if let Some(settings) = self.artistic_settings() {
            material.artistic = settings;
        } else {
            material.artistic.enabled = false;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialAssignment {
    /// May differ from the input mesh when shared instances needed isolation.
    pub mesh: AssetId,
    pub material: AssetId,
}

struct AssignmentPlan {
    mesh: AssetId,
    owners: Vec<NodeId>,
    isolate: bool,
}

fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}

fn next_id(project: &Project) -> Result<AssetId, CoreError> {
    project
        .nodes
        .iter()
        .map(|node| node.id)
        .chain(project.meshes.iter().map(|mesh| mesh.id))
        .chain(project.materials.iter().map(|material| material.id))
        .chain(project.textures.iter().map(|texture| texture.id))
        .chain(project.collections.iter().map(|c| c.id))
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| invalid("закончились идентификаторы ресурсов"))
}

fn assignment_plan(
    project: &Project,
    mesh: AssetId,
    owners: &[NodeId],
) -> Result<AssignmentPlan, CoreError> {
    if project.mesh(mesh).is_none() {
        return Err(invalid("меш для назначения материала не найден"));
    }
    let all: Vec<_> = project
        .nodes
        .iter()
        .filter(|node| node.mesh == Some(mesh))
        .map(|node| node.id)
        .collect();
    // Empty means the explicitly selected resource, including all its instances.
    let targets = if owners.is_empty() {
        all.clone()
    } else {
        let unique: BTreeSet<_> = owners.iter().copied().collect();
        if unique.len() != owners.len() {
            return Err(invalid("повторяющийся владелец материала"));
        }
        for owner in owners {
            let node = project.node(*owner).ok_or(CoreError::MissingNode(*owner))?;
            if node.mesh != Some(mesh) {
                return Err(invalid("объект не использует выбранный меш"));
            }
        }
        owners.to_vec()
    };
    for owner in &targets {
        let mut cursor = Some(*owner);
        let mut seen = BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return Err(CoreError::HierarchyCycle(id));
            }
            let node = project.node(id).ok_or(CoreError::MissingNode(id))?;
            if project.node_locked(node.id) {
                return Err(invalid("объект или его родитель заблокирован"));
            }
            cursor = node.parent;
        }
    }
    Ok(AssignmentPlan {
        mesh,
        isolate: !targets.is_empty() && targets.len() < all.len(),
        owners: targets,
    })
}

fn assign(project: &mut Project, plan: &AssignmentPlan, assignment: MaterialAssignment) {
    if plan.isolate {
        // Keep vertex weights, morph targets and all other geometry byte-for-byte.
        let mut mesh = project.mesh(plan.mesh).unwrap().clone();
        mesh.id = assignment.mesh;
        mesh.material = Some(assignment.material);
        mesh.clear_face_materials();
        project.meshes.push(mesh);
        for owner in &plan.owners {
            project.node_mut(*owner).unwrap().mesh = Some(assignment.mesh);
        }
    } else {
        project
            .meshes
            .iter_mut()
            .find(|mesh| mesh.id == plan.mesh)
            .unwrap()
            .material = Some(assignment.material);
        project
            .meshes
            .iter_mut()
            .find(|m| m.id == plan.mesh)
            .unwrap()
            .clear_face_materials();
    }
}

fn unique_name(project: &Project, base: &str) -> String {
    if !project
        .materials
        .iter()
        .any(|material| material.name == base)
    {
        return base.into();
    }
    for suffix in 2..=project.materials.len() + 2 {
        let candidate = format!("{base} ({suffix})");
        if !project
            .materials
            .iter()
            .any(|material| material.name == candidate)
        {
            return candidate;
        }
    }
    unreachable!("more candidate names than existing materials")
}

impl Editor {
    /// Replace one assigned slot, keeping all other polygon assignments intact.
    pub fn replace_assigned_material(
        &mut self,
        mesh: AssetId,
        owners: &[NodeId],
        source: AssetId,
        material: AssetId,
    ) -> Result<MaterialAssignment, CoreError> {
        let plan = assignment_plan(&self.project, mesh, owners)?;
        if self.project.material(material).is_none()
            || !self
                .project
                .mesh(mesh)
                .unwrap()
                .assigned_materials()
                .contains(&source)
        {
            return Err(invalid(
                "материал отсутствует или больше не назначен объекту",
            ));
        }
        let output = if plan.isolate {
            next_id(&self.project)?
        } else {
            mesh
        };
        let assignment = MaterialAssignment {
            mesh: output,
            material,
        };
        if source == material {
            return Ok(MaterialAssignment { mesh, material });
        }
        let mut edited = self.project.mesh(mesh).unwrap().clone();
        edited.id = output;
        if edited.material == Some(source) {
            edited.material = Some(material);
        }
        if let Some(topology) = &mut edited.topology {
            for slot in topology.face_materials.iter_mut() {
                if *slot == Some(source) {
                    *slot = Some(material);
                }
            }
        }
        self.transact(
            "Заменить назначенный материал",
            |p| {
                if plan.isolate {
                    p.meshes.push(edited);
                    for owner in &plan.owners {
                        p.node_mut(*owner).unwrap().mesh = Some(output);
                    }
                } else {
                    *p.meshes.iter_mut().find(|m| m.id == mesh).unwrap() = edited;
                }
                for owner in &plan.owners {
                    if let Some(order) = p.outliner.materials.get_mut(owner) {
                        for id in order {
                            if *id == source {
                                *id = material;
                            }
                        }
                    }
                }
            },
        )?;
        Ok(assignment)
    }
    /// Detach only the selected owners; other instances and the library asset survive.
    pub fn unassign_material(
        &mut self,
        mesh: AssetId,
        owners: &[NodeId],
    ) -> Result<AssetId, CoreError> {
        let plan = assignment_plan(&self.project, mesh, owners)?;
        if self
            .project
            .mesh(mesh)
            .unwrap()
            .assigned_materials()
            .is_empty()
        {
            return Ok(mesh);
        }
        let output = if plan.isolate {
            next_id(&self.project)?
        } else {
            mesh
        };
        self.transact("Снять материал с объекта", |p| {
            if plan.isolate {
                let mut copy = p.mesh(mesh).unwrap().clone();
                copy.id = output;
                copy.material = None;
                copy.clear_face_materials();
                p.meshes.push(copy);
                for owner in &plan.owners {
                    p.node_mut(*owner).unwrap().mesh = Some(output);
                }
            } else {
                p.meshes.iter_mut().find(|m| m.id == mesh).unwrap().material = None;
                p.meshes
                    .iter_mut()
                    .find(|m| m.id == mesh)
                    .unwrap()
                    .clear_face_materials();
            }
        })?;
        Ok(output)
    }
    /// Remove one material slot from explicit owners, preserving other face assignments.
    pub fn unassign_material_slot(&mut self, mesh: AssetId, owners: &[NodeId], source: AssetId)
        -> Result<AssetId, CoreError>
    {
        let plan = assignment_plan(&self.project, mesh, owners)?;
        let original = self.project.mesh(mesh).unwrap();
        if !original.assigned_materials().contains(&source) { return Err(invalid("материал больше не назначен объекту")); }
        let output = if plan.isolate { next_id(&self.project)? } else { mesh };
        let mut edited = original.clone();
        edited.id = output;
        // Material None is the neutral default. Preserve the old fallback on all
        // other faces before clearing it, so removing a face slot changes only that slot.
        if let Some(topology) = &mut edited.topology {
            topology.face_materials.resize(topology.faces.len(), None);
            for slot in topology.face_materials.iter_mut() {
                let effective = (*slot).or(edited.material);
                *slot = effective.filter(|id| *id != source);
            }
        }
        edited.material = None;
        self.transact("Снять назначение материала", |p| {
            if plan.isolate {
                p.meshes.push(edited);
                for owner in &plan.owners { p.node_mut(*owner).unwrap().mesh = Some(output); }
            } else { *p.meshes.iter_mut().find(|m| m.id == mesh).unwrap() = edited; }
            for owner in &plan.owners {
                if let Some(order) = p.outliner.materials.get_mut(owner) { order.retain(|id| *id != source); }
            }
        })?;
        Ok(output)
    }
    /// Keep every mesh reference valid, including meshes with no scene instances.
    pub fn remove_unused_materials(&mut self) -> Result<usize, CoreError> {
        let used: BTreeSet<_> = self
            .project
            .meshes
            .iter()
            .flat_map(|m| m.assigned_materials())
            .chain(self.project.nodes.iter().flat_map(|n| n.material_layers.iter().map(|l| l.material)))
            .chain(self.project.nodes.iter().filter_map(|n| n.destruction.and_then(|d| d.interior_material)))
            .chain(self.project.materials.iter().filter(|m| m.in_library).map(|m| m.id))
            .collect();
        let count = self
            .project
            .materials
            .iter()
            .filter(|m| !used.contains(&m.id))
            .count();
        if count > 0 {
            self.transact(
                "Убрать неиспользуемые материалы",
                |p| p.materials.retain(|m| used.contains(&m.id)),
            )?;
        }
        Ok(count)
    }

    /// A library asset has no implicit scene assignment.
    pub fn create_library_material(
        &mut self,
        preset: MaterialPreset,
    ) -> Result<AssetId, CoreError> {
        let mut material = preset.create();
        material.id = next_id(&self.project)?;
        material.name = unique_name(&self.project, &material.name);
        let id = material.id;
        material.in_library = true;
        self.transact(
            "Создать материал в библиотеке",
            |p| p.materials.push(material),
        )?;
        Ok(id)
    }

    /// Explicit polygon paint wins over object-wide overlays. None targets all
    /// faces (including disconnected islands) of this one object instance.
    pub fn assign_material_region(
        &mut self, node: NodeId, material: AssetId, faces: Option<&BTreeSet<usize>>,
    ) -> Result<MaterialAssignment, CoreError> {
        if self.has_staged() { return Err(CoreError::UncommittedPose); }
        let mesh = self.project.node(node).and_then(|n|n.mesh).ok_or_else(||invalid("Выберите меш"))?;
        let previous_base = self.project.mesh(mesh).and_then(|mesh| mesh.material);
        let mut prepared = Editor::new(self.project.clone());
        let assignment = if let Some(faces) = faces { prepared.assign_face_material(mesh,&[node],faces,material)? }
            else { prepared.assign_material(mesh,&[node],material)? };
        let layers = &mut prepared.project.node_mut(node).unwrap().material_layers;
        layers.retain(|layer|layer.material!=material);
        // Keep the previous stack available, but a whole-mesh drop explicitly
        // replaces its visible result. Face paint only overrides the painted faces.
        if faces.is_none() {
            if let Some(previous) = previous_base.filter(|id| *id != material) {
                if !layers.iter().any(|layer| layer.material == previous) {
                    if layers.len() >= 64 { return Err(invalid("Максимум 64 сохранённых слоя материала")); }
                    layers.insert(0, crate::MaterialLayer { material: previous, visible: false });
                }
            }
            for layer in layers { layer.visible = false; }
        }
        self.transact("Назначить материал области",move |p|*p=prepared.project)?;
        Ok(assignment)
    }

    /// Remove this material only from the supplied polygons of one instance.
    /// Explicit overrides reveal the base. Removing the base itself preserves
    /// its assignments on all other polygons before clearing the fallback.
    pub fn unassign_face_material(
        &mut self, node: NodeId, material: AssetId, faces: &BTreeSet<usize>,
    ) -> Result<AssetId, CoreError> {
        if self.has_staged() { return Err(CoreError::UncommittedPose); }
        let mesh = self.project.node(node).and_then(|n| n.mesh)
            .ok_or_else(|| invalid("Выберите меш"))?;
        let plan = assignment_plan(&self.project, mesh, &[node])?;
        let source = self.project.mesh(mesh).unwrap();
        if self.project.material(material).is_none() || faces.is_empty()
            || faces.iter().any(|&face| face >= source.face_count()) {
            return Err(invalid("Выберите существующий материал и грани меша"));
        }
        let targets: Vec<_> = faces.iter().copied()
            .filter(|&face| source.face_material(face) == Some(material)).collect();
        if targets.is_empty() { return Ok(mesh); }
        let output = if plan.isolate { next_id(&self.project)? } else { mesh };
        let mut copy = source.clone();
        copy.id = output;
        if copy.topology.is_none() {
            copy.topology = Some(crate::MeshTopology { loose_edges: Vec::new(),
                faces: copy.indices.chunks_exact(3).map(|face| face.to_vec()).collect(),
                face_materials: Default::default(), uv_seams: Vec::new(),
                smooth_normals: false, primitive: None,
            });
        }
        let topology = copy.topology.as_mut().unwrap();
        topology.primitive = None;
        topology.face_materials.resize(topology.faces.len(), None);
        if copy.material == Some(material) {
            for slot in topology.face_materials.iter_mut() { *slot = (*slot).or(copy.material); }
            copy.material = None;
        }
        for face in targets { topology.face_materials[face] = None; }
        self.transact("Удалить назначение полигонов", |project| {
            if plan.isolate {
                project.meshes.push(copy);
                project.node_mut(node).unwrap().mesh = Some(output);
            } else {
                *project.meshes.iter_mut().find(|mesh| mesh.id == output).unwrap() = copy;
            }
        })?;
        Ok(output)
    }

    pub fn assign_face_material(
        &mut self,
        mesh: AssetId,
        owners: &[NodeId],
        faces: &BTreeSet<usize>,
        material: AssetId,
    ) -> Result<MaterialAssignment, CoreError> {
        let plan = assignment_plan(&self.project, mesh, owners)?;
        let source = self.project.mesh(mesh).unwrap();
        if self.project.material(material).is_none()
            || faces.is_empty()
            || faces.iter().any(|&f| f >= source.face_count())
        {
            return Err(invalid("Выберите существующий материал и грани меша"));
        }
        // Painting the base material is still an explicit face assignment: it
        // must cover object-wide layers just like a different material would.
        if faces.iter().all(|&f| {
            source.topology.as_ref().and_then(|t| t.face_materials.get(f))
                == Some(&Some(material))
        })
        {
            return Ok(MaterialAssignment { mesh, material });
        }
        let output = if plan.isolate {
            next_id(&self.project)?
        } else {
            mesh
        };
        let mut copy = source.clone();
        copy.id = output;
        if copy.topology.is_none() {
            copy.topology = Some(crate::MeshTopology { loose_edges: Vec::new(),
                faces: copy
                    .indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|f| f.to_vec())
                    .collect(),
                face_materials: Default::default(),
                uv_seams: Vec::new(),
                smooth_normals: false,
                primitive: None,
            });
        }
        let t = copy.topology.as_mut().unwrap();
        t.primitive = None;
        t.face_materials.resize(t.faces.len(), None);
        for &face in faces {
            t.face_materials[face] = Some(material);
        }
        self.transact("Назначить материал граням", |p| {
            if plan.isolate {
                p.meshes.push(copy);
                for owner in &plan.owners {
                    p.node_mut(*owner).unwrap().mesh = Some(output);
                }
            } else {
                *p.meshes.iter_mut().find(|m| m.id == mesh).unwrap() = copy;
            }
        })?;
        Ok(MaterialAssignment {
            mesh: output,
            material,
        })
    }

    /// Create and assign in one undo step. Nonempty owners isolate just these
    /// instances; empty owners explicitly target every user of the mesh resource.
    pub fn create_material(
        &mut self,
        mesh: AssetId,
        owners: &[NodeId],
        preset: MaterialPreset,
    ) -> Result<MaterialAssignment, CoreError> {
        let plan = assignment_plan(&self.project, mesh, owners)?;
        let mut material = preset.create();
        material.id = next_id(&self.project)?;
        material.name = unique_name(&self.project, &material.name);
        let assignment = MaterialAssignment {
            mesh: if plan.isolate {
                material
                    .id
                    .checked_add(1)
                    .ok_or_else(|| invalid("закончились идентификаторы ресурсов"))?
            } else {
                mesh
            },
            material: material.id,
        };
        self.transact("Создать материал", |project| {
            project.materials.push(material);
            assign(project, &plan, assignment);
        })?;
        Ok(assignment)
    }

    pub fn assign_material(
        &mut self,
        mesh: AssetId,
        owners: &[NodeId],
        material: AssetId,
    ) -> Result<MaterialAssignment, CoreError> {
        if self.project.material(material).is_none() {
            return Err(invalid("материал для назначения не найден"));
        }
        let plan = assignment_plan(&self.project, mesh, owners)?;
        if self.project.mesh(mesh).unwrap().material == Some(material)
            && self
                .project
                .mesh(mesh)
                .unwrap()
                .topology
                .as_ref()
                .is_none_or(|t| t.face_materials.is_empty())
        {
            return Ok(MaterialAssignment { mesh, material });
        }
        let assignment = MaterialAssignment {
            mesh: if plan.isolate {
                next_id(&self.project)?
            } else {
                mesh
            },
            material,
        };
        self.transact("Назначить материал", |project| {
            assign(project, &plan, assignment)
        })?;
        Ok(assignment)
    }

    /// Duplicate the entire material, retaining map references and surface
    /// settings, and assign the copy to the explicit ownership scope.
    pub fn duplicate_material(
        &mut self,
        mesh: AssetId,
        owners: &[NodeId],
    ) -> Result<MaterialAssignment, CoreError> {
        let material = self
            .project
            .mesh(mesh)
            .and_then(|m| m.material)
            .ok_or_else(|| invalid("Нет материала для копирования"))?;
        self.duplicate_assigned_material(mesh, owners, material)
    }

    pub fn duplicate_library_material(&mut self, source: AssetId) -> Result<AssetId, CoreError> {
        let mut material = self
            .project
            .material(source)
            .cloned()
            .ok_or_else(|| invalid("Материал не найден"))?;
        material.in_library = true;
        material.id = next_id(&self.project)?;
        material.name = unique_name(
            &self.project,
            &format!(
                "{} · копия",
                material.name.chars().take(110).collect::<String>()
            ),
        );
        let id = material.id;
        self.transact(
            "Копия материала в библиотеке",
            |p| p.materials.push(material),
        )?;
        Ok(id)
    }

    pub fn duplicate_assigned_material(
        &mut self,
        mesh: AssetId,
        owners: &[NodeId],
        source: AssetId,
    ) -> Result<MaterialAssignment, CoreError> {
        let plan = assignment_plan(&self.project, mesh, owners)?;
        if !self
            .project
            .mesh(mesh)
            .unwrap()
            .assigned_materials()
            .contains(&source)
        {
            return Err(invalid("Материал не назначен этому мешу"));
        }
        let mut material = self
            .project
            .material(source)
            .cloned()
            .ok_or_else(|| invalid("Материал не найден"))?;
        material.id = next_id(&self.project)?;
        let name: String = material.name.chars().take(110).collect();
        material.name = unique_name(&self.project, &format!("{name} · копия"));
        let assignment = MaterialAssignment {
            mesh: if plan.isolate {
                material
                    .id
                    .checked_add(1)
                    .ok_or_else(|| invalid("закончились идентификаторы ресурсов"))?
            } else {
                mesh
            },
            material: material.id,
        };
        let mut copy = self.project.mesh(mesh).unwrap().clone();
        copy.id = assignment.mesh;
        if copy.material == Some(source) {
            copy.material = Some(material.id);
        }
        if let Some(t) = &mut copy.topology {
            for id in &mut t.face_materials {
                if *id == Some(source) {
                    *id = Some(material.id);
                }
            }
        }
        self.transact("Отдельный материал", |project| {
            project.materials.push(material);
            if plan.isolate {
                project.meshes.push(copy);
                for owner in &plan.owners {
                    project.node_mut(*owner).unwrap().mesh = Some(assignment.mesh);
                }
            } else {
                *project.meshes.iter_mut().find(|m| m.id == mesh).unwrap() = copy;
            }
        })?;
        Ok(assignment)
    }

    /// Material names and response are resource edits shared by its users.
    pub fn rename_material(&mut self, material: AssetId, name: &str) -> Result<(), CoreError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control) {
            return Err(invalid(
                "имя материала: от 1 до 128 символов, без переводов строки",
            ));
        }
        if self.project.material(material).is_none() {
            return Err(invalid("материал для переименования не найден"));
        }
        self.transact("Переименовать материал", |project| {
            project
                .materials
                .iter_mut()
                .find(|m| m.id == material)
                .unwrap()
                .name = name.into();
        })
    }

    pub fn apply_material_preset(
        &mut self,
        material: AssetId,
        preset: MaterialPreset,
    ) -> Result<(), CoreError> {
        if self.project.material(material).is_none() {
            return Err(invalid("материал для настройки не найден"));
        }
        self.transact("Основа материала", |project| {
            preset.apply(
                project
                    .materials
                    .iter_mut()
                    .find(|m| m.id == material)
                    .unwrap(),
            );
        })
    }
}

impl Material {
    pub fn graph_output(&self, socket: usize) -> bool {
        self.graph
            .as_ref()
            .is_some_and(|g| g.enabled && g.outputs.get(socket).is_some_and(Option::is_some))
    }
    pub fn uses_transmission(&self) -> bool {
        !self.npr.enabled && (self.transmission > 0. || self.graph_output(5))
    }
    pub fn is_transparent(&self) -> bool {
        self.alpha_mode == crate::AlphaMode::Blend
            || self.uses_transmission()
            || self.graph_output(4)
    }
    pub fn alpha_shader_mode(&self) -> f32 {
        match self.alpha_mode {
            crate::AlphaMode::Mask => self.alpha_cutoff,
            crate::AlphaMode::Blend => -2.,
            _ if self.graph_output(4) => -2.,
            _ => -1.,
        }
    }
}
