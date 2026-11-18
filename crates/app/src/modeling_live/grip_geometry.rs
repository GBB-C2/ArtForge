//! Stable geometry for the lifetime of a frozen-source preview.
use super::*;
use glam::Vec2;
use poseforge_core::MeshConnectivity;

pub(super) struct SurfaceFrame {
    pub corner: Vec3,
    pub previous: Vec3,
    pub next: Vec3,
    pub miter: Vec3,
    pub inward: Vec3,
    pub opposite: Vec3,
}
pub(super) struct Seam {
    pub source: Vec3,
    pub targets: Vec<Vec3>,
}
impl LiveOperation {
    pub(super) fn with_spacing(&self, axis: Vec3, distance: f32) -> [f32; 4] {
        let mut values = self.values;
        let offset = axis * distance.clamp(0., 10000. / axis.abs().max_element());
        values[1..].copy_from_slice(
            &offset
                .clamp(Vec3::splat(-10000.), Vec3::splat(10000.))
                .to_array(),
        );
        values
    }
    pub(super) fn offset(&self) -> Vec3 {
        Vec3::new(self.values[1], self.values[2], self.values[3])
    }
}

fn surface_frame(op: &LiveOperation, g: &MeshConnectivity) -> Option<SurfaceFrame> {
    let mesh = &op.original;
    let edge_key = |a: usize, b: usize| (a.min(b), a.max(b));
    let edge = op
        .selection
        .edges
        .first()
        .and_then(|&(a, b)| Some(edge_key(*g.canonical.get(a)?, *g.canonical.get(b)?)));
    let vertex = op
        .vertices
        .first()
        .and_then(|&v| g.canonical.get(v))
        .copied();
    let point = |i: usize| Vec3::from_array(mesh.vertices[i].position);
    let is_boundary = |a, b| {
        g.edge_faces
            .get(&edge_key(a, b))
            .is_some_and(|fs| fs.iter().filter(|f| op.faces.contains(f)).count() == 1)
    };
    let mut choice = None;
    let face_indices: Box<dyn Iterator<Item = usize> + '_> = match op.kind {
        Kind::Vertex | Kind::VertexInset => {
            let v = vertex?;
            Box::new(
                g.vertex_neighbors[v]
                    .iter()
                    .filter_map(move |&n| g.edge_faces.get(&edge_key(v, n)))
                    .flatten()
                    .copied(),
            )
        }
        Kind::Edge => Box::new(g.edge_faces.get(&edge?)?.iter().copied()),
        _ => Box::new(op.faces.iter().copied()),
    };
    'faces: for f in face_indices {
        let face = mesh.face_vertices(f)?;
        for (i, &id) in face.iter().enumerate() {
            let b = g.canonical[id as usize];
            let a = g.canonical[face[(i + face.len() - 1) % face.len()] as usize];
            let c = g.canonical[face[(i + 1) % face.len()] as usize];
            if a == b || b == c {
                continue;
            }
            let priority = match op.kind {
                Kind::Edge if Some(edge_key(b, c)) == edge => 2,
                Kind::Vertex | Kind::VertexInset if Some(b) == vertex => 2,
                Kind::Face | Kind::Inset if op.face_mode == 2 => 2,
                Kind::Face | Kind::Inset => {
                    u8::from(is_boundary(b, c)) * (1 + u8::from(is_boundary(a, b)))
                }
                _ => continue,
            };
            if choice.is_none_or(|(score, ..): (u8, usize, usize, usize, usize)| priority > score) {
                choice = Some((priority, f, a, b, c));
            }
            if priority == 2 {
                break 'faces;
            }
        }
    }
    let (_, f, a, b, c) = choice?;
    let normal = |f: usize| {
        let face = mesh.face_vertices(f)?;
        let pivot = point(face[0] as usize);
        (1..face.len() - 1)
            .map(|i| (point(face[i] as usize) - pivot).cross(point(face[i + 1] as usize) - pivot))
            .sum::<Vec3>()
            .try_normalize()
    };
    let at = point(b);
    let previous = point(a) - at;
    let next = point(c) - at;
    let n = normal(f)?;
    let inside = n.cross(next.try_normalize()?);
    let other = n.cross(-previous.try_normalize()?);
    // Intersect the two offset lines. Both sides stay the same distance from the border.
    let miter = (inside + other) / (1. + inside.dot(other)).max(1e-4);
    let opposite = g
        .edge_faces
        .get(&edge_key(b, c))
        .into_iter()
        .flatten()
        .copied()
        .find(|&other| other != f)
        .and_then(normal)
        .map(|other| -other.cross(next.normalize()))
        .unwrap_or(n);
    let m = op.model;
    Some(SurfaceFrame {
        corner: m.transform_point3(at),
        previous: m.transform_vector3(previous),
        next: m.transform_vector3(next),
        miter: m.transform_vector3(miter),
        inward: m.transform_vector3(inside),
        opposite: m.transform_vector3(opposite),
    })
}

impl App {
    pub(super) fn refresh_grip_geometry(&self, op: &mut LiveOperation) {
        if matches!(
            op.kind,
            Kind::Face | Kind::Edge | Kind::Inset | Kind::Vertex | Kind::VertexInset
        ) {
            op.surface = surface_frame(op, &self.mesh_cache(&op.original).graph);
        }
        if op.kind == Kind::Bridge {
            let to_world = |i: usize| {
                op.model
                    .transform_point3(Vec3::from_array(op.original.vertices[i].position))
            };
            op.seam = poseforge_core::bridge_seam(&op.original, &op.faces, &op.selection.edges)
                .map(|(first, rest)| Seam {
                    source: to_world(first),
                    targets: rest.into_iter().map(to_world).collect(),
                });
        }
    }

    pub(super) fn projected_rim(&self, op: &LiveOperation) -> Option<Vec<Vec2>> {
        let matrix = self.view().view_projection();
        let viewport = self.viewport();
        op.seam
            .as_ref()?
            .targets
            .iter()
            .map(|&p| {
                Self::project_with_matrix(matrix, viewport, p)
                    .map(Vec2::from_array)
                    .filter(|p| p.is_finite())
            })
            .collect()
    }
}

pub(super) fn rim_coordinate(p: Vec2, rim: &[Vec2]) -> f32 {
    (0..rim.len())
        .map(|i| {
            let a = rim[i];
            let d = rim[(i + 1) % rim.len()] - a;
            let t = ((p - a).dot(d) / d.length_squared().max(1e-6)).clamp(0., 1.);
            (p.distance_squared(a + t * d), i as f32 + t)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(0., |(_, t)| t)
}

pub(super) fn rim_draggable(rim: &[Vec2]) -> bool {
    let mut area = 0.;
    let mut perimeter = 0.;
    for (i, &a) in rim.iter().enumerate() {
        let b = rim[(i + 1) % rim.len()];
        area += (a - rim[0]).perp_dot(b - rim[0]);
        perimeter += a.distance(b);
    }
    perimeter > 24. && area.abs() > 4. * perimeter
}
