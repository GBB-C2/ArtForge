//! Selection tools, mesh repair commands and diagnostics share the core editor.
use super::*;
use crate::ui::{AuthorAction as A, MeshSelection as S};
use poseforge_core::MeshEdit;
impl App {
    pub(super) fn topology_action(&mut self, a: A) -> bool {
        match a {
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
            A::Merge | A::Weld | A::Bridge | A::Dissolve => {
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
                    A::Bridge => MeshEdit::Bridge {
                        edges: self.authoring.edges.clone(),
                        segments: self.authoring.bridge_segments,
                    },
                    _ => MeshEdit::DissolveEdges {
                        edges: self.authoring.edges.clone(),
                    },
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
                    format!("Свободных вершин: {}", report.isolated_vertices),
                ] {
                    u.fitted_label([x, y, w, 18.], &text, 11., p.muted);
                    y += 22.;
                }
            }
        }
    }
}
