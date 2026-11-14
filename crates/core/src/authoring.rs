//! Bounded, transactional mesh and skeleton authoring. Runtime tools own selection.
use crate::{CoreError, Editor, Mesh, Node, Skin, Transform, Vertex};
use glam::Vec3;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_EDIT_VERTICES: usize = 65_536;
pub const MAX_EDIT_TRIANGLES: usize = 131_072;

#[derive(Clone, Debug)]
pub enum MeshEdit {
    KnifeGap { cuts: Vec<crate::KnifeCut>, width: f32 },
    KnifeGapClosed { cuts: Vec<crate::KnifeCut>, width: f32 },
    Dissolve { region: crate::MeshRegion },
    Array { region: crate::MeshRegion, count: u32, offset: Vec3, rotation: Vec3 },
    ExtrudeDistributed { faces: BTreeSet<usize>, distance: f32 },
    ExtrudeManifold { faces: BTreeSet<usize>, distance: f32 },

    KnifeStroke {cuts:Vec<crate::KnifeCut>},
    Topology {region:crate::MeshRegion,edit:crate::TopologyEdit},
    BridgeAdvanced {faces:BTreeSet<usize>,edges:BTreeSet<(usize,usize)>,segments:u32,twist:i32,smooth:f32},
    ExtrudeVertices { vertices: BTreeSet<usize>, distance: f32 },
    DeleteComponents { vertices: BTreeSet<usize>, edges: BTreeSet<(usize, usize)>, faces: BTreeSet<usize> },
    SubdivideSelected { vertices: BTreeSet<usize>, edges: BTreeSet<(usize, usize)>, faces: BTreeSet<usize> },
    BridgeFaces { faces: BTreeSet<usize>, segments: u32 },
    Normals {
        faces: BTreeSet<usize>,
        edit: crate::NormalEdit,
    },
    Shape {
        vertices: BTreeSet<usize>,
        edges: BTreeSet<(usize, usize)>,
        edit: crate::ShapeEdit,
    },
    Knife {
        face: usize,
        first: crate::EdgePoint,
        second: crate::EdgePoint,
    },
    Slide {
        vertices: BTreeSet<usize>,
        edges: BTreeSet<(usize, usize)>,
        amount: f32,
    },
    WeightedTransform {
        weights: Vec<f32>,
        matrix: crate::Mat4,
    },
    Uv(crate::UvEdit),
    VertexBevel {
        vertices: BTreeSet<usize>,
        amount: f32,
    },
    EdgeBevel {
        edges: BTreeSet<(usize, usize)>,
        width: f32,
        segments: u32,
        profile: f32,
    },
    MergeVertices {
        vertices: BTreeSet<usize>,
    },
    WeldVertices {
        vertices: BTreeSet<usize>,
        distance: f32,
    },
    Bridge {
        edges: BTreeSet<(usize, usize)>,
        segments: u32,
    },
    DissolveEdges {
        edges: BTreeSet<(usize, usize)>,
    },

