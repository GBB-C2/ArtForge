//! Faceted octahedral bones and matching circular joints in viewport pixels.
//!
//! A segment belongs to the joint at its base: dragging that joint's rotation
//! moves the displayed segment. Skin formats provide joint origins, not bone
//! tails, so joint origins use small filled circular controls. An isolated joint
//! has a short display-only tail; extending it replaces that guide with its real
//! child connection. Projection and picking share the same segment geometry.
use crate::ui::{self, Ui};
use glam::{Mat4, Vec2, Vec3, Vec4};
use poseforge_core::{BoneDisplay, BoneRole, BoneSide, EvaluatedScene, JointShape, Node, NodeId, Project};
use poseforge_render::View;
use std::collections::{BTreeMap, BTreeSet};

#[path = "bone_shape.rs"]
mod shape;
#[cfg(test)]
#[path = "bone_display_tests.rs"]
mod display_tests;

const JOINT_RADIUS: f32 = 5.;
const ACTIVE_JOINT_RADIUS: f32 = 6.;
// Keep the small visible circle easy to acquire without changing the bone hit
// geometry. Exact joint endpoints retain priority over neighboring bodies.
const JOINT_HIT_RADIUS: f32 = 8.;

pub fn role_color(role: Option<BoneRole>, palette: &ui::Palette) -> [f32; 4] {
    match role.map(BoneRole::side) {
        Some(Some(BoneSide::Left)) => [0.94, 0.27, 0.32, 1.],
        Some(Some(BoneSide::Right)) => [0.24, 0.52, 0.96, 1.],
        Some(None) => [0.25, 0.76, 0.46, 1.],
        None => palette.muted,
    }
}

#[derive(Clone, Copy)]
struct NodeState {
    visible: bool,
    locked: bool,
}

struct Bone {
    owner: NodeId,
    shape: shape::Shape,
    depth: f32,
    locked: bool,
    display: Option<BoneDisplay>,
}

#[derive(Clone)]
struct JointControl {
    id: NodeId,
    center: Vec2,
    depth: f32,
    locked: bool,
    display: Option<BoneDisplay>,
    basis: [Vec2;2],
}

impl JointControl {
    fn polygon(&self, radius: f32) -> Vec<Vec2> {
        let d = self.display.unwrap_or_default();
        let points = match d.joint_shape {
            JointShape::Rectangle => vec![Vec2::new(-1.,-1.),Vec2::new(1.,-1.),Vec2::new(1.,1.),Vec2::new(-1.,1.)],
            JointShape::Diamond => vec![Vec2::Y,Vec2::X,-Vec2::Y,-Vec2::X],
            shape => {
                let count = match shape { JointShape::Triangle => 3, JointShape::Hexagon => 6, _ => 24 };
                (0..count).map(|i| {
                    let angle = i as f32*std::f32::consts::TAU/count as f32 - std::f32::consts::FRAC_PI_2;
                    Vec2::new(angle.cos(),angle.sin())
                }).collect()
            }
        };
        points.into_iter().map(|p| self.center+(self.basis[0]*p.x+self.basis[1]*p.y)*radius*d.joint_size).collect()
    }

    fn hit(&self, point: Vec2) -> Option<f32> {
        let Some(d) = self.display else {
            let distance = point.distance(self.center);
            return (distance<=JOINT_HIT_RADIUS).then_some(distance);
        };
        let points = self.polygon(JOINT_RADIUS);
        let mut inside = false;
        let mut distance = f32::INFINITY;
        for i in 0..points.len() {
            let (a,b) = (points[i],points[(i+1)%points.len()]);
            distance = distance.min(shape::distance_to_segment(point,a,b));
            if (a.y>point.y)!=(b.y>point.y)
                && point.x<(b.x-a.x)*(point.y-a.y)/(b.y-a.y)+a.x { inside = !inside; }
        }
        if d.joint_filled && inside { Some(0.) } else { (distance<=3.).then_some(distance) }
    }
}

pub struct BoneOverlay {
    viewport: [f32; 4],
    bones: Vec<Bone>,
    joints: Vec<JointControl>,
}

