//! Selection tools, mesh repair commands and diagnostics share the core editor.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S};
use poseforge_core::MeshEdit;
impl App {
    pub(super) fn mesh_region(&self) -> poseforge_core::MeshRegion {
        poseforge_core::MeshRegion {
            vertices: if self.authoring.selection_mode == S::Vertex {
                self.authoring.vertices.clone()
            } else {
                Default::default()
            },
            edges: if self.authoring.selection_mode == S::Edge {
                self.authoring.edges.clone()
            } else {
                Default::default()
            },
            faces: self.selected_face_region(),
        }
    }
    fn select_geometry_issue(&mut self) {
        let Some(mesh) = self
            .authoring
            .editing
            .and_then(|id| self.editor.editable_mesh(id).ok())
        else {
            return;
        };
        let report = poseforge_core::inspect_topology(mesh);
        let graph = poseforge_core::MeshConnectivity::new(mesh);
        self.authoring.vertices.clear();
        self.authoring.edges.clear();
        self.authoring.faces.clear();
        match poseforge_core::RepairKind::ALL[self.authoring.repair_kind as usize] {
            poseforge_core::RepairKind::Duplicates => {
                self.authoring.selection_mode = S::Vertex;
                self.authoring.vertices = report.duplicate_vertex_ids;
                for f in report.duplicate_face_ids {
                    self.authoring
                        .vertices
                        .extend(mesh.face_vertices(f).unwrap().iter().map(|&v| v as usize));
                }
            }
            poseforge_core::RepairKind::Holes => {
                self.authoring.selection_mode = S::Edge;
                self.authoring.edges = report.boundary_edge_ids;
            }
            poseforge_core::RepairKind::Winding => {
                self.authoring.selection_mode = S::Face;
                self.authoring.faces = report
                    .winding_face_ids
                    .union(&report.inverted_face_ids)
                    .copied()
                    .collect();
            }
            poseforge_core::RepairKind::Degenerate => {
                self.authoring.selection_mode = S::Face;
                self.authoring.faces = report.degenerate_face_ids;
            }
            poseforge_core::RepairKind::NonManifold => {
                self.authoring.selection_mode = S::Face;
                self.authoring.faces = report
                    .nonmanifold_edge_ids
                    .iter()
                    .flat_map(|e| graph.edge_faces[e].iter().copied())
                    .collect();
            }
        }
    }

