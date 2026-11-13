//! Material metadata, library layout and drag assignment share the command layer.
use super::*;
#[derive(Clone)]
pub(super) struct MaterialDrag {
    pub action: Action,
    pub start: [f32; 2],
    pub target: Option<u64>,
    pub add_layer: bool,
}
impl App {
    pub(super) fn material_layer_owner(&self) -> Option<u64> {
        let mesh = self.surface_mesh_id()?;
        self.material_owners(mesh).first().copied()
    }
    pub(super) fn select_material_click(&mut self, id: u64) {
        let now = Instant::now();
        let double = self.surface.material_click.is_some_and(|(time, material, point)| {
            material == id && now.duration_since(time) < Duration::from_millis(350)
                && (point[0] - self.mouse[0]).hypot(point[1] - self.mouse[1]) < 5.
        });
        self.surface.material_click = if double { None } else { Some((now, id, self.mouse)) };
        if double { self.action(Action::SurfaceMaterialName); }
    }
    pub(super) fn append_material_layer(&mut self, source: Option<u64>) {
        if !self.material_authoring_available() || self.scene_write_pending() { return; }
        let Some(node) = self.material_layer_owner() else { return; };
        let result = self.editor.add_material_layer(node, source);
        if let Some(id) = self.result(result) {
            self.select_added_material(node, id);
            self.status = "Добавлен слой материала. Двойной щелчок — переименовать; … — библиотека и порядок.".into();
        }
    }
    pub(super) fn select_added_material(&mut self, node: u64, material: u64) {
        self.surface.material_library = false;
        self.surface.object_material = self.surface_mesh_id().map(|mesh| (mesh, material));
        // Layers are listed top-to-bottom, so the newest layer is the first row.
        self.surface.material_offset = 0;
        self.surface.material_click = None;
        self.scene_tools.material_expanded.insert(node);
        self.invalidate_geometry();
    }
    pub(super) fn add_from_material_library(&mut self, action: Action) {
        if !self.material_authoring_available() || self.scene_write_pending()
            || self.editor.has_active_edit() || self.editor.has_staged() || self.editor.playing { return; }
        let Some(node) = self.material_layer_owner() else {
            // Without a mesh, the library remains an independent asset editor.
            self.action(action); return;
        };
        let pending_faces = self.material_faces_available();
        self.editor.begin_edit("Добавить материал из библиотеки");
        let result = match action {
            Action::AssetMaterial(id) => self.editor.add_material_layer(node, Some(id)),
            Action::AssetPreset(index) => poseforge_core::MaterialPreset::ALL.get(index)
                .ok_or_else(|| poseforge_core::CoreError::Invalid("Шаблон материала удалён".into()))
                .and_then(|preset| self.editor.add_material_layer_preset(node, *preset)),
            _ => { self.editor.cancel_edit(); return; }
        }.and_then(|id| {
            if pending_faces {
                // Adding a slot must not paint unrelated faces while authoring a selection.
                self.editor.transact("Подготовить назначение полигонов", |p| {
                    if let Some(layer) = p.node_mut(node).and_then(|n| n.material_layers.iter_mut().find(|l| l.material == id)) {
                        layer.visible = false;
                    }
                })?;
            }
            Ok(id)
        });
        match result {
            Ok(id) => {
                let committed = self.editor.commit_edit();
                if self.result(committed).is_some() {
                    self.select_added_material(node, id);
                    self.status = if pending_faces {
                        "Материал добавлен в список · «Назначить» — для выделенных полигонов"
                    } else { "Добавлен отдельный слой материала · Ctrl+Z — отменить" }.into();
                }
            }
            Err(error) => { self.editor.cancel_edit(); self.status = error.to_string(); }
        }
    }
    pub(super) fn material_capacity(&self) -> usize { if self.material_list_ids().iter().any(|id| self.surface.expanded_materials.contains(id)) { self.material_rows().len().clamp(3, 7) } else { 3 } }
    pub(super) fn material_row_height(&self) -> f32 { 40. }
    pub(super) fn material_library_action(&mut self, action: &Action) -> bool {
        match action {
            Action::MaterialUsers(id) => {
                if !self.surface.expanded_materials.remove(id) { self.surface.expanded_materials.insert(*id); }
                self.surface.material_click = None; self.redraw(); return true;
            }
            Action::MaterialUserSelect(node, material) => {
                self.surface.material_click = None;
                self.select_material_faces(*node, *material);
                self.surface.material_click = None; return true;
            }
            Action::MaterialUserDetach(node, material) => {
                let faces = self.material_user_faces(*node, *material);
                if !faces.is_empty() { self.remove_material_faces(*node, *material, &faces); }
                return true;
            }
            Action::MaterialBrowse(library) => {
                self.surface.material_library = *library;
                self.surface.material_query.clear();
                self.inspector_scroll = 0.;
                self.preferences.collapsed_sections.remove(&100);
                self.preferences.collapsed_sections.remove(&101);
            }
            Action::MaterialSelect(id) => {
                if self.editor.project.material(*id).is_none() {
                    return true;
                }
                if self.surface.material_library && !self.material_list_ids().contains(id) {
                    self.surface.library_material = Some(*id);
                } else if let Some(mesh) = self.surface_mesh_id() {
                    if !self.material_list_ids().contains(id) { return true; }
                    self.surface.material_library = false;
                    self.surface.object_material = Some((mesh, *id));
                } else { return true; }
                self.select_material_click(*id);
                self.redraw();
                return true;
            }
            Action::MaterialApplyFaces | Action::MaterialUnassignFaces => {
                if !self.material_faces_available() || !self.material_authoring_available()
                    || self.editor.has_active_edit() || self.editor.has_staged() || self.editor.playing {
                    self.status = "Выберите полигоны меша и материал.".into(); return true;
                }
                let (Some(node), Some(material)) = (self.authoring.editing, self.selected_material_id()) else { return true; };
                let faces = self.authoring.faces.clone();
                if *action == Action::MaterialUnassignFaces {
                    self.remove_material_faces(node, material, &faces);
                } else {
                    let result = self.editor.assign_material_region(node, material, Some(&faces));
                    if let Some(assignment) = self.result(result) {
                        self.surface.material_library = false;
                        self.remember_material_assignment(assignment, "Материал назначен выбранным полигонам · Ctrl+Z — отменить");
                        self.invalidate_geometry();
                    }
                }
            }
            Action::MaterialLibraryToggle | Action::MaterialLibraryRemove(_) => {
                if !self.material_authoring_available() { return true; }
                let target = match action { Action::MaterialLibraryRemove(id) => Some(*id), _ => self.selected_material_id() };
                if let Some(id) = target {
                    let present = !matches!(action, Action::MaterialLibraryRemove(_))
                        && self.editor.project.material(id).is_some_and(|m| !m.in_library);
                    let result = self.editor.set_material_library(id, present);
                    self.result(result);
                    self.status = if present { "Материал добавлен в библиотеку." } else { "Материал убран из библиотеки; слои на объектах сохранены." }.into();
                }
            }
            Action::MaterialLayerVisible(node, material) => {
                if !self.material_authoring_available() || self.editor.project.node_locked(*node) { return true; }
                let Some(visible) = self.editor.project.node(*node)
                    .and_then(|n| n.material_layers.iter().find(|l| l.material == *material))
                    .map(|l| l.visible && self.editor.project.material(*material).is_some_and(|m| m.visible))
                else { return true; };
                let result = self.editor.transact("Видимость слоя материала", |p| {
                    let result=p.node(*node).and_then(|n|n.destruction).and_then(|d|d.result);
                    if let Some(layer) = p.node_mut(*node).and_then(|n| n.material_layers.iter_mut().find(|l| l.material == *material)) {
                        layer.visible = !visible;
                    }
                    if let Some(root)=result {
                        for shard in p.nodes.iter_mut().filter(|n|n.parent==Some(root)) {
                            if let Some(layer)=shard.material_layers.iter_mut().find(|l|l.material==*material) {layer.visible=!visible;}
                        }
                    }
                    if !visible {
                        if let Some(m) = p.materials.iter_mut().find(|m| m.id == *material) { m.visible = true; }
                    }
                });
                self.result(result); self.invalidate_geometry();
            }
            Action::MaterialLayerMove(direction) => {
                if !self.material_authoring_available() { return true; }
                if let (Some(node), Some(material)) = (self.material_layer_owner(), self.selected_material_id()) {
                    if self.editor.project.node_locked(node) { return true; }
                    let result = self.editor.transact("Порядок слоёв материала", |p| {
                        let layers = &mut p.node_mut(node).unwrap().material_layers;
                        if let Some(index) = layers.iter().position(|l| l.material == material) {
                            let next = (index as isize + *direction as isize).clamp(0, layers.len() as isize - 1) as usize;
                            layers.swap(index, next);
                        }
                    });
                    self.result(result); self.invalidate_geometry();
                }
            }
            Action::MaterialGrid => {
                self.preferences.material_grid = !self.preferences.material_grid
            }
            Action::MaterialSize => {
                self.preferences.material_thumbnail =
                    match self.preferences.material_thumbnail as u32 {
                        0..=36 => 52.,
                        37..=52 => 72.,
                        _ => 36.,
                    }
            }
            Action::MaterialFavorites => {
                self.preferences.material_favorites = !self.preferences.material_favorites
            }
            Action::MaterialFavorite => {
                if self.material_locked() || self.scene_write_pending() {
                    return true;
                }
                if let Some(id) = self.selected_material_id() {
                    let r = self.editor.transact("Избранный материал", |p| {
                        if let Some(m) = p.materials.iter_mut().find(|m| m.id == id) {
                            m.favorite = !m.favorite;
                        }
                    });
                    self.result(r);
                }
            }
            Action::MaterialCategory | Action::MaterialTags => {
                if self.material_locked() || self.scene_write_pending() {
                    return true;
                }
                if let Some(m) = self
                    .selected_material_id()
                    .and_then(|id| self.editor.project.material(id))
                {
                    let text = if *action == Action::MaterialCategory {
                        m.category.clone()
                    } else {
                        m.tags.join(", ")
                    };
                    self.preferences.collapsed_sections.remove(&100);
                    self.inspector_scroll = 0.;
                    self.input(action.clone(), text);
                }
            }
            _ => return false,
        }
        self.surface.material_offset = 0;
        self.persist_preferences();
        self.redraw();
        true
    }
    pub(super) fn material_metadata_confirm(
        &mut self,
        action: &Action,
        value: &str,
    ) -> Result<bool, String> {
        if !matches!(action, Action::MaterialCategory | Action::MaterialTags) {
            return Ok(false);
        }
        if self.material_locked() {
            return Err("Объект заблокирован".into());
        }
        let id = self
            .selected_material_id()
            .ok_or("Материал больше недоступен")?;
        let value = value.trim();
        if value.chars().any(char::is_control) || value.chars().count() > 512 {
            return Err("До 512 символов без управляющих знаков".into());
        }
        if *action == Action::MaterialCategory && value.chars().count() > 64 {
            return Err("Категория: до 64 символов".into());
        }
        let mut tags: Vec<String> = value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        tags.sort();
        tags.dedup();
        if *action == Action::MaterialTags
            && (tags.len() > 16 || tags.iter().any(|s| s.chars().count() > 32))
        {
            return Err("До 16 тегов, по 32 символа; разделитель — запятая".into());
        }
        let r = self.editor.transact("Сведения о материале", |p| {
            if let Some(m) = p.materials.iter_mut().find(|m| m.id == id) {
                if *action == Action::MaterialCategory {
                    m.category = value.into();
                } else {
                    m.tags = tags;
                }
            }
        });
        self.result(r).ok_or("Не удалось изменить материал")?;
        Ok(true)
    }
    pub(super) fn begin_material_drag(&mut self, action: &Action) -> bool {
        if !matches!(
            action,
            Action::MaterialAssign(..) | Action::MaterialSelect(_) | Action::AssetMaterial(_) | Action::AssetPreset(_)
        ) || self.has_modal()
            || self.job.is_some() || self.scene_write_pending() || self.editor.has_staged()
        {
            return false;
        }
        if !self.resolve_input() {
            return true;
        }
        self.drag = Some(Drag::Material(material_library::MaterialDrag {
            action: action.clone(),
            start: self.mouse,
            target: self.authoring.editing.or(self.editor.selected).filter(|id|self.editor.project.node(*id).is_some_and(|n|n.mesh.is_some())),
            add_layer: false,
        }));
        self.drag_button = Some(MouseButton::Left);
        true
    }
    pub(super) fn material_drop_scope(&self, fallback: Option<u64>) -> Option<(u64, Option<BTreeSet<usize>>)> {
        if !self.canvas_input_at(self.mouse) { return None; }
        let eval = self.editor.evaluated().ok()?;
        if let Some(hit) = crate::picking::pick_material_surface(&self.picking_project(),&eval,self.view().view_projection(),self.viewport(),self.mouse) {
            if self.editor.project.node_locked(hit.node) { return None; }
            let mesh = self.editor.project.node(hit.node)?.mesh.and_then(|id|self.editor.project.mesh(id))?;
            let faces = if self.authoring.editing==Some(hit.node) && self.authoring.selection_mode==ui::MeshSelection::Face
                && self.authoring.faces.contains(&hit.face) { self.authoring.faces.clone() }
                else { self.mesh_cache(mesh).graph.connected_faces(hit.face) };
            if faces.is_empty() { return None; }
            Some((hit.node,Some(faces)))
        } else {
            let node = fallback?;
            (self.editor.project.node(node).is_some_and(|n|n.mesh.is_some()) && !self.editor.project.node_locked(node)).then_some((node,None))
        }
    }
    pub(super) fn finish_material_drag(&mut self, d: MaterialDrag) {
        if (self.mouse[0]-d.start[0]).hypot(self.mouse[1]-d.start[1])<6. {
            if d.add_layer { self.add_from_material_library(d.action); }
            else { self.action(match d.action { Action::MaterialAssign(id,..)=>Action::AssetMaterial(id),other=>other }); }
            return;
        }
        if self.has_modal() || self.scene_write_pending() || self.job.is_some() || self.editor.has_staged() { return; }
        let material = match d.action {
            Action::MaterialAssign(id,..) | Action::MaterialSelect(id) | Action::AssetMaterial(id) => Some(id),
            Action::AssetPreset(_) => None,
            _ => return,
        };
        if let Some(material) = material {
            if self.editor.project.material(material).is_none() { return; }
            if let (Some(node),Some(Action::MaterialSelect(target))) = (self.material_layer_owner(),self.ui.hit(self.mouse)) {
                if self.editor.project.node_locked(node) { return; }
                let result = self.editor.transact("Порядок слоёв материала",|p| {
                    let layers = &mut p.node_mut(node).unwrap().material_layers;
                    if let (Some(from),Some(to)) = (layers.iter().position(|l|l.material==material),layers.iter().position(|l|l.material==target)) {
                        let layer = layers.remove(from); layers.insert(to,layer);
                    }
                }); self.result(result); self.invalidate_geometry(); return;
            }
            if self.drop_material_in_scene(None,material) { return; }
        }
        let Some((node,faces)) = self.material_drop_scope(d.target) else {
            self.status = "Перетащите на доступный меш; пустое место назначает материал всему текущему мешу".into(); return;
        };
        self.editor.begin_edit("Перетащить материал");
        let result = (|| {
            let material = if let Some(id) = material { id } else {
                let Action::AssetPreset(index) = d.action else { unreachable!() };
                let preset = *poseforge_core::MaterialPreset::ALL.get(index).ok_or_else(||poseforge_core::CoreError::Invalid("Шаблон материала удалён".into()))?;
                self.editor.create_library_material(preset)?
            };
            self.editor.assign_material_region(node,material,faces.as_ref())
        })();
        match result {
            Ok(assignment) => {
                let committed = self.editor.commit_edit();
                if self.result(committed).is_some() {
                    if self.editor.selected==Some(node) {
                        self.surface.material_library = false;
                        self.remember_material_assignment(assignment, "Материал назначен");
                    }
                    self.invalidate_geometry(); self.update_dirty();
                    self.status = match faces { Some(faces)=>format!("Материал назначен · {} граней · Ctrl+Z — отменить",faces.len()),
                        None=>"Материал назначен всему мешу, включая несвязанные части · Ctrl+Z — отменить".into() };
                }
            }
            Err(error) => { self.editor.cancel_edit(); self.status = error.to_string(); }
        }
    }
    pub(super) fn draw_material_drag(&self, u: &mut Ui) {
        let Some(Drag::Material(d)) = &self.drag else {
            return;
        };
        if (self.mouse[0] - d.start[0]).hypot(self.mouse[1] - d.start[1]) < 6. {
            return;
        }
        let name = match d.action {
            Action::MaterialAssign(id,..) | Action::MaterialSelect(id) | Action::AssetMaterial(id) => self.editor.project.material(id).map(|m|m.name.clone()),
            Action::AssetPreset(i) => poseforge_core::MaterialPreset::ALL.get(i).map(|p|p.label().to_string()),
            _ => None,
        };
        if let Some(name) = name {
            let x = (self.mouse[0]+16.).min(self.width-340.); let y = (self.mouse[1]+16.).min(self.height-80.); let p=u.palette();
            u.surface([x,y,324.,54.],p.elevated);
            u.fitted_label([x+8.,y+8.,308.,18.],&name,12.,p.text);
            u.fitted_label([x+8.,y+30.,308.,16.],"Грани / связная часть · пустота — весь меш",10.,p.muted);
        }
    }
}