impl BoneOverlay {
    pub fn new(
        project: &Project,
        evaluated: &EvaluatedScene,
        view: &View,
        viewport: [f32; 4],
    ) -> Self {
        let mut result = Self {
            viewport,
            bones: Vec::new(),
            joints: Vec::new(),
        };
        if !viewport.iter().all(|v| v.is_finite()) || viewport[2] <= 0. || viewport[3] <= 0. {
            return result;
        }
        let matrix = view.view_projection();
        if !matrix.is_finite() {
            return result;
        }
        let nodes: BTreeMap<_, _> = project.nodes.iter().map(|n| (n.id, n)).collect();
        let joint_parents: BTreeMap<_, _> = project.nodes.iter().filter(|n| n.is_joint)
            .map(|n| (n.id, nearest_joint_parent(n, &nodes))).collect();
        let parents_with_children: BTreeSet<_> = joint_parents.values().filter_map(|id| *id).collect();
        let visible_ids = project.visible_node_ids(false);
        let selectable_ids = project.selectable_node_ids();
        let states: BTreeMap<_, _> = project
            .nodes
            .iter()
            .filter(|n| n.is_joint)
            .filter_map(|n| {
                inherited_state(n.id, &nodes).map(|mut s| {
                    s.visible &= visible_ids.contains(&n.id);
                    s.locked |= !selectable_ids.contains(&n.id);
                    (n.id, s)
                })
            })
            .collect();
        for child in project.nodes.iter().filter(|n| n.is_joint) {
            if !states.get(&child.id).is_some_and(|s| s.visible) {
                continue;
            }
            let (owner, local_tip, show_tip) = if let Some(tip) = child.bone_tip {
                (child.id,Some(tip),!parents_with_children.contains(&child.id))
            } else { match joint_parents[&child.id] {
                Some(parent) if nodes[&parent].bone_tip.is_none() => (parent,None,false),
                None if !parents_with_children.contains(&child.id) => (child.id,Some(Vec3::Y*0.35),true),
                _ => continue,
            }};
            let Some(owner_state) = states.get(&owner).filter(|s| s.visible) else {
                continue;
            };
            let (Some(head), Some(tail)) = (
                evaluated.global.get(&owner),
                evaluated.global.get(&child.id),
            ) else {
                continue;
            };
            let display = nodes[&owner].bone_display;
            let tail = if let Some(tip) = local_tip { head.transform_point3(tip) } else { tail.w_axis.truncate() };
            if let Some(shape) = shape::Shape::new(*head, tail, view, viewport,display.unwrap_or_default()) {
                result.bones.push(Bone { owner, depth: shape.depth, shape, locked: owner_state.locked, display });
            }
            if show_tip {
                if let Some((center, depth)) = project_point(matrix, viewport, tail)
                    .filter(|(center,_)| ui::contains(viewport, center.to_array()))
                {
                    let global = Mat4::from_cols(head.x_axis,head.y_axis,head.z_axis,tail.extend(1.));
                    let basis = if display.is_some_and(|d| !d.joint_follow_view) {
                        shape::joint_basis(global,view,viewport).unwrap_or([Vec2::X,Vec2::Y])
                    } else { [Vec2::X,Vec2::Y] };
                    result.joints.push(JointControl { id: owner, center, depth, locked: owner_state.locked, display,basis });
                }
            }
        }
        for joint in project.nodes.iter().filter(|n| n.is_joint) {
            let Some(state) = states.get(&joint.id).filter(|s| s.visible) else {
                continue;
            };
            let Some(global) = evaluated.global.get(&joint.id) else {
                continue;
            };
            let Some((center, depth)) = project_point(matrix, viewport, global.w_axis.truncate())
            else {
                continue;
            };
            if !ui::contains(viewport, center.to_array()) {
                continue;
            }
            result.joints.push(JointControl {
                id: joint.id,
                center,
                depth,
                locked: state.locked,
                display: joint.bone_display,
                basis: if joint.bone_display.is_some_and(|d| !d.joint_follow_view) {
                    shape::joint_basis(*global,view,viewport).unwrap_or([Vec2::X,Vec2::Y])
                } else { [Vec2::X,Vec2::Y] },
            });
        }
        // Painter order agrees with depth tie-breaking during picking.
        result
            .bones
            .sort_by(|a, b| b.depth.total_cmp(&a.depth).then(a.owner.cmp(&b.owner)));
        result
            .joints
            .sort_by(|a, b| b.depth.total_cmp(&a.depth).then(a.id.cmp(&b.id)));
        result
    }

