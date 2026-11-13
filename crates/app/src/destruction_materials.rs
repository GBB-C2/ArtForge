//! Interior slots are polygon assignments, never overlays on the exterior.
use super::App;
use poseforge_core::{Editor, Material, MaterialLayer, Project};

pub(super) fn prepare(project: &mut Project, source: u64) -> Result<u64, String> {
    let node = project.node(source).ok_or("Исходный объект удалён")?.clone();
    let mesh = node.mesh.and_then(|id| project.mesh(id)).ok_or("Нужен меш")?;
    if mesh.assigned_materials().is_empty() && node.material_layers.is_empty() {
        let id = project.next_id();
        project.materials.push(Material { id, name: format!("Внешний · {}",node.name), in_library: false, ..Default::default() });
        project.node_mut(source).unwrap().material_layers.push(MaterialLayer { material:id, visible:true });
    }
    if let Some(id) = node.destruction.and_then(|d|d.interior_material).filter(|id|project.material(*id).is_some()) { return Ok(id); }
    let id = project.next_id();
    project.materials.push(Material { id, name:format!("Внутренний · {}",node.name),
        base_color:[0.48,0.36,0.24,1.], roughness:0.82, in_library:false, ..Default::default() });
    Ok(id)
}

impl App {
    pub(super) fn detach_destruction_material(&mut self, source:u64, material:u64) -> bool {
        let Some(component)=self.editor.project.node(source).and_then(|n|n.destruction) else{return false;};
        let interior=component.interior_material==Some(material);
        if self.editor.project.node_locked(source) {return true;}
        let mut prepared=Editor::new(self.editor.project.clone());
        let owners:Vec<_>=prepared.project.nodes.iter().filter(|n|(!interior && n.id==source)
            || (n.parent==component.result && component.result.is_some()))
            .filter_map(|n|n.mesh.map(|m|(n.id,m))).collect();
        for (node,mesh) in owners {
            if prepared.project.node(node).is_some_and(|n|n.material_layers.iter().any(|l|l.material==material)) {
                if let Err(error)=prepared.remove_material_layer(node,material) {self.status=error.to_string();return true;}
            }
            if prepared.project.mesh(mesh).is_some_and(|m|m.assigned_materials().contains(&material)) {
                if let Err(error)=prepared.unassign_material_slot(mesh,&[node],material) {self.status=error.to_string();return true;}
            }
        }
        if interior {prepared.project.node_mut(source).unwrap().destruction.as_mut().unwrap().interior_material=None;}
        let result=self.editor.transact("Снять материал разрушения",|p|*p=prepared.project);
        if self.result(result).is_some() {self.surface.object_material=None;self.invalidate_geometry();}
        true
    }
}