    pub(super) fn topology_action(&mut self, a: A) -> bool {
        if self.snap_action(a) {
            return true;
        }
        match a {
            A::RepairMode(mode) => self.authoring.repair_kind = mode.min(4),
            A::SelectIssue => self.select_geometry_issue(),
            A::BridgeTwist => {
                self.input(Action::Author(a), self.authoring.bridge_twist.to_string())
            }
            A::BridgeSmooth => self.input(
                Action::Author(a),
                ui::number_caption((self.authoring.bridge_smooth * 100.) as f64),
            ),
            A::Separate => {
                let Some(id) = self.authoring.editing else {
                    return true;
                };
                let result = self.editor.separate_faces(id, &self.selected_face_region());
                if let Some(next) = self.result(result) {
                    self.editor.selected = Some(next);
                    self.scene_tools.selected = BTreeSet::from([next]);
                    self.authoring.editing = Some(next);
                    self.authoring.selection_mode = S::Face;
                    self.authoring.vertices.clear();
                    self.authoring.edges.clear();
                    self.authoring.faces =
                        (0..self.editor.editable_mesh(next).unwrap().face_count()).collect();
                    self.authoring.diagnostics = None;
                    self.invalidate_geometry();
                }
            }
            A::FillHoles | A::Triangulate | A::RepairIssue => {
                let Some(id) = self.authoring.editing else {
                    return true;
                };
                if a == A::RepairIssue && self.edit_vertices().is_empty() {
                    self.select_geometry_issue();
                    if self.edit_vertices().is_empty() {
                        self.status = "Ошибки этой категории не найдены".into();
                        return true;
                    }
                }
                let edit = match a {
                    A::FillHoles => poseforge_core::TopologyEdit::FillHoles,
                    A::Triangulate => poseforge_core::TopologyEdit::Triangulate,
                    _ => poseforge_core::TopologyEdit::Repair(
                        poseforge_core::RepairKind::ALL[self.authoring.repair_kind as usize],
                    ),
                };
                let result = self.editor.edit_mesh(
                    id,
                    MeshEdit::Topology {
                        region: self.mesh_region(),
                        edit,
                    },
                );
                if self.result(result).is_some() {
                    self.authoring.vertices.clear();
                    self.authoring.edges.clear();
                    self.authoring.faces.clear();
                    self.authoring.diagnostics = None;
                    self.invalidate_geometry();
                    if a == A::RepairIssue {
                        self.topology_action(A::Inspect);
                    }
                }
            }

            A::SelectTool(tool) => {
                self.authoring.selection_tool = tool.min(2);
                self.tool = Tool::Move;
                self.status = match tool {
                    1 => "Рамка · потяните в просмотре; Shift — добавить, Ctrl — вычесть",
                    2 => "Лассо · обведите компоненты; Shift — добавить, Ctrl — вычесть",
                    _ => "Щелчок · двойной щелчок — связанные компоненты",
                }
                .into();
            }
            A::EdgeLoop | A::EdgeRing => {
                let Some(id) = self.authoring.editing else {
                    return true;
                };
                let result = self.editor.editable_mesh(id).and_then(|m| {
                    poseforge_core::select_edge_path(m, &self.authoring.edges, a == A::EdgeRing)
                });
                if let Some(edges) = self.result(result) {
                    self.authoring.selection_mode = S::Edge;
                    self.authoring.edges = edges;
                }
            }
            A::WeldDistance => self.input(
                Action::Author(a),
                ui::number_caption(self.authoring.weld_distance as f64),
            ),
            A::BridgeSegments => self.input(
                Action::Author(a),
                self.authoring.bridge_segments.to_string(),
            ),
            A::Merge | A::Weld | A::Bridge | A::BridgeFaces | A::Dissolve => {
                let Some(id) = self.authoring.editing else {
                    return true;
                };
                let operation = match a {
                    A::Merge => MeshEdit::MergeVertices {
                        vertices: self.edit_vertices(),
                    },
                    A::Weld => MeshEdit::WeldVertices {
                        vertices: self.edit_vertices(),
                        distance: self.authoring.weld_distance,
                    },
                    A::Bridge | A::BridgeFaces => MeshEdit::BridgeAdvanced {
                        faces: if a == A::BridgeFaces || self.authoring.selection_mode==S::Face {
                            self.selected_face_region()
                        } else {
                            Default::default()
                        },
                        edges: if a == A::Bridge {
                            self.workflow_bridge_edges()
                        } else {
                            Default::default()
                        },
                        segments: self.authoring.bridge_segments,
                        twist: self.authoring.bridge_twist,
                        smooth: self.authoring.bridge_smooth,
                    },
                    _ => MeshEdit::Dissolve {region:self.mesh_region()},
                };
                let result = self.editor.edit_mesh(id, operation);
                if self.result(result).is_some() {
                    self.authoring.vertices.clear();
                    self.authoring.edges.clear();
                    self.authoring.faces.clear();
                    self.authoring.diagnostics = None;
                    self.invalidate_geometry();
                }
            }
            A::Inspect | A::SelectDefects => {
                if let Some(mesh) = self
                    .authoring
                    .editing
                    .and_then(|id| self.editor.editable_mesh(id).ok())
                {
                    let report = poseforge_core::inspect_topology(mesh);
                    let key = mesh_pick::MeshCache::key(mesh);
                    if a == A::SelectDefects {
                        self.authoring.selection_mode = S::Face;
                        self.authoring.faces = report.problem_faces.clone();
                        self.authoring.edges.clear();
                        self.authoring.vertices.clear();
                    }
                    self.status=format!("Границы: {}; рёбра >2 граней: {}; направление: {}; дубли: {}; вырожденные: {}; свободные вершины: {}",report.boundary_edges,report.nonmanifold_edges,report.winding_edges,report.duplicate_faces,report.degenerate_faces,report.isolated_vertices);
                    self.authoring.diagnostics = Some((key, report));
                }
            }
            _ => return false,
        }
        true
    }
    pub(super) fn topology_confirm(&mut self, a: &Action, text: &str) -> Result<bool, String> {
        match a {
            Action::Author(A::BridgeTwist) => {
                let v = surfaces::number(text, -4096., 4096.)?;
                if v.fract() != 0. {
                    return Err("Скручивание задаётся целым числом шагов".into());
                }
                self.authoring.bridge_twist = v as i32;
            }
            Action::Author(A::BridgeSmooth) => {
                self.authoring.bridge_smooth = surfaces::number(text, 0., 100.)? / 100.
            }
            Action::Author(A::WeldDistance) => {
                self.authoring.weld_distance = surfaces::number(text, 0.000001, 10000.)?
            }
            Action::Author(A::BridgeSegments) => {
                let v = surfaces::number(text, 1., 32.)?;
                if v.fract() != 0. {
                    return Err("Число сегментов должно быть целым".into());
                }
                self.authoring.bridge_segments = v as u32;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub(super) fn draw_topology_report(&self, u: &mut Ui, x: f32, mut y: f32, w: f32) {
        let p = u.palette();
        if let Some((key, report)) = &self.authoring.diagnostics {
            if self
                .authoring
                .editing
                .and_then(|id| self.editor.editable_mesh(id).ok())
                .is_some_and(|m| mesh_pick::MeshCache::key(m) == *key)
            {
                for text in [
                    format!("Открытых рёбер: {}", report.boundary_edges),
                    format!(
                        "Развилки / направление: {} / {}",
                        report.nonmanifold_edges, report.winding_edges
                    ),
                    format!(
                        "Дубли / вырожденные: {} / {}",
                        report.duplicate_faces, report.degenerate_faces
                    ),
                    format!("Совпадающих точек: {}", report.duplicate_vertex_ids.len()),
                    format!("Граней внутрь: {}", report.inverted_face_ids.len()),
                    format!("Свободных вершин: {}", report.isolated_vertices),
                ] {
                    u.fitted_label([x, y, w, 18.], &text, 11., p.muted);
                    y += 22.;
                }
            }
        }
    }
}