    /// Pick the complete bone body, with a small logical-pixel tolerance.
    /// At a shared endpoint the joint there wins, so an elbow is selected at
    /// the elbow and the upper arm is selected along its tapered body.
    pub fn hit(&self, pointer: [f32; 2]) -> Option<NodeId> {
        if !pointer.iter().all(|v| v.is_finite()) || !ui::contains(self.viewport, pointer) {
            return None;
        }
        let point = Vec2::from_array(pointer);
        let joint = self
            .joints
            .iter()
            .filter(|j| !j.locked)
            .filter_map(|j| {
                j.hit(point).map(|distance| (distance, j.depth, j.id))
            })
            .min_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then(a.1.total_cmp(&b.1))
                    .then(a.2.cmp(&b.2))
            });
        if let Some((_, _, id)) = joint {
            return Some(id);
        }
        self.bones
            .iter()
            .filter(|b| !b.locked)
            .filter_map(|bone| {
                let hit = if bone.display.is_some_and(|d| !d.filled) { bone.shape.hit_outline(point) }
                    else { bone.shape.hit(point) };
                hit.map(|(distance, depth)| (distance, depth, bone.owner))
            })
            .min_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then(a.1.total_cmp(&b.1))
                    .then(a.2.cmp(&b.2))
            })
            .map(|(_, _, id)| id)
    }

    #[cfg(test)]
    pub fn draw(&self, ui: &mut Ui, selected: Option<NodeId>, hovered: Option<NodeId>) {
        self.draw_selected(ui, &selected.into_iter().collect(), hovered);
    }
    pub fn draw_selected(&self, ui: &mut Ui, selected: &BTreeSet<NodeId>, hovered: Option<NodeId>) {
        let palette = ui.palette();
        // Selected connections remain above other chains; endpoints retain their own selection.
        for active_pass in [false, true] {
            for bone in &self.bones {
                let active = selected.contains(&bone.owner) || hovered == Some(bone.owner);
                if active != active_pass {
                    continue;
                }
                let fill = display_color(&palette, bone.locked, active,bone.display.map(|d| d.color));
                // A bounded number of GPU triangles replaces scanline fills;
                // body size and DPI do not multiply the CPU primitive count.
                for face in bone.shape.faces.iter().filter(|_| bone.display.is_none_or(|d| d.filled)) {
                    let color = [(fill[0]*face.shade).min(1.), (fill[1]*face.shade).min(1.),
                        (fill[2]*face.shade).min(1.), fill[3]];
                    for i in 1..face.points.len()-1 {
                        ui.mesh_triangle([face.points[0].to_array(), face.points[i].to_array(),
                            face.points[i+1].to_array()], color, false);
                    }
                }
                let outline = if active && !bone.locked { palette.accent }
                    else if bone.display.is_some_and(|d| !d.filled) { fill } else { [0.22,0.25,0.27,1.] };
                for [a,b] in &bone.shape.edges {
                    stroke_segment(ui, *a, *b, if active { 1.5 } else { 0.9 }, outline, self.viewport);
                }
            }
        }
        for active_pass in [false, true] {
            for joint in &self.joints {
                let active = selected.contains(&joint.id) || hovered == Some(joint.id);
                if active != active_pass {
                    continue;
                }
                let radius = if active {
                    ACTIVE_JOINT_RADIUS
                } else {
                    JOINT_RADIUS
                };
                let color = display_color(&palette, joint.locked, active,joint.display.map(|d| d.joint_color));
                if let Some(d) = joint.display {
                    let points = joint.polygon(radius);
                    if d.joint_filled { fill_polygon(ui,&points,color,self.viewport); }
                    let outline = if active && !joint.locked { palette.accent } else { color };
                    for i in 0..points.len() {
                        if let Some((a,b)) = clip_segment(points[i],points[(i+1)%points.len()],self.viewport) {
                            stroke_segment(ui,a,b,if active { 1.5 } else { 1. },outline,self.viewport);
                        }
                    }
                } else {
                    fill_circle(ui, joint.center, radius + 1.4, [0.015, 0.02, 0.03, 0.8], self.viewport);
                    fill_circle(ui, joint.center, radius, color, self.viewport);
                }
            }
        }
    }
}

fn bone_color(palette: &ui::Palette, locked: bool, active: bool) -> [f32; 4] {
    if locked { palette.disabled } else if active { palette.accent }
    else { [0.58, 0.62, 0.64, 1.] }
}

fn display_color(palette: &ui::Palette, locked: bool, active: bool, custom: Option<[f32;3]>) -> [f32;4] {
    if locked { palette.disabled }
    else if let Some([r,g,b]) = custom { [r,g,b,1.] }
    else { bone_color(palette,locked,active) }
}