impl App {
    pub(super) fn start_node_image_load(&mut self, path: PathBuf) {
        if self.job.is_some() {
            return;
        }
        let Some((material, node)) = self.node_texture_target.take() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        std::thread::spawn(move || {
            let result = poseforge_io::load_texture(&path);
            if !c.load(Ordering::Relaxed) {
                let _ = tx.send(match result {
                    Ok(t) => JobMessage::NodeImageLoaded(t, material, node),
                    Err(e) => JobMessage::Error(e.to_string()),
                });
            }
        });
        self.job = Some(Job {
            receiver: rx,
            cancel,
            label: "Загрузка текстуры".into(),
        });
    }
    pub(super) fn finish_node_image_load(
        &mut self,
        mut texture: poseforge_core::Texture,
        material: u64,
        node: u32,
    ) {
        if self
            .job
            .as_ref()
            .is_some_and(|j| j.cancel.load(Ordering::Relaxed))
        {
            return;
        }
        let valid = self
            .editor
            .project
            .material(material)
            .and_then(|m| m.graph.as_ref())
            .is_some_and(|g| {
                g.nodes
                    .iter()
                    .any(|n| n.id == node && n.kind == poseforge_core::MaterialNodeKind::Texture)
            });
        if !valid {
            self.status = "Узел изображения больше не существует".into();
            return;
        }
        texture.id = self.editor.project.next_id();
        let id = texture.id;
        let r = self.editor.transact("Загрузить изображение узла", |p| {
            p.textures.push(texture);
            p.materials
                .iter_mut()
                .find(|m| m.id == material)
                .unwrap()
                .graph
                .as_mut()
                .unwrap()
                .nodes
                .iter_mut()
                .find(|n| n.id == node)
                .unwrap()
                .texture = Some(id);
        });
        if self.result(r).is_some() {
            self.invalidate();
            self.status = "Изображение встроено в проект и назначено узлу".into();
        }
    }
}
