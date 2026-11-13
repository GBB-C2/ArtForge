//! Material rows expose persistent polygon assignments, independent of transient selection.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MaterialRow { Material(u64), Owner(u64, u64), Unused }
pub(super) type FaceGroupsCache = (Option<u64>, u64, usize, Arc<BTreeMap<u64, BTreeSet<usize>>>);
impl App {
    pub(super) fn material_face_groups(&self, mesh: &poseforge_core::Mesh) -> Arc<BTreeMap<u64, BTreeSet<usize>>> {
        let revision = mesh.topology.as_ref().map_or(0, |t| t.face_materials.revision());
        let count = mesh.face_count();
        let mut cache = self.surface.material_face_groups.borrow_mut();
        if let Some((base, old_revision, old_count, groups)) = cache.get(&mesh.id) {
            if *base == mesh.material && *old_revision == revision && *old_count == count { return groups.clone(); }
        }
        if cache.len() >= 4096 || cache.len() > self.editor.project.meshes.len() + 16 { cache.clear(); }
        let mut groups = BTreeMap::<u64, BTreeSet<usize>>::new();
        for face in 0..count {
            if let Some(id) = mesh.face_material(face) { groups.entry(id).or_default().insert(face); }
        }
        let groups = Arc::new(groups);
        cache.insert(mesh.id, (mesh.material, revision, count, groups.clone())); groups
    }
    pub(super) fn material_user_faces(&self, node: u64, material: u64) -> BTreeSet<usize> {
        self.editor.project.node(node).and_then(|n| n.mesh).and_then(|id| self.editor.project.mesh(id))
            .and_then(|mesh| self.material_face_groups(mesh).get(&material).cloned()).unwrap_or_default()
    }
    pub(super) fn material_users(&self, material: u64) -> Vec<u64> {
        self.editor.project.nodes.iter().filter(|node| node.mesh.and_then(|id| self.editor.project.mesh(id))
            .is_some_and(|mesh| self.material_face_groups(mesh).contains_key(&material))).map(|n| n.id).collect()
    }
    pub(super) fn select_material_faces(&mut self, node: u64, material: u64) {
        if self.job.is_some() || self.has_modal() || self.native_picker.is_some() || self.editor.has_staged() { return; }
        if !self.node_selectable(node) { self.status = "Меш скрыт или заблокирован".into(); return; }
        if let Err(error) = self.editor.editable_mesh(node) { self.status = error.to_string(); return; }
        self.cancel_drag(); self.cancel_modeling_live();
        let faces = self.material_user_faces(node, material);
        if faces.is_empty() { self.status = "У материала нет назначенных полигонов этого меша".into(); return; }
        self.action(Action::SceneMaterial(node, material));
        self.surface.material_click = None;
        if self.authoring.editing != Some(node) { self.authoring.editing = None; }
        self.authoring_action(ui::AuthorAction::SelectMode(ui::MeshSelection::Face));
        if self.authoring.editing != Some(node) { return; }
        self.authoring.vertices.clear(); self.authoring.edges.clear(); self.authoring.faces = faces;
        self.set_properties_tab(1);
        self.status = format!("Выбрано полигонов материала: {}", self.authoring.faces.len()); self.redraw();
    }
    pub(super) fn remove_material_faces(&mut self, node: u64, material: u64, faces: &BTreeSet<usize>) {
        if self.job.is_some() || self.has_modal() || self.native_picker.is_some() || self.editor.has_staged()
            || self.editor.has_active_edit() || self.editor.playing { return; }
        let result = self.editor.unassign_face_material(node, material, faces);
        if let Some(mesh) = self.result(result) {
            if self.editor.selected == Some(node) {
                self.surface.mesh = Some(mesh); self.surface.selection = Some(node);
                self.surface.object_material = Some((mesh, material));
                if !self.material_list_ids().contains(&material) {
                    self.surface.material_library = true; self.surface.library_material = Some(material);
                }
            }
            self.status = "Назначение выбранных полигонов удалено · Ctrl+Z — отменить".into();
            self.invalidate_geometry();
        }
    }
    pub(super) fn material_tree_rows(&self, ids: &[u64]) -> Vec<MaterialRow> {
        let mut rows = Vec::new();
        for &id in ids {
            rows.push(MaterialRow::Material(id));
            if self.surface.expanded_materials.contains(&id) {
                let owners = self.material_users(id);
                if owners.is_empty() { rows.push(MaterialRow::Unused); }
                rows.extend(owners.into_iter().map(|node| MaterialRow::Owner(node, id)));
            }
        }
        rows
    }
    pub(super) fn material_rows(&self) -> Vec<MaterialRow> { self.material_tree_rows(&self.material_list_ids()) }
    pub(super) fn draw_material_tree_row(&self, u: &mut Ui, r: [f32; 4], row: MaterialRow, assigned: bool) {
        let p = u.palette();
        match row {
            MaterialRow::Material(id) => {
                let Some(material) = self.editor.project.material(id) else { return; };
                u.menu_row(r, if assigned { Action::MaterialSelect(id) } else { Action::AssetMaterial(id) }, self.selected_material_id() == Some(id));
                u.disclosure_button([r[0], r[1] + 7., 20., 24.], Some(Action::MaterialUsers(id)), self.surface.expanded_materials.contains(&id));
                self.material_previews.draw(u, [r[0] + 22., r[1] + 7., 24., 24.], material, &self.editor.project.textures);
                let name_rect = [r[0] + 51., r[1] + 4., (r[2] - if assigned { 102. } else { 78. }).max(16.), 30.];
                if self.selected_material_id() == Some(id) && self.surface.material_library != assigned && self.input.as_ref().is_some_and(|i| i.purpose == Action::SurfaceMaterialName) {
                    u.field(name_rect, &material.name, Action::SurfaceMaterialName);
                } else { u.fitted_label([name_rect[0], r[1] + 11., name_rect[2], 16.], &material.name, 12., p.text); }
                if assigned {
                    if let Some(owner) = self.material_layer_owner() {
                        let layer = self.editor.project.node(owner).and_then(|n| n.material_layers.iter().find(|l| l.material == id));
                        u.eye_button([r[0] + r[2] - 48., r[1] + 7., 24., 24.],
                            if layer.is_some() { Action::MaterialLayerVisible(owner, id) } else { Action::SceneMaterialVisible(id) }, material.visible && layer.is_none_or(|l| l.visible));
                        if !self.editor.project.node_locked(owner) { u.scene_close_button([r[0] + r[2] - 24., r[1] + 7., 24., 24.], Action::SceneMaterialDetach(owner, id)); }
                    }
                } else {
                    u.scene_close_button([r[0] + r[2] - 24., r[1] + 7., 24., 24.], Action::MaterialLibraryRemove(id));
                }
            }
            MaterialRow::Owner(node, material) => {
                let Some(n) = self.editor.project.node(node) else { return; };
                let Some(mesh) = n.mesh.and_then(|id| self.editor.project.mesh(id)) else { return; };
                let groups = self.material_face_groups(mesh);
                let Some(faces) = groups.get(&material) else { return; };
                let selected = self.authoring.editing == Some(node) && self.authoring.selection_mode == ui::MeshSelection::Face
                    && !faces.is_empty() && self.authoring.faces == *faces;
                if self.node_selectable(node) && self.editor.editable_mesh(node).is_ok() {
                    u.menu_row(r, Action::MaterialUserSelect(node, material), selected);
                }
                Icon::Face.draw(u, [r[0] + 24., r[1] + 11., 16., 16.], p.muted);
                let label = format!("{} · {} полиг.", n.name, faces.len());
                u.fitted_label([r[0] + 46., r[1] + 11., (r[2] - 74.).max(16.), 16.], &label, 11., p.text);
                if !self.editor.project.node_locked(node) {
                    u.scene_close_button([r[0] + r[2] - 24., r[1] + 7., 24., 24.], Action::MaterialUserDetach(node, material));
                }
            }
            MaterialRow::Unused => u.fitted_label([r[0] + 24., r[1] + 11., r[2] - 30., 16.], "Нет назначенных полигонов", 11., p.muted),
        }
    }
}