fn stroke_segment(ui: &mut Ui, a: Vec2, b: Vec2, width: f32, color: [f32; 4], viewport: [f32; 4]) {
    let start = ui.rects.len();
    ui.mesh_segment(a.to_array(), b.to_array(), width, color);
    let Some(rect) = ui.rects.get_mut(start) else { return; };
    // The GPU stroke includes an antialiasing pad. Clip that pad as well as the
    // endpoints so it cannot leak into adjacent editor panels.
    let [x,y,w,h] = viewport.map(|v| v*ui.scale);
    let left = rect.rect[0].max(x);
    let top = rect.rect[1].max(y);
    let right = (rect.rect[0]+rect.rect[2]).min(x+w);
    let bottom = (rect.rect[1]+rect.rect[3]).min(y+h);
    if right <= left || bottom <= top { ui.rects.truncate(start); }
    else { rect.rect = [left,top,right-left,bottom-top]; }
}

fn inherited_state(id: NodeId, nodes: &BTreeMap<NodeId, &Node>) -> Option<NodeState> {
    let mut cursor = Some(id);
    let mut seen = BTreeSet::new();
    let mut result = NodeState {
        visible: true,
        locked: false,
    };
    while let Some(id) = cursor {
        if !seen.insert(id) {
            return None;
        }
        let node = nodes.get(&id)?;
        result.visible &= node.visible;
        result.locked |= node.locked;
        cursor = node.parent;
    }
    Some(result)
}

fn nearest_joint_parent(node: &Node, nodes: &BTreeMap<NodeId, &Node>) -> Option<NodeId> {
    let mut cursor = node.parent;
    let mut seen = BTreeSet::from([node.id]);
    while let Some(id) = cursor {
        if !seen.insert(id) {
            return None;
        }
        let parent = nodes.get(&id)?;
        if parent.is_joint {
            return Some(id);
        }
        cursor = parent.parent;
    }
    None
}

fn screen(viewport: [f32; 4], clip: Vec4) -> Option<(Vec2, f32)> {
    if !clip.is_finite() || clip.w <= 1e-7 {
        return None;
    }
    let p = clip.truncate() / clip.w;
    let screen = Vec2::new(
        viewport[0] + (p.x + 1.) * viewport[2] * 0.5,
        viewport[1] + (1. - p.y) * viewport[3] * 0.5,
    );
    (screen.is_finite() && p.z.is_finite()).then_some((screen, p.z))
}

fn project_point(matrix: Mat4, viewport: [f32; 4], point: Vec3) -> Option<(Vec2, f32)> {
    let clip = matrix * point.extend(1.);
    if clip.z < 0. || clip.z > clip.w {
        return None;
    }
    screen(viewport, clip)
}

/// Clip in homogeneous depth first, so a segment crossing the near plane never
/// disappears merely because one endpoint is behind the camera.
pub(crate) fn project_segment(
    matrix: Mat4,
    viewport: [f32; 4],
    head: Vec3,
    tail: Vec3,
) -> Option<(Vec2, Vec2, f32)> {
    let a = matrix * head.extend(1.);
    let b = matrix * tail.extend(1.);
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    let mut start = 0f32;
    let mut end = 1f32;
    for (fa, fb) in [(a.z, b.z), (a.w - a.z, b.w - b.z), (a.w - 1e-6, b.w - 1e-6)] {
        if fa < 0. && fb < 0. {
            return None;
        }
        if (fa < 0.) != (fb < 0.) {
            let t = fa / (fa - fb);
            if fa < 0. {
                start = start.max(t);
            } else {
                end = end.min(t);
            }
        }
    }
    if start > end {
        return None;
    }
    let (head, head_depth) = screen(viewport, a.lerp(b, start))?;
    let (tail, tail_depth) = screen(viewport, a.lerp(b, end))?;
    Some((head, tail, (head_depth + tail_depth) * 0.5))
}

