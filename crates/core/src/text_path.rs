//! Shared evaluated geometry for viewport, picking, exports and conversion.
use crate::{CoreError,CurveData,Mat4,Mesh,NodeId,Project,SharedBuffer,Transform,Vec3,Vertex};
use std::{cell::RefCell,collections::{BTreeMap,VecDeque}};
#[derive(PartialEq)]
struct Key {source:u64,curve:CurveData,curve_model:Mat4,model:Mat4,local:Transform,phase:f32}
thread_local! {static CACHE:RefCell<VecDeque<(Key,SharedBuffer<Vertex>)>>=const {RefCell::new(VecDeque::new())};}
impl Project {
    pub(crate) fn evaluated_text_meshes(&self,globals:&BTreeMap<NodeId,Mat4>,local:&BTreeMap<NodeId,Transform>,time:f64)->Result<BTreeMap<NodeId,Mesh>,CoreError> {
        let mut meshes=BTreeMap::new();
        for node in self.nodes.iter().filter(|n|n.text.as_ref().is_some_and(|s|s.on_curve)&&n.curve_binding.is_some_and(|b| self.node(b.curve).is_some_and(|target| target.curve.is_some()))) {
            // Physics and skin operate on converted text, not on two competing deformations.
            if node.skin.is_some() || node.physics.is_some() || node.fluid.is_some() || node.destruction.is_some() {continue;}
            let binding=node.curve_binding.unwrap();
            let curve=self.node(binding.curve).and_then(|n|n.curve.as_ref()).ok_or(CoreError::MissingNode(binding.curve))?;
            let mesh=self.mesh(node.mesh.ok_or(CoreError::MissingNode(node.id))?).ok_or(CoreError::MissingNode(node.id))?;
            if !mesh.morph_targets.is_empty() {continue;}
            let model=globals[&node.id];let transform=local[&node.id];let curve_model=globals[&binding.curve];
            let phase=binding.phase(time)?;
            let key=Key {source:mesh.vertices.revision(),curve:curve.clone(),curve_model,model,local:transform,phase};
            let cached=CACHE.with(|cache|cache.borrow().iter().find(|(k,_)|*k==key).map(|(_,v)|v.clone()));
            let vertices=if let Some(vertices)=cached {vertices} else {
                let path=crate::curve_binding::Path::for_text(curve,curve_model)?;
                let inverse=model.inverse();let matrix=transform.matrix();let normal_matrix=matrix.inverse().transpose();
                let left=mesh.vertices.iter().map(|v|v.position[0]).fold(f32::INFINITY,f32::min)*transform.scale.x;
                let start=phase*path.length()-left;
                let mut vertices=mesh.vertices.to_vec();
                for vertex in &mut vertices {
                    let q=matrix.transform_point3(Vec3::from_array(vertex.position));
                    let frame=path.text_frame(start+q.x);
                    vertex.position=inverse.transform_point3(frame.transform_point3(Vec3::new(0.,q.y,q.z))).to_array();
                    vertex.normal=model.transpose().transform_vector3(frame.transform_vector3(normal_matrix.transform_vector3(Vec3::from_array(vertex.normal)))).normalize_or_zero().to_array();
                    if !Vec3::from_array(vertex.position).is_finite() {return Err(CoreError::Invalid("Текст выходит за пределы координат кривой".into()));}
                }
                let vertices:SharedBuffer<Vertex>=vertices.into();
                CACHE.with(|cache| {
                    let mut cache=cache.borrow_mut();
                    while !cache.is_empty() && (cache.len()>=16 || cache.iter().map(|(_,v)|v.len()).sum::<usize>()+vertices.len()>524_288) {cache.pop_front();}
                    cache.push_back((key,vertices.clone()));
                });
                vertices
            };
            let mut deformed=mesh.clone();deformed.vertices=vertices;meshes.insert(node.id,deformed);
        }
        Ok(meshes)
    }
    /// Instance-local replacement keeps linked text copies independent.
    pub fn apply_text_meshes(&mut self,meshes:&BTreeMap<NodeId,Mesh>) {
        for (&id,mesh) in meshes {
            let Some(old)=self.node(id).and_then(|n|n.mesh) else {continue;};
            let shared=self.nodes.iter().any(|n|n.id!=id&&n.mesh==Some(old));
            let next=if shared {self.next_id()} else {old};
            let mut mesh=mesh.clone();mesh.id=next;
            if let Some(slot)=self.meshes.iter_mut().find(|m|m.id==next) {*slot=mesh;} else {self.meshes.push(mesh);}
            self.node_mut(id).unwrap().mesh=Some(next);
        }
    }
}