    Transform {
        vertices: BTreeSet<usize>,
        matrix: crate::Mat4,
    },
    Bevel {
        faces: BTreeSet<usize>,
        inset: f32,
        shift: f32,
    },
    LoopCut {
        edge: (usize, usize),
        position: f32,
    },
    Translate {
        vertices: BTreeSet<usize>,
        delta: Vec3,
    },
    Extrude {
        faces: BTreeSet<usize>,
        distance: f32,
    },
    DeleteFaces(BTreeSet<usize>),
    Subdivide,
    SmoothNormals,
    FlatNormals,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkeletonTemplate {
    Bone,
    Chain,
    Humanoid,
}

fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}
pub fn position_key(p: [f32; 3]) -> [i64; 3] {
    p.map(|v| (f64::from(v) * 1_000_000.).round() as i64)
}
pub fn recompute_normals(mesh: &mut Mesh, smooth_seams: bool) {
    let mut normals = vec![Vec3::ZERO; mesh.vertices.len()];
    for t in mesh.indices.as_chunks::<3>().0.iter() {
        let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
        let p = |i: usize| Vec3::from_array(mesh.vertices[i].position);
        let n = (p(b) - p(a)).cross(p(c) - p(a));
        for i in [a, b, c] {
            normals[i] += n;
        }
    }
    if smooth_seams {
        let mut sums = BTreeMap::<[i64; 3], Vec3>::new();
        for (v, n) in mesh.vertices.iter().zip(&normals) {
            *sums.entry(position_key(v.position)).or_default() += *n;
        }
        for (v, n) in mesh.vertices.iter().zip(&mut normals) {
            *n = sums[&position_key(v.position)];
        }
    }
    for (v, n) in mesh.vertices.iter_mut().zip(normals) {
        v.normal = n.try_normalize().unwrap_or(Vec3::Y).to_array();
    }
}
fn compact(mesh: &mut Mesh) {
    let mut remap = BTreeMap::new();
    let mut vertices = Vec::new();
    for i in &mut mesh.indices {
        *i = *remap.entry(*i).or_insert_with(|| {
            let next = vertices.len() as u32;
            vertices.push(mesh.vertices[*i as usize]);
            next
        });
    }
    mesh.vertices = vertices.into();
}
fn faces_valid(mesh: &Mesh, faces: &BTreeSet<usize>) -> Result<(), CoreError> {
    if faces.is_empty() || faces.iter().any(|&i| i >= mesh.indices.len() / 3) {
        return Err(invalid("Выберите грани меша"));
    }
    Ok(())
}
pub(crate) fn apply_mesh_edit(mesh: &mut Mesh, operation: MeshEdit) -> Result<(), CoreError> {
    let preserve_wires = !matches!(operation,
        MeshEdit::ExtrudeVertices { .. } | MeshEdit::DeleteComponents { .. } | MeshEdit::SubdivideSelected { .. } | MeshEdit::Topology {..}
        | MeshEdit::Transform { .. } | MeshEdit::Translate { .. } | MeshEdit::Slide { .. }
        | MeshEdit::WeightedTransform { .. } | MeshEdit::Shape { .. } | MeshEdit::Knife { .. } | MeshEdit::KnifeStroke {..} | MeshEdit::KnifeGap {..} | MeshEdit::KnifeGapClosed {..} | MeshEdit::Array {..} | MeshEdit::Uv(_));
    let wires: Vec<_> = if preserve_wires { mesh.topology.iter().flat_map(|t|t.loose_edges.iter())
        .map(|e|(*e,e.map(|v|mesh.vertices[v as usize]))).collect() } else {Vec::new()};
    if !wires.is_empty() && !matches!(operation, MeshEdit::BridgeAdvanced {..} | MeshEdit::Bridge {..}) {mesh.topology.as_mut().unwrap().loose_edges.clear();}
    let remap = !matches!(
        operation,
        MeshEdit::Uv(_)
            | MeshEdit::VertexBevel { .. }
            | MeshEdit::Shape { .. }
            | MeshEdit::Knife { .. } | MeshEdit::KnifeStroke {..} | MeshEdit::KnifeGap {..} | MeshEdit::KnifeGapClosed {..} | MeshEdit::Array {..}
            | MeshEdit::Transform { .. }
            | MeshEdit::Translate { .. }
            | MeshEdit::Slide { .. }
            | MeshEdit::WeightedTransform { .. }
    );
    let seams: Vec<_> = if remap {
        crate::uv_seams(mesh)
            .into_iter()
            .map(|(a, b)| {
                (
                    Vec3::from_array(mesh.vertices[a].position),
                    Vec3::from_array(mesh.vertices[b].position),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    if remap {
        if let Some(t) = &mut mesh.topology {
            t.uv_seams.clear();
        }
    }
    apply_mesh_edit_inner(mesh, operation)?;
    if !wires.is_empty() {
        crate::component_mesh::ensure_topology(mesh);
        let graph=crate::MeshConnectivity::new(mesh);
        let mut points:BTreeMap<_,_>=mesh.vertices.iter().enumerate().map(|(i,v)|(position_key(v.position),i as u32)).collect();
        let mut restored=Vec::new();
        for (old,pair) in wires {
            let ids=std::array::from_fn(|i| {
                let v=pair[i];
                if mesh.vertices.get(old[i] as usize).is_some_and(|current|current.position==v.position) {old[i]}
                else {*points.entry(position_key(v.position)).or_insert_with(||{let id=mesh.vertices.len() as u32;mesh.vertices.push(v);id})}
            });
            let canonical=ids.map(|v|graph.canonical.get(v as usize).copied().unwrap_or(v as usize));
            if !graph.edges.contains(&(canonical[0].min(canonical[1]),canonical[0].max(canonical[1]))) {restored.push(ids);}
        }
        mesh.topology.as_mut().unwrap().loose_edges.extend(restored);
        if mesh.vertices.len()>MAX_EDIT_VERTICES {return Err(invalid("Операция превышает предел вершин"));}
    }
    if !seams.is_empty() {
        let g = crate::MeshConnectivity::new(mesh);
        let points: BTreeMap<_, _> = mesh
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| (position_key(v.position), g.canonical[i]))
            .collect();
        let mut marked = BTreeSet::new();
        for (a, b) in seams {
            let Some(&start) = points.get(&position_key(a.to_array())) else {
                continue;
            };
            let line = b - a;
            let length = line.length_squared();
            if length < 1e-16 {
                continue;
            }
            let mut seen = BTreeSet::from([start]);
            let mut pending = vec![start];
            while let Some(v) = pending.pop() {
                for &n in &g.vertex_neighbors[v] {
                    let p = Vec3::from_array(mesh.vertices[n].position);
                    let t = (p - a).dot(line) / length;
                    if !(-1e-5..=1.00001).contains(&t)
                        || (p - a - line * t).length_squared() > 1e-10 * length.max(1.)
                    {
                        continue;
                    }
                    marked.insert([v.min(n) as u32, v.max(n) as u32]);
                    if seen.insert(n) {
                        pending.push(n);
                    }
                }
            }
        }
        if let Some(t) = &mut mesh.topology {
            t.uv_seams = marked.into_iter().collect();
        }
    }
    Ok(())
}
fn apply_mesh_edit_inner(mesh: &mut Mesh, operation: MeshEdit) -> Result<(), CoreError> {
    match &operation {
        MeshEdit::KnifeGapClosed {cuts,width}=>return crate::knife_stroke::apply_gap_closed(mesh,cuts,*width),
        MeshEdit::KnifeGap {cuts,width}=>return crate::knife_stroke::apply_gap(mesh,cuts,*width),
        MeshEdit::Dissolve {region}=>return crate::modeling_workflow::dissolve(mesh,region),
        MeshEdit::Array {region,count,offset,rotation}=>return crate::modeling_workflow::array(mesh,region,*count,*offset,*rotation),
        MeshEdit::ExtrudeManifold {faces,distance}=>return crate::extrude_manifold::apply(mesh,faces,*distance),
        MeshEdit::ExtrudeDistributed {faces,distance}=>return crate::modeling_workflow::extrude(mesh,faces,*distance),
        MeshEdit::KnifeStroke {cuts}=>return crate::knife_stroke::apply(mesh,cuts),
        MeshEdit::Topology {region,edit}=>return crate::mesh_cleanup::apply(mesh,region,*edit),
        MeshEdit::BridgeAdvanced {faces,edges,segments,twist,smooth}=>return if faces.is_empty() {crate::mesh_topology_tools::bridge_options(mesh,edges,*segments,*twist,*smooth)} else {crate::mesh_topology_tools::bridge_faces_options(mesh,faces,*segments,*twist,*smooth)},
        MeshEdit::ExtrudeVertices { vertices, distance } => return crate::component_mesh::extrude_vertices(mesh, vertices, *distance),
        MeshEdit::DeleteComponents { vertices, edges, faces } => return crate::component_mesh::delete(mesh, vertices, edges, faces),
        MeshEdit::SubdivideSelected { vertices, edges, faces } => return crate::component_mesh::subdivide(mesh, vertices, edges, faces),
        MeshEdit::BridgeFaces { faces, segments } => return crate::mesh_topology_tools::bridge_faces(mesh, faces, *segments),
        MeshEdit::Normals { faces, edit } => return crate::mesh_normals::apply(mesh, faces, edit),
        MeshEdit::Shape {
            vertices,
            edges,
            edit,
        } => return crate::mesh_shape::apply(mesh, vertices, edges, *edit),
        MeshEdit::Knife {
            face,
            first,
            second,
        } => return crate::mesh_advanced::knife(mesh, *face, *first, *second),
        MeshEdit::Slide {
            vertices,
            edges,
            amount,
        } => return crate::mesh_advanced::slide(mesh, vertices, edges, *amount),
        MeshEdit::WeightedTransform { weights, matrix } => {
            return crate::mesh_advanced::weighted_transform(mesh, weights, *matrix)
        }
        MeshEdit::Uv(edit) => {
            return crate::edit_uv(
                mesh,
                edit.clone(),
                &std::sync::atomic::AtomicBool::new(false),
            )
        }
        MeshEdit::VertexBevel { vertices, amount } => {
            return crate::vertex_bevel::apply(mesh, vertices, *amount)
        }
        MeshEdit::EdgeBevel {
            edges,
            width,
            segments,
            profile,
        } => return crate::edge_bevel::apply(mesh, edges, *width, *segments, *profile),
        MeshEdit::MergeVertices { vertices } => {
            return crate::mesh_topology_tools::merge(mesh, vertices, None)
        }
        MeshEdit::WeldVertices { vertices, distance } => {
            return crate::mesh_topology_tools::merge(mesh, vertices, Some(*distance))
        }
        MeshEdit::Bridge { edges, segments } => {
            return crate::mesh_topology_tools::bridge(mesh, edges, *segments)
        }
        MeshEdit::DissolveEdges { edges } => {
            return crate::mesh_topology_tools::dissolve(mesh, edges)
        }

        MeshEdit::Transform { vertices, matrix } => {
            return crate::mesh_modeling::transform(mesh, vertices, *matrix)
        }
        MeshEdit::Bevel {
            faces,
            inset,
            shift,
        } => return crate::mesh_modeling::bevel(mesh, faces, *inset, *shift),
        MeshEdit::LoopCut { edge, position } => {
            return crate::mesh_modeling::loop_cut(mesh, *edge, *position)
        }
        _ => {}
    }
    if mesh
        .topology
        .as_ref()
        .is_some_and(|t| t.faces.iter().any(|f| f.len() != 3))
    {
        return crate::polygon_edit::apply(mesh, operation);
    }
    // A legacy triangle mesh can retain a shading policy without changing its
    // subdivision or extrusion semantics into polygon operations.
    let mut smooth = mesh.smooth_normals_enabled();
    let keep_policy = mesh.topology.is_some() || matches!(operation, MeshEdit::SmoothNormals);
    match operation {
        MeshEdit::ExtrudeVertices { .. } | MeshEdit::DeleteComponents { .. }
        | MeshEdit::SubdivideSelected { .. } | MeshEdit::BridgeFaces { .. } | MeshEdit::BridgeAdvanced {..} | MeshEdit::Topology {..} | MeshEdit::KnifeStroke {..} | MeshEdit::KnifeGap {..} | MeshEdit::KnifeGapClosed {..} | MeshEdit::Dissolve {..} | MeshEdit::Array {..} | MeshEdit::ExtrudeDistributed {..} | MeshEdit::ExtrudeManifold {..} => unreachable!(),
        MeshEdit::Normals { .. }
        | MeshEdit::Shape { .. }
        | MeshEdit::Knife { .. }
        | MeshEdit::Slide { .. }
        | MeshEdit::WeightedTransform { .. }
        | MeshEdit::Uv(_)
        | MeshEdit::VertexBevel { .. }
        | MeshEdit::EdgeBevel { .. }
        | MeshEdit::MergeVertices { .. }
        | MeshEdit::WeldVertices { .. }
        | MeshEdit::Bridge { .. }
        | MeshEdit::DissolveEdges { .. }
        | MeshEdit::Transform { .. }
        | MeshEdit::Bevel { .. }
        | MeshEdit::LoopCut { .. } => {
            unreachable!()
        }
        MeshEdit::Translate { vertices, delta } => {
            if vertices.is_empty()
                || vertices.iter().any(|&i| i >= mesh.vertices.len())
                || !delta.is_finite()
                || delta.abs().max_element() > 10000.
            {
                return Err(invalid("Выберите вершины и задайте конечное смещение"));
            }
            if delta == Vec3::ZERO {
                return Ok(());
            }
            let keys: BTreeSet<_> = vertices
                .iter()
                .map(|&i| position_key(mesh.vertices[i].position))
                .collect();
            let all = mesh
                .vertices
                .iter()
                .all(|v| keys.contains(&position_key(v.position)));
            for v in &mut mesh.vertices {
                if keys.contains(&position_key(v.position)) {
                    v.position = (Vec3::from_array(v.position) + delta).to_array();
                }
            }
            if !all {
                recompute_normals(mesh, smooth);
            }
        }
        MeshEdit::Extrude { faces, distance } => {
            faces_valid(mesh, &faces)?;
            if distance==0. {return Ok(());}
            if !distance.is_finite() || distance.abs() < 1e-6 || distance.abs() > 1000. {
                return Err(invalid(
                    "Расстояние экструзии: ненулевое число от −1000 до 1000",
                ));
            }
            let mut normal = Vec3::ZERO;
            let mut used = BTreeSet::new();
            let mut edges = BTreeMap::<(u32, u32), Vec<(u32, u32)>>::new();
            // Position keys join UV seams when finding the boundary of a selected region.
            let mut weld = BTreeMap::new();
            let canonical: Vec<u32> = mesh
                .vertices
                .iter()
                .enumerate()
                .map(|(i, v)| *weld.entry(position_key(v.position)).or_insert(i as u32))
                .collect();
            for &face in &faces {
                let t = &mesh.indices[face * 3..face * 3 + 3];
                let p = |i: u32| Vec3::from_array(mesh.vertices[i as usize].position);
                normal += (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
                for j in 0..3 {
                    let a = t[j];
                    let b = t[(j + 1) % 3];
                    used.insert(a);
                    let ca = canonical[a as usize];
                    let cb = canonical[b as usize];
                    edges
                        .entry((ca.min(cb), ca.max(cb)))
                        .or_default()
                        .push((a, b));
                }
            }
            let delta = normal.try_normalize().ok_or_else(|| {
                invalid("У выбранной области нет общего направления; выберите одну сторону")
            })? * distance;
            let mut cap = BTreeMap::new();
            for i in used {
                let mut v = mesh.vertices[i as usize];
                v.position = (Vec3::from_array(v.position) + delta).to_array();
                cap.insert(i, mesh.vertices.len() as u32);
                mesh.vertices.push(v);
            }
            for &face in &faces {
                for i in &mut mesh.indices[face * 3..face * 3 + 3] {
                    *i = cap[i];
                }
            }
            for boundary in edges.values() {
                if let [(a, b)] = boundary.as_slice() {
                    mesh.indices.extend([*a, *b, cap[b], *a, cap[b], cap[a]]);
                }
            }
            compact(mesh);
            recompute_normals(mesh, smooth);
        }
        MeshEdit::DeleteFaces(faces) => {
            faces_valid(mesh, &faces)?;
            if faces.len() == mesh.indices.len() / 3 {
                return Err(invalid(
                    "Оставьте хотя бы одну грань; весь объект удаляется в режиме сцены",
                ));
            }
            mesh.indices = mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .enumerate()
                .filter(|(i, _)| !faces.contains(i))
                .flat_map(|(_, t)| t.iter().copied())
                .collect();
            compact(mesh);
            recompute_normals(mesh, smooth);
        }
        MeshEdit::Subdivide => {
            if mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES / 4 {
                return Err(invalid("Слишком много граней для подразделения"));
            }
            let mut midpoint = BTreeMap::new();
            let mut out = Vec::with_capacity(mesh.indices.len() * 4);
            for tri in mesh.indices.as_chunks::<3>().0.iter() {
                let [a, b, c] = [tri[0], tri[1], tri[2]];
                let mut mid = |a: u32, b: u32| {
                    *midpoint.entry((a.min(b), a.max(b))).or_insert_with(|| {
                        let va = mesh.vertices[a as usize];
                        let vb = mesh.vertices[b as usize];
                        let id = mesh.vertices.len() as u32;
                        mesh.vertices.push(Vertex {
                            position: ((Vec3::from_array(va.position)
                                + Vec3::from_array(vb.position))
                                * 0.5)
                                .to_array(),
                            uv: [(va.uv[0] + vb.uv[0]) * 0.5, (va.uv[1] + vb.uv[1]) * 0.5],
                            color: std::array::from_fn(|i| (va.color[i] + vb.color[i]) * 0.5),
                            ..Vertex::default()
                        });
                        id
                    })
                };
                let ab = mid(a, b);
                let bc = mid(b, c);
                let ca = mid(c, a);
                out.extend([a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
            }
            mesh.indices = out.into();
            recompute_normals(mesh, smooth);
        }
        MeshEdit::SmoothNormals => {
            smooth = true;
            recompute_normals(mesh, true);
        }
        MeshEdit::FlatNormals => {
            smooth = false;
            if mesh.indices.len() > MAX_EDIT_VERTICES {
                return Err(invalid("Слишком много вершин для плоских нормалей"));
            }
            mesh.vertices = mesh
                .indices
                .iter()
                .map(|&i| mesh.vertices[i as usize])
                .collect();
            mesh.indices = (0..mesh.vertices.len() as u32).collect();
            recompute_normals(mesh, false);
        }
    }
    if keep_policy {
        mesh.topology = Some(crate::MeshTopology { loose_edges: Vec::new(),
            face_materials: Default::default(),
            uv_seams: Vec::new(),
            faces: mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|f| f.to_vec())
                .collect(),
            primitive: None,
            smooth_normals: smooth,
        });
    }
    if mesh.vertices.len() > MAX_EDIT_VERTICES || mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES {
        return Err(invalid(
            "Предел редактирования: 65536 вершин и 131072 треугольника",
        ));
    }
    Ok(())
}

impl Editor {
    pub fn authoring_unlocked(&self, id: u64) -> Result<(), CoreError> {
        let mut current = Some(id);
        let mut depth = 0;
        while let Some(id) = current {
            let n = self.project.node(id).ok_or(CoreError::MissingNode(id))?;
            if n.locked {
                return Err(invalid("Объект или его родитель заблокирован"));
            }
            current = n.parent;
            depth += 1;
            if depth > self.project.nodes.len() {
                return Err(CoreError::HierarchyCycle(id));
            }
        }
        if self.has_staged() {
            return Err(CoreError::UncommittedPose);
        }
        Ok(())
    }
    pub fn editable_mesh(&self, id: u64) -> Result<&Mesh, CoreError> {
        self.authoring_unlocked(id)?;
        let n = self.project.node(id).ok_or(CoreError::MissingNode(id))?;
        if n.text.is_some() {
            return Err(invalid(
                "Измените надпись в объекте или преобразуйте текст в меш",
            ));
        }
        if n.curve.is_some() {
            return Err(invalid(
                "Редактируйте точки кривой в объекте или преобразуйте её в меш",
            ));
        }
        let mesh = n
            .mesh
            .and_then(|id| self.project.mesh(id))
            .ok_or_else(|| invalid("Выберите объект с мешем"))?;
        if n.skin.is_some() || !mesh.morph_targets.is_empty() {
            return Err(invalid("Редактируйте геометрию до привязки скелета; skin и shape keys не поддержаны режимом сетки"));
        }
        if mesh.vertices.len() > MAX_EDIT_VERTICES || mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES {
            return Err(invalid(
                "Предел редактирования: 65536 вершин и 131072 треугольника",
            ));
        }
        Ok(mesh)
    }
    pub fn edit_mesh(&mut self, id: u64, operation: MeshEdit) -> Result<(), CoreError> {
        let original = self.editable_mesh(id)?.clone();
        let mut mesh = original.clone();
        apply_mesh_edit(&mut mesh, operation)?;
        if mesh == original {
            return Ok(());
        }
        if self
            .project
            .nodes
            .iter()
            .filter(|n| n.mesh == Some(original.id))
            .count()
            > 1
        {
            mesh.id = self.project.next_id();
            mesh.name = format!("{} · правка", mesh.name);
        }
        self.transact("Редактировать меш", move |p| {
            p.node_mut(id).unwrap().mesh = Some(mesh.id);
            if let Some(slot) = p.meshes.iter_mut().find(|m| m.id == mesh.id) {
                *slot = mesh;
            } else {
                p.meshes.push(mesh);
            }
        })
    }
    /// Update one mesh from a frozen gesture snapshot. Avoid cloning and
    /// validating every unrelated scene asset for each pointer event; commit_edit
    /// performs the complete document validation and cancel_edit restores it.
    pub fn preview_mesh(
        &mut self,
        id: u64,
        original: &Mesh,
        operation: MeshEdit,
    ) -> Result<(), CoreError> {
        if !self.has_active_edit() {
            return Err(invalid("Сначала начните действие редактирования"));
        }
        let current = self.editable_mesh(id)?.id;
        let mut mesh = original.clone();
        apply_mesh_edit(&mut mesh, operation)?;
        mesh.validate_topology()?;
        if mesh.vertices.len() > MAX_EDIT_VERTICES
            || mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES
            || mesh.vertices.iter().any(|v| {
                v.position
                    .iter()
                    .chain(&v.normal)
                    .chain(&v.uv)
                    .any(|x| !x.is_finite())
            })
        {
            return Err(invalid("Некорректная геометрия предпросмотра"));
        }
        if mesh == *original {
            self.project.node_mut(id).unwrap().mesh = Some(original.id);
            if current != original.id && !self.project.nodes.iter().any(|n| n.mesh == Some(current))
            {
                self.project.meshes.retain(|m| m.id != current);
            }
            if let Some(slot) = self.project.meshes.iter_mut().find(|m| m.id == original.id) {
                *slot = original.clone();
            }
            return Ok(());
        }
        mesh.id = if current != original.id {
            current
        } else if self
            .project
            .nodes
            .iter()
            .any(|n| n.id != id && n.mesh == Some(current))
        {
            self.project.next_id()
        } else {
            current
        };
        self.project.node_mut(id).unwrap().mesh = Some(mesh.id);
        if let Some(slot) = self.project.meshes.iter_mut().find(|m| m.id == mesh.id) {
            *slot = mesh;
        } else {
            self.project.meshes.push(mesh);
        }
        Ok(())
    }
    pub fn create_skeleton(
        &mut self,
        template: SkeletonTemplate,
        target: Option<u64>,
    ) -> Result<u64, CoreError> {
        if self.has_staged() {
            return Err(CoreError::UncommittedPose);
        }
        let mut min = Vec3::new(-0.7, 0., -0.3);
        let mut max = Vec3::new(0.7, 2., 0.3);
        if let Some(id) = target {
            self.authoring_unlocked(id)?;
            if let Some(mesh) = self
                .project
                .node(id)
                .and_then(|n| n.mesh)
                .and_then(|m| self.project.mesh(m))
            {
                let model = self.project.global_transforms()?[&id];
                min = Vec3::splat(f32::INFINITY);
                max = Vec3::splat(f32::NEG_INFINITY);
                for v in &mesh.vertices {
                    let p = model.transform_point3(Vec3::from_array(v.position));
                    min = min.min(p);
                    max = max.max(p);
                }
            }
        }
        let height = (max.y - min.y).max(0.5);
        let center = (min + max) * 0.5;
        let base = Vec3::new(center.x, min.y, center.z);
        let root = self.project.next_id();
        if template == SkeletonTemplate::Humanoid {
            let (nodes, profile) = crate::humanoid_template::create(root, base, height);
            self.transact("Создать скелет человека", move |p| {
                p.nodes.extend(nodes);
                p.rig_profiles.push(profile);
            })?;
            return Ok(root);
        }
        let mut nodes = vec![Node::new(root, "Скелет")];
        let data: Vec<(Option<usize>, Vec3)> = match template {
            SkeletonTemplate::Bone => vec![(None, Vec3::ZERO)],
            SkeletonTemplate::Chain => vec![
                (None, Vec3::ZERO),
                (Some(0), Vec3::new(0., 0.5, 0.)),
                (Some(1), Vec3::Y),
            ],
            SkeletonTemplate::Humanoid => unreachable!(),
        };
        for (i, (parent, point)) in data.iter().enumerate() {
            let id = root + 1 + i as u64;
            let mut node = Node::new(id, format!("Кость.{:03}", i + 1));
            node.parent = Some(parent.map_or(root, |p| root + 1 + p as u64));
            node.is_joint = true;
            node.transform = Transform::from_translation(if let Some(parent) = parent {
                (*point - data[*parent].1) * height
            } else {
                base + *point * height
            });
            node.rest_transform = Some(node.transform);
            nodes.push(node);
        }
        self.transact("Создать скелет", move |p| {
            p.nodes.extend(nodes)
        })?;
        Ok(root)
    }
    pub fn extend_joint(&mut self, parent: u64, offset: Vec3) -> Result<u64, CoreError> {
        self.authoring_unlocked(parent)?;
        let p = self
            .project
            .node(parent)
            .ok_or(CoreError::MissingNode(parent))?;
        if !p.is_joint || !offset.is_finite() || offset.length() < 1e-5 || offset.length() > 1000. {
            return Err(invalid("Выберите кость и задайте длину от 0,001 до 1000"));
        }
        let id = self.project.next_id();
        let mut n = Node::new(id, format!("Кость.{id}"));
        n.parent = Some(parent);
        n.is_joint = true;
        n.transform = Transform::from_translation(offset);
        n.rest_transform = Some(n.transform);
        n.bone_display = p.bone_display;
        self.transact("Продолжить кость", move |p| {
            p.node_mut(parent).unwrap().bone_tip = None;
            p.nodes.push(n);
        })?;
        Ok(id)
    }
    pub fn bind_mesh(&mut self, node: u64, root: u64) -> Result<(), CoreError> {
        self.bind_mesh_cancellable(node, root, || false)
    }
    pub fn bind_mesh_cancellable(
        &mut self,
        node: u64,
        root: u64,
        cancelled: impl Fn() -> bool,
    ) -> Result<(), CoreError> {
        self.authoring_unlocked(node)?;
        self.authoring_unlocked(root)?;
        let n = self
            .project
            .node(node)
            .ok_or(CoreError::MissingNode(node))?;
        if n.skin.is_some() {
            return Err(invalid("Меш уже привязан; сначала снимите привязку"));
        }
        if n.physics
            .is_some_and(|s| s.kind != crate::PhysicsKind::Collider)
        {
            return Err(invalid("Сначала отключите деформируемую физику меша"));
        }
        let original = n
            .mesh
            .and_then(|id| self.project.mesh(id))
            .ok_or_else(|| invalid("Выберите меш для привязки"))?;
        let joints: Vec<u64> = self
            .project
            .nodes
            .iter()
            .filter(|n| {
                n.is_joint
                    && crate::humanoid::is_descendant(&self.project, n.id, root).unwrap_or(false)
            })
            .map(|n| n.id)
            .collect();
        if joints.is_empty() || joints.len() > crate::MAX_SKIN_JOINTS {
            return Err(invalid("Скелет должен содержать от 1 до 65 536 костей"));
        }
        for &id in &joints {
            self.authoring_unlocked(id)?;
        }
        let globals = self.project.global_transforms()?;
        let model = globals[&node];
        let skin = Skin {
            inverse_bind_matrices: joints
                .iter()
                .map(|id| globals[id].inverse() * model)
                .collect(),
            joints: joints.clone(),
        };
        let mut child_heads: BTreeMap<u64, Vec<Vec3>> = BTreeMap::new();
        for joint in self.project.nodes.iter().filter(|n| n.is_joint) {
            if let Some(parent) = joint.parent {
                child_heads
                    .entry(parent)
                    .or_default()
                    .push(globals[&joint.id].w_axis.truncate());
            }
        }
        let explicit_tips: BTreeMap<_, _> = self
            .project
            .nodes
            .iter()
            .filter_map(|n| {
                n.bone_tip
                    .map(|tip| (n.id, globals[&n.id].transform_point3(tip)))
            })
            .collect();
        let segments: Vec<_> = joints
            .iter()
            .map(|id| {
                let head = globals[id].w_axis.truncate();
                let tails = if let Some(tip) = explicit_tips.get(id) {
                    vec![*tip]
                } else {
                    child_heads.remove(id).unwrap_or_default()
                };
                (head, tails)
            })
            .collect();
        let mut mesh = original.clone();
        if self
            .project
            .nodes
            .iter()
            .filter(|n| n.mesh == Some(original.id))
            .count()
            > 1
        {
            mesh.id = self.project.next_id();
        }
        for (index, v) in mesh.vertices.iter_mut().enumerate() {
            if index % 32 == 0 && cancelled() {
                return Err(invalid("Привязка отменена"));
            }
            let point = model.transform_point3(Vec3::from_array(v.position));
            let mut nearest: Vec<_> = segments
                .iter()
                .enumerate()
                .map(|(i, (head, tails))| {
                    let distance = tails
                        .iter()
                        .map(|tail| {
                            let d = *tail - *head;
                            let t = ((point - *head).dot(d) / d.length_squared().max(1e-12))
                                .clamp(0., 1.);
                            point.distance_squared(*head + d * t)
                        })
                        .reduce(f32::min)
                        .unwrap_or_else(|| point.distance_squared(*head));
                    (i, distance)
                })
                .collect();
            let order =
                |a: &(usize, f32), b: &(usize, f32)| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0));
            if nearest.len() > 4 {
                nearest.select_nth_unstable_by(4, order);
            }
            nearest.truncate(4);
            nearest.sort_by(order);
            let floor = nearest[0].1.max(1e-8);
            let weights: Vec<_> = nearest
                .iter()
                .map(|(_, d)| (floor / d.max(1e-8)).powi(2))
                .collect();
            let sum: f32 = weights.iter().sum();
            v.joints = [0; 4];
            v.weights = [0.; 4];
            for (slot, ((joint, _), weight)) in nearest.iter().zip(weights).enumerate() {
                v.joints[slot] = *joint as u32;
                v.weights[slot] = weight / sum;
            }
        }
        self.transact("Привязать меш к скелету", move |p| {
            let n = p.node_mut(node).unwrap();
            n.mesh = Some(mesh.id);
            n.skin = Some(skin);
            if let Some(m) = p.meshes.iter_mut().find(|m| m.id == mesh.id) {
                *m = mesh;
            } else {
                p.meshes.push(mesh);
            }
        })
    }
    pub fn unbind_mesh(&mut self, node: u64) -> Result<(), CoreError> {
        self.authoring_unlocked(node)?;
        let n = self
            .project
            .node(node)
            .ok_or(CoreError::MissingNode(node))?;
        if n.skin.is_none() {
            return Ok(());
        }
        let mut mesh = n
            .mesh
            .and_then(|id| self.project.mesh(id))
            .ok_or_else(|| invalid("Выберите меш"))?
            .clone();
        if self
            .project
            .nodes
            .iter()
            .filter(|n| n.mesh == Some(mesh.id))
            .count()
            > 1
        {
            mesh.id = self.project.next_id();
        }
        for v in &mut mesh.vertices {
            v.joints = [0; 4];
            v.weights = [0.; 4];
        }
        self.transact(
            "Снять привязку — исходная форма",
            move |p| {
                let n = p.node_mut(node).unwrap();
                n.skin = None;
                n.mesh = Some(mesh.id);
                if let Some(m) = p.meshes.iter_mut().find(|m| m.id == mesh.id) {
                    *m = mesh;
                } else {
                    p.meshes.push(mesh);
                }
            },
        )
    }
}
