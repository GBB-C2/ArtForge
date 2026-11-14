use super::*;
use glam::Vec2;
use poseforge_core::KnifeCut;
use std::collections::BTreeMap;

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn knife_snaps_vertices_midpoints_and_keeps_angle_constraint() {
        let mut a = App::new(None);
        let mut node = poseforge_core::Node::new(1, "Plane");
        node.mesh = Some(2);
        a.editor = Editor::new(Project {
            nodes: vec![node],
            meshes: vec![poseforge_core::PrimitiveSettings::new(
                poseforge_core::PrimitiveKind::Plane,
            )
            .mesh(2, None)
            .unwrap()],
            ..Default::default()
        });
        a.preferences = Preferences::default();
        a.width = 1440.;
        a.height = 1050.;
        a.editor.mode = EditorMode::Scene;
        a.editor.selected = Some(1);
        a.frame_all();
        a.action(Action::Author(A::SelectMode(
            crate::ui::MeshSelection::Vertex,
        )));
        a.authoring.xray = true;
        a.action(Action::Author(A::Knife));
        let source = &a.editor.project.meshes[0];
        let graph = poseforge_core::MeshConnectivity::new(source);
        let &(i, j) = graph.edges.first().unwrap();
        let p = Vec3::from_array(source.vertices[i].position);
        let q = Vec3::from_array(source.vertices[j].position);
        for (point, expected) in [(p, 0.), (p.lerp(q, 0.5), 0.5)] {
            let mut screen = a.project_point(point).unwrap();
            screen[0] += 2.;
            let hit = a.smart_knife_pick(screen).unwrap();
            assert!((hit.t - expected).abs() < 1e-5);
        }
        let origin = a.project_point(p).unwrap();
        a.authoring.knife.as_mut().unwrap().press = Some(origin);
        a.authoring.knife_angle = 3;
        a.mouse = [origin[0] + 55., origin[1] + 33.];
        let constrained = a.knife_cursor();
        let delta = Vec2::from_array(constrained) - Vec2::from_array(origin);
        assert!((delta.x - delta.y).abs() < 0.001);
        if let Some(hit) = a.smart_knife_pick(constrained) {
            let source = &a.editor.project.meshes[0];
            let p = Vec3::from_array(source.vertices[hit.edge.0].position).lerp(
                Vec3::from_array(source.vertices[hit.edge.1].position),
                hit.t,
            );
            let screen = a.project_point(p).unwrap();
            assert!(
                cross(
                    Vec2::from_array(screen) - Vec2::from_array(origin),
                    delta.normalize()
                )
                .abs()
                    < 0.2
            );
        }
        a.cancel_knife();
        assert_eq!(a.editor.undo_len(), 0);
    }
}
impl App {
    pub(super) fn knife_cursor(&self) -> [f32; 2] {
        let Some(k) = &self.authoring.knife else {
            return self.mouse;
        };
        let origin = k.press.or_else(|| {
            let point = k.first?;
            let globals = self.editor.project.global_transforms().ok()?;
            authoring_controls::project(
                self.view().view_projection(),
                self.viewport(),
                globals[&k.node].transform_point3(point),
            )
        });
        let step = if self.modifiers.shift_key() {
            45_f32
        } else {
            [0., 15., 30., 45., 90.][self.authoring.knife_angle as usize]
        };
        let Some(origin) = origin.filter(|_| step > 0.) else {
            return self.mouse;
        };
        let delta = Vec2::from_array(self.mouse) - Vec2::from_array(origin);
        let angle = (delta.y.atan2(delta.x) / step.to_radians()).round() * step.to_radians();
        (Vec2::from_array(origin) + Vec2::new(angle.cos(), angle.sin()) * delta.length()).to_array()
    }
    pub(super) fn smart_knife_pick(&self, pointer: [f32; 2]) -> Option<EdgePoint> {
        let k = self.authoring.knife.as_ref()?;
        let mesh = self.editor.editable_mesh(k.node).ok()?;
        let model = self.editor.project.global_transforms().ok()?[&k.node];
        let projection = self.view().view_projection();
        let vp = self.viewport();
        let cache = self.mesh_cache(mesh);
        let pointer = Vec2::from_array(pointer);
        let mut best = (4, 12_f32);
        let mut found = None;
        let origin = k
            .press
            .or_else(|| {
                k.first.and_then(|p| {
                    authoring_controls::project(projection, vp, model.transform_point3(p))
                })
            })
            .map(Vec2::from_array);
        let constrained = if self.modifiers.shift_key() || self.authoring.knife_angle != 0 {
            origin.and_then(|o| (pointer - o).try_normalize().map(|d| (o, d)))
        } else {
            None
        };
        for (&(a, b), faces) in &cache.graph.edge_faces {
            if faces.is_empty() {
                continue;
            }
            let pa = model.transform_point3(Vec3::from_array(mesh.vertices[a].position));
            let pb = model.transform_point3(Vec3::from_array(mesh.vertices[b].position));
            let (Some(sa), Some(sb)) = (
                authoring_controls::project(projection, vp, pa),
                authoring_controls::project(projection, vp, pb),
            ) else {
                continue;
            };
            let sa = Vec2::from_array(sa);
            let sb = Vec2::from_array(sb);
            let d = sb - sa;
            if d.length_squared() < 1e-8 {
                continue;
            }
            let along = if let Some((origin, direction)) = constrained {
                let divisor = cross(direction, d);
                if divisor.abs() < 1e-6 {
                    continue;
                }
                let u = cross(sa - origin, direction) / divisor;
                if !(0. ..=1.).contains(&u) {
                    continue;
                }
                u
            } else {
                ((pointer - sa).dot(d) / d.length_squared()).clamp(0., 1.)
            };
            let wa = (projection * pa.extend(1.)).w;
            let wb = (projection * pb.extend(1.)).w;
            let mut t = (along * wa / (wb * (1. - along) + wa * along)).clamp(0., 1.);
            let mut distance = (sa + d * along - pointer).length();
            let mut priority = 2;
            if self.authoring.knife_snap {
                let midpoint = authoring_controls::project(projection, vp, pa.lerp(pb, 0.5))
                    .map(Vec2::from_array);
                let candidates = [
                    (0., sa, 0),
                    (1., sb, 0),
                    (0.5, midpoint.unwrap_or(sa.lerp(sb, 0.5)), 1),
                ];
                for (value, point, rank) in candidates {
                    if constrained.is_some_and(|(o, d)| cross(point - o, d).abs() > 0.15) {
                        continue;
                    }
                    let error = point.distance(pointer);
                    if error <= 7. && (rank < priority || (rank == priority && error < distance)) {
                        t = value;
                        distance = error;
                        priority = rank;
                    }
                }
            }
            if distance >= 12. || priority > best.0 || priority == best.0 && distance >= best.1 {
                continue;
            }
            let point = pa.lerp(pb, t);
            if !self.authoring.xray {
                let screen = authoring_controls::project(projection, vp, point)?;
                let (origin, direction) =
                    authoring_controls::ray(projection.inverse(), vp, screen)?;
                let distance = (point - origin).dot(direction);
                let inverse = model.inverse();
                if cache
                    .hit(
                        inverse.transform_point3(origin),
                        inverse.transform_vector3(direction),
                    )
                    .is_some_and(|(_, hit)| hit + distance.abs().max(1.) * 0.0001 < distance)
                {
                    continue;
                }
            }
            best = (priority, distance);
            found = Some(EdgePoint { edge: (a, b), t });
        }
        found
    }
    pub(super) fn smart_knife_click(&mut self) {
        let cursor = self.knife_cursor();
        let Some(k) = self.authoring.knife.as_ref() else {
            return;
        };
        let node = k.node;
        let Some(hit) = self.smart_knife_pick(cursor) else {
            self.authoring.knife.as_mut().unwrap().press = Some(cursor);
            return;
        };
        let Ok(mesh) = self.editor.editable_mesh(node) else {
            return;
        };
        let point = Vec3::from_array(mesh.vertices[hit.edge.0].position)
            .lerp(Vec3::from_array(mesh.vertices[hit.edge.1].position), hit.t);
        let Ok(globals) = self.editor.project.global_transforms() else {
            return;
        };
        let model = globals[&node];
        let projection = self.view().view_projection();
        let vp = self.viewport();
        let Some(end) = authoring_controls::project(projection, vp, model.transform_point3(point))
        else {
            return;
        };
        if let Some(first) = k
            .first
            .and_then(|p| authoring_controls::project(projection, vp, model.transform_point3(p)))
        {
            self.apply_knife_stroke(first, end);
        } else {
            self.authoring.knife.as_mut().unwrap().first = Some(point);
        }
        self.authoring.knife.as_mut().unwrap().press = Some(end);
        self.redraw();
    }
    pub(crate) fn knife_pointer_up(&mut self) -> bool {
        let Some(start) = self.authoring.knife.as_ref().and_then(|k| k.press) else {
            return false;
        };
        let mut end = self.knife_cursor();
        if Vec2::from_array(start).distance(Vec2::from_array(end)) > 3. {
            if let Some(hit) = self.smart_knife_pick(end) {
                let k = self.authoring.knife.as_ref().unwrap();
                if let (Ok(mesh), Ok(globals)) = (
                    self.editor.editable_mesh(k.node),
                    self.editor.project.global_transforms(),
                ) {
                    let p = Vec3::from_array(mesh.vertices[hit.edge.0].position)
                        .lerp(Vec3::from_array(mesh.vertices[hit.edge.1].position), hit.t);
                    if let Some(screen) = authoring_controls::project(
                        self.view().view_projection(),
                        self.viewport(),
                        globals[&k.node].transform_point3(p),
                    ) {
                        end = screen;
                    }
                }
            }
            self.apply_knife_stroke(start, end);
        }
        if let Some(k) = &mut self.authoring.knife {
            k.press = None;
        }
        true
    }
    fn apply_knife_stroke(&mut self, start: [f32; 2], end: [f32; 2]) {
        let line = Vec2::from_array(end) - Vec2::from_array(start);
        if line.length_squared() < 1e-6 {
            return;
        }
        let Some(k) = &self.authoring.knife else {
            return;
        };
        let node = k.node;
        let Ok(mesh) = self.editor.editable_mesh(node).cloned() else {
            return;
        };
        let Ok(globals) = self.editor.project.global_transforms() else {
            return;
        };
        let model = globals[&node];
        let projection = self.view().view_projection();
        let vp = self.viewport();
        let cache = self.mesh_cache(&mesh);
        let mut crossings = BTreeMap::<usize, Vec<(f32, Vec3)>>::new();
        let mut last = None::<(f32, Vec3)>;
        for (&(a, b), faces) in &cache.graph.edge_faces {
            if faces.is_empty() {
                continue;
            }
            let local_a = Vec3::from_array(mesh.vertices[a].position);
            let local_b = Vec3::from_array(mesh.vertices[b].position);
            let pa = model.transform_point3(local_a);
            let pb = model.transform_point3(local_b);
            let (Some(sa), Some(sb)) = (
                authoring_controls::project(projection, vp, pa),
                authoring_controls::project(projection, vp, pb),
            ) else {
                continue;
            };
            let sa = Vec2::from_array(sa);
            let sb = Vec2::from_array(sb);
            let edge = sb - sa;
            let denominator = cross(line, edge);
            if denominator.abs() < 1e-6 {
                continue;
            }
            let offset = sa - Vec2::from_array(start);
            let t = cross(offset, edge) / denominator;
            let u = cross(offset, line) / denominator;
            if !(-1e-5..=1.00001).contains(&t) || !(-1e-5..=1.00001).contains(&u) {
                continue;
            }
            let u = u.clamp(0., 1.);
            let wa = (projection * pa.extend(1.)).w;
            let wb = (projection * pb.extend(1.)).w;
            let fraction = if u < 1e-5 {
                0.
            } else if u > 1. - 1e-5 {
                1.
            } else {
                u * wa / (wb * (1. - u) + wa * u)
            };
            let point = local_a.lerp(local_b, fraction);
            for &face in faces {
                crossings.entry(face).or_default().push((t, point));
            }
        }
        let mut cuts = Vec::new();
        for (face, mut points) in crossings {
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
            points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-5);
            let projected: Option<Vec<_>> = mesh
                .face_vertices(face)
                .unwrap()
                .iter()
                .map(|&v| {
                    authoring_controls::project(
                        projection,
                        vp,
                        model
                            .transform_point3(Vec3::from_array(mesh.vertices[v as usize].position)),
                    )
                    .map(Vec2::from_array)
                })
                .collect();
            let Some(polygon) = projected else {
                continue;
            };
            for pair in points.windows(2) {
                if pair[0].1.distance_squared(pair[1].1) < 1e-14 {
                    continue;
                }
                let middle = Vec2::from_array(start) + line * (pair[0].0 + pair[1].0) * 0.5;
                let mut inside = false;
                for i in 0..polygon.len() {
                    let a = polygon[i];
                    let b = polygon[(i + 1) % polygon.len()];
                    if (a.y > middle.y) != (b.y > middle.y)
                        && middle.x < (b.x - a.x) * (middle.y - a.y) / (b.y - a.y) + a.x
                    {
                        inside = !inside;
                    }
                }
                if !inside {
                    continue;
                }
                if !self.authoring.xray {
                    let Some((origin, direction)) =
                        authoring_controls::ray(projection.inverse(), vp, middle.to_array())
                    else {
                        continue;
                    };
                    let inverse = model.inverse();
                    if cache
                        .hit(
                            inverse.transform_point3(origin),
                            inverse.transform_vector3(direction),
                        )
                        .is_none_or(|(t, _)| mesh.triangle_face(t) != Some(face))
                    {
                        continue;
                    }
                }
                cuts.push(KnifeCut {
                    face,
                    first: pair[0].1.to_array(),
                    second: pair[1].1.to_array(),
                });
                if last.is_none_or(|last| pair[1].0 > last.0) {
                    last = Some(pair[1]);
                }
            }
        }
        if cuts.is_empty() {
            return;
        }
        let count = cuts.len();
        let result = self
            .editor
            .preview_mesh(node, &mesh, MeshEdit::KnifeStroke { cuts });
        if self.result(result).is_some() {
            let k = self.authoring.knife.as_mut().unwrap();
            k.cuts += count;
            k.first = last.map(|(_, p)| p);
            self.authoring.vertices.clear();
            self.authoring.edges.clear();
            self.authoring.faces.clear();
            self.invalidate_geometry();
            self.update_dirty();
            self.status =
                format!("Нож · разрезано граней: {count} · Enter — применить · Esc — отменить");
        }
    }
}
