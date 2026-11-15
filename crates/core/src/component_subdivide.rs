//! Uniform selected-edge subdivision with shared boundary samples and UV corners.
use super::*;
pub(crate) fn apply(
    mesh: &mut Mesh,
    region: &crate::MeshRegion,
    segments: u32,
) -> Result<(), CoreError> {
    if !(2..=32).contains(&segments) {
        return Err(invalid("Количество делений: 2–32"));
    }
    if segments == 2 {
        return subdivide(mesh, &region.vertices, &region.edges, &region.faces);
    }
    let n = segments as usize;
    let (graph, selected, mut cuts) =
        selection(mesh, &region.vertices, &region.edges, &region.faces)?;
    let source = crate::mesh_topology_tools::faces(mesh);
    let mut selected_faces = region.faces.clone();
    if !selected.is_empty() {
        cuts.extend(
            graph
                .edges
                .iter()
                .copied()
                .filter(|(a, b)| selected.contains(a) && selected.contains(b)),
        );
        selected_faces.extend(source.iter().enumerate().filter_map(|(f, ring)| {
            ring.iter()
                .all(|&v| selected.contains(&graph.canonical[v as usize]))
                .then_some(f)
        }));
    }
    for &f in &selected_faces {
        for i in 0..source[f].len() {
            let ring = &source[f];
            cuts.insert(edge(
                graph.canonical[ring[i] as usize],
                graph.canonical[ring[(i + 1) % ring.len()] as usize],
            ));
        }
    }
    if cuts.is_empty() {
        return Err(invalid(
            "Выберите грани, рёбра или соседние точки для подразделения",
        ));
    }
    let triangles = selected_faces
        .iter()
        .map(|&f| source[f].len().saturating_sub(2) * n * n)
        .sum::<usize>();
    if triangles + mesh.indices.len() / 3 > MAX_EDIT_TRIANGLES {
        return Err(invalid(
            "Подразделение превышает предел сетки; уменьшите количество делений",
        ));
    }
    ensure_topology(mesh);
    let materials: Vec<_> = (0..source.len()).map(|f| mesh.face_material(f)).collect();
    let mut cache = BTreeMap::<(u32, u32), Vec<u32>>::new();
    let mut chain = |mesh: &mut Mesh, a: u32, b: u32| -> Result<Vec<u32>, CoreError> {
        let key = (a.min(b), a.max(b));
        if !cache.contains_key(&key) {
            if mesh.vertices.len() + n - 1 > MAX_EDIT_VERTICES {
                return Err(invalid("Подразделение превышает предел вершин"));
            }
            let (va, vb) = (mesh.vertices[key.0 as usize], mesh.vertices[key.1 as usize]);
            let (ca, cb) = (
                graph.canonical[key.0 as usize],
                graph.canonical[key.1 as usize],
            );
            let mut ids = vec![key.0];
            for i in 1..n {
                let t = i as f32 / n as f32;
                let mut v = interpolate(va, vb, t);
                v.position = if ca < cb {
                    interpolate(mesh.vertices[ca], mesh.vertices[cb], t).position
                } else {
                    interpolate(
                        mesh.vertices[cb],
                        mesh.vertices[ca],
                        (n - i) as f32 / n as f32,
                    )
                    .position
                };
                ids.push(mesh.vertices.len() as u32);
                mesh.vertices.push(v);
            }
            ids.push(key.1);
            cache.insert(key, ids);
        }
        let mut result = cache[&key].clone();
        if a > b {
            result.reverse();
        }
        Ok(result)
    };
    let mut output = Vec::new();
    let mut attributes = Vec::new();
    for (f, ring) in source.iter().enumerate() {
        let start = output.len();
        if !selected_faces.contains(&f) {
            let mut face = Vec::new();
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                face.push(a);
                if cuts.contains(&edge(
                    graph.canonical[a as usize],
                    graph.canonical[b as usize],
                )) {
                    face.extend_from_slice(&chain(mesh, a, b)?[1..n]);
                }
            }
            output.push(face);
        } else {
            let normal = crate::polygon_edit::face_normal(mesh, ring);
            let p =
                |i: usize| Vec3::from_array(mesh.vertices[ring[i % ring.len()] as usize].position);
            let convex = (0..ring.len())
                .all(|i| (p(i + 1) - p(i)).cross(p(i + 2) - p(i + 1)).dot(normal) >= -1e-10);
            let patches: Vec<Vec<u32>> = if ring.len() == 4 && convex {
                vec![ring.clone()]
            } else {
                crate::triangulate::triangulate(mesh, ring)?
                    .chunks_exact(3)
                    .map(|t| t.to_vec())
                    .collect()
            };
            for patch in patches {
                let boundary: Vec<_> = (0..patch.len())
                    .map(|i| chain(mesh, patch[i], patch[(i + 1) % patch.len()]))
                    .collect::<Result<_, _>>()?;
                let v: Vec<_> = patch.iter().map(|&i| mesh.vertices[i as usize]).collect();
                let mut grid = vec![vec![0; n + 1]; n + 1];
                for j in 0..=n {
                    for i in 0..=if patch.len() == 4 { n } else { n - j } {
                        let id = if j == 0 {
                            boundary[0][i]
                        } else if patch.len() == 4 && i == n {
                            boundary[1][j]
                        } else if patch.len() == 4 && j == n {
                            boundary[2][n - i]
                        } else if i == 0 {
                            boundary[patch.len() - 1][n - j]
                        } else if patch.len() == 3 && i + j == n {
                            boundary[1][j]
                        } else {
                            if mesh.vertices.len() >= MAX_EDIT_VERTICES {
                                return Err(invalid("Подразделение превышает предел вершин"));
                            }
                            let vertex = if patch.len() == 4 {
                                interpolate(
                                    interpolate(v[0], v[1], i as f32 / n as f32),
                                    interpolate(v[3], v[2], i as f32 / n as f32),
                                    j as f32 / n as f32,
                                )
                            } else {
                                interpolate(
                                    interpolate(v[0], v[1], i as f32 / (n - j) as f32),
                                    v[2],
                                    j as f32 / n as f32,
                                )
                            };
                            let id = mesh.vertices.len() as u32;
                            mesh.vertices.push(vertex);
                            id
                        };
                        grid[j][i] = id;
                    }
                }
                for j in 0..n {
                    for i in 0..if patch.len() == 4 { n } else { n - j } {
                        if patch.len() == 4 {
                            output.push(vec![
                                grid[j][i],
                                grid[j][i + 1],
                                grid[j + 1][i + 1],
                                grid[j + 1][i],
                            ]);
                        } else {
                            output.push(vec![grid[j][i], grid[j][i + 1], grid[j + 1][i]]);
                            if i + 1 < n - j {
                                output.push(vec![
                                    grid[j][i + 1],
                                    grid[j + 1][i + 1],
                                    grid[j + 1][i],
                                ]);
                            }
                        }
                    }
                }
            }
        }
        attributes.extend(std::iter::repeat_n(materials[f], output.len() - start));
    }
    let mut wires = Vec::new();
    for [a, b] in mesh.topology.as_ref().unwrap().loose_edges.clone() {
        if cuts.contains(&edge(
            graph.canonical[a as usize],
            graph.canonical[b as usize],
        )) {
            wires.extend(chain(mesh, a, b)?.windows(2).map(|e| [e[0], e[1]]));
        } else {
            wires.push([a, b]);
        }
    }
    let topology = mesh.topology.as_mut().unwrap();
    topology.faces = output.into();
    topology.face_materials = attributes.into();
    topology.loose_edges = wires;
    finish(mesh)
}