pub(crate) fn clip_segment(a: Vec2, b: Vec2, viewport: [f32; 4]) -> Option<(Vec2, Vec2)> {
    let delta = b - a;
    let mut start = 0f32;
    let mut end = 1f32;
    for (p, q) in [
        (-delta.x, a.x - viewport[0]),
        (delta.x, viewport[0] + viewport[2] - a.x),
        (-delta.y, a.y - viewport[1]),
        (delta.y, viewport[1] + viewport[3] - a.y),
    ] {
        if p.abs() < 1e-8 {
            if q < 0. {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0. {
                start = start.max(t);
            } else {
                end = end.min(t);
            }
        }
    }
    (start <= end).then(|| (a + delta * start, a + delta * end))
}

fn fill_circle(ui: &mut Ui, center: Vec2, radius: f32, color: [f32; 4], viewport: [f32; 4]) {
    let points: [Vec2; 24] = std::array::from_fn(|i| {
        let angle = i as f32 * std::f32::consts::TAU / 24.;
        let (sin, cos) = angle.sin_cos();
        center + Vec2::new(cos, sin) * radius
    });
    fill_polygon(ui, &points, color, viewport);
}

fn fill_polygon(ui: &mut Ui, polygon: &[Vec2], color: [f32; 4], viewport: [f32; 4]) {
    let mut points = polygon.to_vec();
    for (axis, value, keep_greater) in [
        (0, viewport[0], true),
        (0, viewport[0] + viewport[2], false),
        (1, viewport[1], true),
        (1, viewport[1] + viewport[3], false),
    ] {
        if points.is_empty() {
            return;
        }
        let mut clipped = Vec::with_capacity(points.len() + 1);
        let mut previous = *points.last().unwrap();
        let mut previous_inside = if keep_greater {
            previous[axis] >= value
        } else {
            previous[axis] <= value
        };
        for current in points {
            let current_inside = if keep_greater {
                current[axis] >= value
            } else {
                current[axis] <= value
            };
            if current_inside != previous_inside {
                let t = (value - previous[axis]) / (current[axis] - previous[axis]);
                let mut intersection = previous.lerp(current, t);
                // The clipped coordinate is exact. An interpolation overshoot
                // of a few ulps otherwise becomes a full extra raster column.
                intersection[axis] = value;
                clipped.push(intersection);
            }
            if current_inside {
                clipped.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
        points = clipped;
    }
    let minimum = Vec2::new(viewport[0], viewport[1]);
    let maximum = minimum + Vec2::new(viewport[2], viewport[3]);
    let points: Vec<_> = points
        .into_iter()
        .map(|point| point.clamp(minimum, maximum).to_array())
        .collect();
    ui.convex_polygon(&points, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Theme;
    use poseforge_core::Transform;

    fn view() -> View {
        View {
            eye: Vec3::new(0., 0., 6.),
            target: Vec3::ZERO,
            up: Vec3::Y,
            viewport: [20., 30., 800., 600.],
            orthographic: Some(2.),
            ..View::default()
        }
    }

    fn joint(id: NodeId, parent: Option<NodeId>, translation: Vec3) -> Node {
        let mut node = Node::new(id, format!("Joint {id}"));
        node.is_joint = true;
        node.parent = parent;
        node.transform = Transform::from_translation(translation);
        node
    }

    fn chain() -> Project {
        Project {
            nodes: vec![
                joint(1, None, Vec3::new(-1., 0., 0.)),
                joint(2, Some(1), Vec3::X),
                joint(3, Some(2), Vec3::X),
            ],
            ..Project::default()
        }
    }

    fn overlay(project: &Project) -> BoneOverlay {
        BoneOverlay::new(
            project,
            &project.evaluated(None, 0.).unwrap(),
            &view(),
            view().viewport,
        )
    }

    fn pointer(point: Vec3) -> [f32; 2] {
        project_point(view().view_projection(), view().viewport, point)
            .unwrap()
            .0
            .to_array()
    }

    #[test]
    fn empty_bone_guide_is_pickable_and_extension_replaces_it() {
        let mut project = Project { nodes: vec![joint(1, None, Vec3::ZERO)], ..Default::default() };
        let single = overlay(&project);
        assert_eq!(single.bones.len(), 1);
        assert_eq!(single.joints.len(), 2);
        assert_eq!(single.hit(pointer(Vec3::Y * 0.175)), Some(1));
        assert_eq!(single.hit(pointer(Vec3::Y * 0.35)), Some(1));
        project.nodes.push(joint(2, Some(1), Vec3::Y * 0.75));
        let extended = overlay(&project);
        assert_eq!(extended.bones.len(), 1);
        assert_eq!(extended.joints.len(), 2);
        assert_eq!(extended.hit(pointer(Vec3::Y * 0.5)), Some(1));
        assert_eq!(extended.hit(pointer(Vec3::Y * 0.75)), Some(2));
        project.nodes[0].visible = false;
        assert!(overlay(&project).bones.is_empty());
    }

    #[test]
    fn bone_body_selects_its_base_and_shared_endpoint_selects_the_joint() {
        let overlay = overlay(&chain());
        assert_eq!(overlay.bones.len(), 2);
        assert_eq!(overlay.hit(pointer(Vec3::new(-0.45, 0., 0.))), Some(1));
        assert_eq!(overlay.hit(pointer(Vec3::new(0.55, 0., 0.))), Some(2));
        assert_eq!(overlay.hit(pointer(Vec3::ZERO)), Some(2));
        assert_eq!(overlay.hit(pointer(Vec3::X)), Some(3));
    }

    #[test]
    fn every_joint_has_a_larger_filled_circle_with_contrast_halo() {
        let mut overlay = overlay(&chain());
        // Inspect the markers independently of the adjacent body fills. The
        // parent and middle joints must be visible even without selection.
        overlay.bones.clear();
        for scale in [1., 2.] {
            for theme in [Theme::Dark, Theme::Light] {
                let mut ui = Ui::with_theme(scale, theme, [0., 0.]);
                overlay.draw(&mut ui, None, None);
                let color = bone_color(&theme.palette(), false, false);
                assert!(ui.rects.iter().any(|r| r.color[..3] == color[..3]));
                assert!(ui.rects.iter().any(|r| r.color[..3] == [0.015, 0.02, 0.03]));
                let alpha_at = |point: Vec2| {
                    let point = (point * scale).to_array();
                    ui.rects
                        .iter()
                        .filter(|r| ui::contains(r.rect, point))
                        .fold(0f32, |alpha, r| alpha + r.color[3] * (1. - alpha))
                };
                for joint in &overlay.joints {
                    assert!(alpha_at(joint.center + Vec2::splat(0.25)) > 0.99);
                    assert!(alpha_at(joint.center + Vec2::new(3.5, 1.5)) > 0.05);
                    assert_eq!(alpha_at(joint.center + Vec2::new(7.5, 0.5)), 0.);
                    assert_eq!(
                        overlay.hit((joint.center + Vec2::new(7., 0.)).to_array()),
                        Some(joint.id)
                    );
                }
                let mut active = Ui::with_theme(scale, theme, [0., 0.]);
                overlay.draw(&mut active, Some(2), None);
                assert!(active
                    .rects
                    .iter()
                    .any(|r| r.color[..3] == theme.palette().accent[..3]));
            }
        }
    }

    #[test]
    fn octahedral_bodies_are_shaded_tapered_and_pickable_across_their_width() {
        let mut overlay = overlay(&chain());
        overlay.bones.truncate(1);
        overlay.joints.clear();
        // The broad part is much wider than the old six-pixel stick hit target.
        assert_eq!(overlay.hit(pointer(Vec3::new(-0.8,0.075,0.))),Some(1));
        assert_eq!(overlay.hit(pointer(Vec3::new(-0.1,0.075,0.))),None);
        let geometry = &overlay.bones[0].shape;
        assert!(geometry.faces.len() >= 2);
        assert!(geometry.faces.windows(2).any(|p| (p[0].shade-p[1].shade).abs()>0.01));
        for scale in [1., 2.] {
            for theme in [Theme::Dark, Theme::Light] {
                for selected in [None, Some(overlay.bones[0].owner)] {
                    let mut ui = Ui::with_theme(scale, theme, [0., 0.]);
                    overlay.draw(&mut ui, selected, None);
                    assert!(!ui.rects.is_empty());
                    assert!(ui.rects.iter().any(|r| r.triangle.is_some()));
                    assert!(ui.rects.iter().any(|r| r.segment.is_some()));
                    assert!(ui.rects.len() <= 20, "body cost must not grow with size or DPI");
                    assert!(ui.rects.iter().all(|r| r.rect.iter().chain(&r.color).all(|v| v.is_finite())));
                }
            }
        }
    }

    #[test]
    fn non_joint_helpers_preserve_segment_ownership_and_branches() {
        let mut project = chain();
        project.nodes[1].is_joint = false;
        project
            .nodes
            .push(joint(4, Some(1), Vec3::new(0.8, 0.6, 0.)));
        let overlay = overlay(&project);
        assert_eq!(overlay.bones.len(), 2);
        assert!(overlay.bones.iter().all(|b| b.owner == 1));
        assert_eq!(overlay.hit(pointer(Vec3::new(0.2, 0., 0.))), Some(1));
        assert_eq!(overlay.hit(pointer(Vec3::new(-0.6, 0.3, 0.))), Some(1));
    }

    #[test]
    fn hidden_and_locked_ancestors_apply_to_drawing_and_picking() {
        let mut project = chain();
        let mut container = Node::new(10, "Container");
        container.visible = false;
        project.nodes[0].parent = Some(10);
        project.nodes.push(container);
        let hidden = overlay(&project);
        assert!(hidden.bones.is_empty() && hidden.joints.is_empty());
        project.nodes.last_mut().unwrap().visible = true;
        project.nodes.last_mut().unwrap().locked = true;
        let locked = overlay(&project);
        assert_eq!(locked.bones.len(), 2);
        assert_eq!(locked.hit(pointer(Vec3::new(-0.45, 0., 0.))), None);
        assert_eq!(locked.hit(pointer(Vec3::X)), None);
    }

    #[test]
    fn end_on_and_zero_length_joints_remain_finite_and_selectable() {
        let project = Project {
            nodes: vec![
                joint(1, None, Vec3::ZERO),
                joint(2, Some(1), Vec3::Z),
                joint(3, Some(2), Vec3::ZERO),
            ],
            ..Project::default()
        };
        let overlay = overlay(&project);
        // An end-on octahedron has a visible cap; only the zero-length link is omitted.
        assert_eq!(overlay.bones.len(),1);
        assert_eq!(overlay.joints.len(), 3);
        assert!(overlay.hit(pointer(Vec3::ZERO)).is_some());
        for theme in [Theme::Dark, Theme::Light] {
            let mut ui = Ui::with_theme(2., theme, [0., 0.]);
            overlay.draw(&mut ui, Some(1), None);
            assert!(!ui.rects.is_empty());
            assert!(ui
                .rects
                .iter()
                .all(|r| r.rect.iter().all(|v| v.is_finite())));
        }
    }

    #[test]
    fn octahedral_faces_follow_joint_roll_and_camera_rotation() {
        let mut project = Project { nodes: vec![joint(1,None,Vec3::ZERO),
            joint(2,Some(1),Vec3::Y)], ..Default::default() };
        let before = overlay(&project);
        let points = |overlay: &BoneOverlay| overlay.bones[0].shape.faces.iter()
            .flat_map(|f| f.points.iter().map(|p| p.to_array())).collect::<Vec<_>>();
        project.nodes[0].transform.rotation = glam::Quat::from_rotation_y(0.6);
        let rolled = overlay(&project);
        assert_ne!(points(&before),points(&rolled));
        assert_eq!(before.joints[0].center,rolled.joints[0].center);
        let turned_view = View { eye: Vec3::new(4.,2.,6.), ..view() };
        let turned = BoneOverlay::new(&project,&project.evaluated(None,0.).unwrap(),&turned_view,turned_view.viewport);
        assert_ne!(points(&rolled),points(&turned));
        for shape in [before, rolled, turned] {
            assert!(!shape.bones[0].shape.edges.is_empty());
            assert!(shape.bones[0].shape.faces.iter().all(|f| f.points.iter().all(|p| p.is_finite())));
        }
    }

    #[test]
    fn near_plane_clips_bone_faces_and_strokes_before_drawing() {
        let project = Project { nodes: vec![joint(1,None,Vec3::new(-1.,0.,7.)),
            joint(2,Some(1),Vec3::new(1.,0.5,-7.))], ..Default::default() };
        let perspective = View { orthographic: None, ..view() };
        let overlay = BoneOverlay::new(&project,&project.evaluated(None,0.).unwrap(),&perspective,perspective.viewport);
        assert_eq!(overlay.bones.len(),1);
        assert!(overlay.bones[0].shape.faces.iter().all(|f| f.points.iter().all(|p| (0. ..=1.).contains(&p.z))));
        let mut ui = Ui::new(2.);
        overlay.draw(&mut ui,None,None);
        let [x,y,w,h] = perspective.viewport.map(|v| v*2.);
        assert!(!ui.rects.is_empty());
        for primitive in &ui.rects {
            let r = primitive.rect;
            assert!(r.iter().all(|v| v.is_finite()));
            assert!(r[0]>=x-0.01 && r[1]>=y-0.01 && r[0]+r[2]<=x+w+0.01 && r[1]+r[3]<=y+h+0.01);
        }
    }

    #[test]
    fn reference_style_uses_matching_bone_and_joint_colors_and_exports_actual_primitives() {
        let project = Project { nodes: vec![joint(1,None,Vec3::new(-1.,1.,0.)),
            joint(2,Some(1),Vec3::new(0.55,-0.45,0.12)),
            joint(3,Some(2),Vec3::new(1.9,-1.45,-0.22))], ..Default::default() };
        let evaluated = project.evaluated(None,0.).unwrap();
        for (name,theme,selected,eye) in [
            ("normal",Theme::Dark,None,Vec3::new(1.5,0.8,7.)),
            ("selected",Theme::Dark,Some(2),Vec3::new(1.5,0.8,7.)),
            ("rotated",Theme::Dark,None,Vec3::new(-3.,2.,5.)),
            ("light",Theme::Light,None,Vec3::new(1.5,0.8,7.)),
        ] {
            let view = View { eye, target: Vec3::new(0.2,0.,0.), viewport: [0.,0.,720.,520.],
                orthographic: Some(1.65), ..View::default() };
            let overlay = BoneOverlay::new(&project,&evaluated,&view,view.viewport);
            let mut ui = Ui::with_theme(1.,theme,[0.,0.]);
            overlay.draw(&mut ui,selected,None);
            assert_eq!(overlay.bones.len(),2);
            let base = bone_color(&ui.palette(),false,false);
            assert!(ui.rects.iter().any(|r| r.triangle.is_none() && r.segment.is_none() && r.color==base));
            assert!(ui.rects.iter().filter(|r| r.triangle.is_some()).all(|r| {
                let color = if selected.is_some() && (r.color[0]/r.color[2] - ui.palette().accent[0]/ui.palette().accent[2]).abs()<0.001 {
                    ui.palette().accent
                } else { base };
                (r.color[0]/r.color[2]-color[0]/color[2]).abs()<0.001
            }));
            if let Some(directory) = std::env::var_os("ARTFORGE_QA_BONE_DIR") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                let data = serde_json::json!({
                    "width":720,"height":520,"background":ui.palette().viewport,
                    "source":"Actual BoneOverlay::new/draw output, CPU primitive export; not a GPU screenshot",
                    "rectangles":ui.rects.iter().map(|r| serde_json::json!({
                        "rect":r.rect,"color":r.color,"triangle":r.triangle,"segment":r.segment
                    })).collect::<Vec<_>>()
                });
                std::fs::write(directory.join(format!("bones-{name}.json")),serde_json::to_vec(&data).unwrap()).unwrap();
            }
        }
    }

    #[test]
    fn bones_crossing_viewport_edges_are_pickable_and_draw_inside_it() {
        for direction in [Vec3::X, Vec3::Y, Vec3::new(1., 0.7, 0.)] {
            let project = Project {
                nodes: vec![
                    joint(1, None, direction * -6.),
                    joint(2, Some(1), direction * 12.),
                ],
                ..Project::default()
            };
            let overlay = overlay(&project);
            assert_eq!(overlay.bones.len(), 1);
            assert_eq!(overlay.hit(pointer(Vec3::ZERO)), Some(1));
            assert_eq!(overlay.hit([10., 330.]), None);
            for scale in [1., 2.] {
                let mut ui = Ui::with_theme(scale, Theme::Dark, [0., 0.]);
                overlay.draw(&mut ui, None, None);
                let v = view().viewport;
                assert!(!ui.rects.is_empty());
                for primitive in &ui.rects {
                    let r = primitive.rect.map(|value| value / scale);
                    assert!(
                        r[0] >= v[0] - 0.01 && r[1] >= v[1] - 0.01,
                        "rect={r:?}, viewport={v:?}, scale={scale}"
                    );
                    assert!(
                        r[0] + r[2] <= v[0] + v[2] + 0.01 && r[1] + r[3] <= v[1] + v[3] + 0.01,
                        "rect={r:?}, viewport={v:?}, scale={scale}"
                    );
                }
            }
        }
        let viewport = view().viewport;
        let corners = [
            Vec2::new(viewport[0], viewport[1]),
            Vec2::new(viewport[0] + viewport[2], viewport[1]),
            Vec2::new(viewport[0], viewport[1] + viewport[3]),
            Vec2::new(viewport[0] + viewport[2], viewport[1] + viewport[3]),
        ];
        for scale in [1., 2.] {
            let mut ui = Ui::with_theme(scale, Theme::Dark, [0., 0.]);
            for center in corners {
                fill_circle(&mut ui, center, ACTIVE_JOINT_RADIUS, [1.; 4], viewport);
            }
            assert!(!ui.rects.is_empty());
            for primitive in &ui.rects {
                let r = primitive.rect.map(|value| value / scale);
                assert!(r[0] >= viewport[0] && r[1] >= viewport[1], "{r:?}");
                assert!(
                    r[0] + r[2] <= viewport[0] + viewport[2]
                        && r[1] + r[3] <= viewport[1] + viewport[3],
                    "{r:?}"
                );
            }
        }
    }

    #[test]
    fn depth_clipping_accepts_crossing_near_plane_and_rejects_hidden_segment() {
        let view = View {
            orthographic: None,
            ..view()
        };
        let matrix = view.view_projection();
        let crossing =
            project_segment(matrix, view.viewport, Vec3::new(-1., 0., 7.), Vec3::ZERO).unwrap();
        assert!(crossing.0.is_finite() && crossing.1.is_finite() && crossing.2.is_finite());
        assert!(project_segment(
            matrix,
            view.viewport,
            Vec3::new(0., 0., 7.),
            Vec3::new(0., 0., 8.)
        )
        .is_none());
    }
}
