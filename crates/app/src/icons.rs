//! Small application-owned vector symbols. They share the UI color tokens and
//! logical coordinate system, and never depend on platform icon fonts or emoji.
use crate::ui::Ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    SelectPointer, SelectBox, SelectLasso, GrowSelection, ShrinkSelection,
    Connected, InvertSelection, EdgeLoop, EdgeRing,
    Extrude, Inset, Bevel, LoopCut, Knife, SlideEdge,
    Merge, Weld, Bridge, Dissolve, Flatten, Straighten, EqualSpace, Relax,
    ModelSphere, ModelCircle, Inflate, Twist, Subdivide, InspectMesh, SmoothNormals, FlatNormals, DeleteFaces,
    Physics,
    Object,
    Text,
    Empty,
    Force,Wind,Vortex,Magnet,Cylinder,Cone,Torus,Capsule,Pyramid,
    Fx,Fire,Smoke,Fog,
    Scene,
    Vertex,
    Edge,
    Face,
    Plane,
    Sphere,
    Cloth,
    Fox,
    ChevronRight,
    ChevronDown,
    Sun,
    Spot,
    Area,
    Lock,
    Material,
    MaterialAssign,
    MaterialUnassign,
    Render,
    Fluid,
    File,
    Folder,
    FolderAdd,
    Import,
    Save,
    Export,
    Sequence,
    Undo,
    Redo,
    Orbit,
    View,
    Perspective,
    Orthographic,
    Hand,
    Fracture,
    AxisSpace,
    Curve,
    CurveCircle,
    Library,
    Move,
    Rotate,
    Reset,
    Scale,
    Play,
    Pause,
    Start,
    Previous,
    Next,
    Key,
    Copy,
    Paste,
    Duplicate,
    Delete,
    Light,
    Eye,
    Grid,
    Bone,
    RigBind,
    Eyedropper,
    Pose,
    Normals,
    Uv,
    UvIslands,
    Settings,
    Camera,
    Search,
    Filter,
    Close,
    Help,
    Frame,
    Check,
}

#[derive(Default)]
struct Geometry {
    lines: Vec<([f32; 2], [f32; 2])>,
    fills: Vec<Vec<[f32; 2]>>,
}
impl Geometry {
    fn path(&mut self, points: &[[f32; 2]]) {
        self.lines.extend(points.windows(2).map(|p| (p[0], p[1])));
    }
    fn closed(&mut self, points: &[[f32; 2]]) {
        self.path(points);
        if let (Some(first), Some(last)) = (points.first(), points.last()) {
            self.lines.push((*last, *first));
        }
    }
    fn fill(&mut self, points: &[[f32; 2]]) {
        self.fills.push(points.to_vec());
    }
    fn arc(&mut self, center: [f32; 2], radii: [f32; 2], start: f32, end: f32) {
        let steps = ((end - start).abs() / 12.).ceil().clamp(1., 32.) as usize;
        let points: Vec<_> = (0..=steps)
            .map(|i| {
                let angle = (start + (end - start) * i as f32 / steps as f32).to_radians();
                [
                    center[0] + radii[0] * angle.cos(),
                    center[1] + radii[1] * angle.sin(),
                ]
            })
            .collect();
        self.path(&points);
    }
    fn circle(&mut self, center: [f32; 2], radius: f32) {
        self.arc(center, [radius; 2], 0., 360.);
    }
    fn dot(&mut self, center: [f32; 2], radius: f32) {
        let points: Vec<_> = (0..16)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / 16.;
                [
                    center[0] + radius * angle.cos(),
                    center[1] + radius * angle.sin(),
                ]
            })
            .collect();
        self.fill(&points);
    }
}

impl Icon {
    /// Draw inside the supplied logical rectangle. The square is centered, and
    /// coverage is cropped to these bounds before joining the caller's UI.
    pub fn draw(self, ui: &mut Ui, rect: [f32; 4], color: [f32; 4]) {
        if rect.iter().any(|v| !v.is_finite())
            || color.iter().any(|v| !v.is_finite())
            || !ui.scale.is_finite()
            || ui.scale <= 0.
            || color[3] <= 0.
        {
            return;
        }
        let size = rect[2].min(rect[3]);
        if !(4.0..=128.0).contains(&size) {
            return;
        }
        let unit = size / 24.;
        let origin = [
            rect[0] + (rect[2] - size) * 0.5,
            rect[1] + (rect[3] - size) * 0.5,
        ];
        let point = |p: [f32; 2]| [origin[0] + p[0] * unit, origin[1] + p[1] * unit];
        let mut geometry = Geometry::default();
        match self {
            Self::MaterialAssign | Self::MaterialUnassign => {
                geometry.closed(&[[3.,4.],[15.,4.],[15.,16.],[3.,16.]]);
                geometry.path(&[[9.,4.],[9.,16.]]);
                geometry.path(&[[3.,10.],[15.,10.]]);
                if self == Self::MaterialAssign {
                    geometry.path(&[[12.,19.],[15.,22.],[22.,14.]]);
                } else {
                    geometry.path(&[[17.,15.],[23.,21.]]);
                    geometry.path(&[[23.,15.],[17.,21.]]);
                }
            }
            Self::Fx=>{geometry.path(&[[12.,2.],[14.5,9.5],[22.,12.],[14.5,14.5],[12.,22.],[9.5,14.5],[2.,12.],[9.5,9.5],[12.,2.]]);}
            Self::Fire=>{geometry.path(&[[12.,2.],[13.,8.],[17.,5.],[20.,11.],[20.,16.],[17.,21.],[7.,21.],[4.,16.],[5.,11.],[8.,8.],[12.,2.]]);geometry.path(&[[12.,12.],[15.,17.],[12.,21.],[9.,17.],[12.,12.]]);}
            Self::Smoke=>{geometry.path(&[[6.,17.],[3.,15.],[3.,11.],[6.,8.],[9.,8.],[11.,4.],[16.,4.],[19.,8.],[19.,10.],[22.,12.],[22.,15.],[19.,17.],[6.,17.]]);geometry.path(&[[8.,20.],[17.,20.]]);}
            Self::Fog=>{for (y,a,b) in [(5.,5.,19.),(10.,2.,22.),(15.,4.,20.),(20.,7.,17.)] {geometry.path(&[[a,y],[b,y]]);}}
            Self::Force => {geometry.circle([12.,12.],3.);for a in [0_f32,90.,180.,270.] {let (s,c)=a.to_radians().sin_cos();geometry.path(&[[12.+c*5.,12.+s*5.],[12.+c*10.,12.+s*10.]]);geometry.path(&[[12.+c*7.-s*2.,12.+s*7.+c*2.],[12.+c*10.,12.+s*10.],[12.+c*7.+s*2.,12.+s*7.-c*2.]]);}}
            Self::Wind=>{for y in [6.,12.,18.] {geometry.path(&[[3.,y],[20.,y],[17.,y-2.]]);}}
            Self::Vortex=>{let p:Vec<_>=(0..=64).map(|i|{let a=i as f32*0.18;let r=1.+i as f32*0.13;[12.+r*a.cos(),12.+r*a.sin()]}).collect();geometry.path(&p);}
            Self::Magnet=>{geometry.path(&[[4.,4.],[4.,14.],[7.,20.],[17.,20.],[20.,14.],[20.,4.],[15.,4.],[15.,13.],[13.,15.],[11.,15.],[9.,13.],[9.,4.],[4.,4.]]);geometry.path(&[[4.,8.],[9.,8.]]);geometry.path(&[[15.,8.],[20.,8.]]);}
            Self::Cylinder=>{geometry.arc([12.,5.],[8.,3.],0.,360.);geometry.arc([12.,19.],[8.,3.],0.,180.);geometry.path(&[[4.,5.],[4.,19.]]);geometry.path(&[[20.,5.],[20.,19.]]);}
            Self::Cone=>{geometry.arc([12.,19.],[8.,3.],0.,360.);geometry.path(&[[4.,19.],[12.,2.],[20.,19.]]);}
            Self::Torus=>{geometry.arc([12.,12.],[10.,7.],0.,360.);geometry.arc([12.,11.],[5.,3.],0.,360.);}
            Self::Capsule=>{geometry.arc([12.,7.],[6.,5.],180.,360.);geometry.arc([12.,17.],[6.,5.],0.,180.);geometry.path(&[[6.,7.],[6.,17.]]);geometry.path(&[[18.,7.],[18.,17.]]);}
            Self::Pyramid=>{geometry.closed(&[[3.,18.],[12.,3.],[22.,18.],[13.,22.]]);geometry.path(&[[12.,3.],[13.,22.]]);geometry.path(&[[3.,18.],[13.,14.],[22.,18.]]);}
            Self::Text => {geometry.path(&[[4.,7.],[4.,4.],[20.,4.],[20.,7.]]);geometry.path(&[[12.,4.],[12.,20.]]);geometry.path(&[[8.,20.],[16.,20.]]);}
            Self::Empty => {geometry.path(&[[4.,18.],[20.,6.]]);geometry.path(&[[4.,6.],[20.,18.]]);geometry.path(&[[12.,2.],[12.,22.]]);geometry.path(&[[9.,3.],[12.,2.],[15.,3.]]);}
            Self::SelectPointer => {
                geometry.closed(&[[5., 3.], [19., 13.], [12., 14.], [9., 21.]]);
            }
            Self::SelectBox => {
                for (x, y, dx, dy) in [(4., 4., 1., 1.), (20., 4., -1., 1.), (4., 20., 1., -1.), (20., 20., -1., -1.)] {
                    geometry.path(&[[x, y + 5. * dy], [x, y], [x + 5. * dx, y]]);
                }
                geometry.dot([12., 12.], 2.);
            }
            Self::SelectLasso => {
                geometry.arc([12., 10.], [9., 6.], 0., 360.);
                geometry.path(&[[6., 14.], [8., 20.], [13., 22.], [16., 19.]]);
            }
            Self::GrowSelection | Self::ShrinkSelection => {
                geometry.closed(&[[4., 4.], [16., 4.], [16., 16.], [4., 16.]]);
                geometry.path(&[[12., 19.], [22., 19.]]);
                if self == Self::GrowSelection { geometry.path(&[[17., 14.], [17., 24.]]); }
            }
            Self::Connected => {
                for p in [[5., 5.], [19., 5.], [12., 19.]] { geometry.circle(p, 3.); }
                geometry.path(&[[8., 5.], [16., 5.]]);
                geometry.path(&[[6., 8.], [11., 16.]]);
                geometry.path(&[[18., 8.], [13., 16.]]);
            }
            Self::InvertSelection => {
                geometry.closed(&[[4., 4.], [20., 4.], [20., 20.], [4., 20.]]);
                geometry.fill(&[[4., 4.], [20., 4.], [4., 20.]]);
            }
            Self::EdgeLoop | Self::EdgeRing => {
                geometry.closed(&[[4., 5.], [20., 5.], [20., 19.], [4., 19.]]);
                geometry.path(&[[4., 12.], [20., 12.]]);
                if self == Self::EdgeLoop { geometry.closed(&[[7., 8.], [17., 8.], [17., 16.], [7., 16.]]); }
                else if self == Self::EdgeRing {
                    for x in [8., 12., 16.] { geometry.path(&[[x, 3.], [x, 21.]]); }
                } else { geometry.path(&[[12., 2.], [12., 22.]]); }
            }
            Self::Extrude => {
                geometry.path(&[[4., 16.], [4., 21.], [16., 21.], [20., 17.], [20., 12.]]);
                geometry.path(&[[16., 16.], [16., 21.]]);
                geometry.path(&[[12., 9.], [12., 2.]]);
                geometry.path(&[[9., 5.], [12., 2.], [15., 5.]]);
                geometry.closed(&[[4., 12.], [8., 8.], [20., 8.], [16., 12.]]);
                geometry.closed(&[[4., 12.], [16., 12.], [16., 16.], [4., 16.]]);
            }
            Self::Inset => {
                geometry.path(&[[3., 4.], [8., 8.]]);
                geometry.path(&[[21., 4.], [16., 8.]]);
                geometry.path(&[[21., 20.], [16., 16.]]);
                geometry.path(&[[3., 20.], [8., 16.]]);
                geometry.closed(&[[3., 4.], [21., 4.], [21., 20.], [3., 20.]]);
                geometry.closed(&[[8., 8.], [16., 8.], [16., 16.], [8., 16.]]);
            }
            Self::Bevel => {
                geometry.path(&[[3., 9.], [14., 9.], [20., 3.]]);
                geometry.path(&[[14., 9.], [14., 20.]]);
                geometry.path(&[[5., 7.], [16., 7.], [18., 5.]]);
                geometry.closed(&[[3., 9.], [9., 3.], [20., 3.], [20., 20.], [3., 20.]]);
            }
            Self::LoopCut => {
                geometry.path(&[[16., 7.], [16., 21.]]);
                geometry.path(&[[3., 14.], [16., 14.], [21., 10.]]);
                geometry.closed(&[[3., 7.], [16., 7.], [21., 3.], [8., 3.]]);
                geometry.closed(&[[3., 7.], [3., 21.], [16., 21.], [21., 17.], [21., 3.]]);
                geometry.dot([3., 14.], 1.1);
                geometry.dot([16., 14.], 1.1);
                geometry.dot([21., 10.], 1.1);
            }
            Self::Knife => {
                geometry.path(&[[7., 11.], [13., 17.]]);
                geometry.path(&[[17., 5.], [19., 7.]]);
                geometry.path(&[[3., 21.], [9., 18.]]);
                geometry.closed(&[[3., 21.], [7., 11.], [16., 2.], [22., 8.], [13., 17.]]);
            }
            Self::SlideEdge => {
                geometry.path(&[[3., 4.], [3., 20.]]);
                geometry.path(&[[21., 4.], [21., 20.]]);
                geometry.path(&[[12., 4.], [12., 20.]]);
                geometry.path(&[[5., 12.], [19., 12.]]);
                geometry.path(&[[8., 9.], [5., 12.], [8., 15.]]);
                geometry.path(&[[16., 9.], [19., 12.], [16., 15.]]);
                geometry.dot([12., 4.], 1.4);
                geometry.dot([12., 20.], 1.4);
            }
            Self::Merge => {
                geometry.path(&[[5., 5.], [9., 9.]]);
                geometry.path(&[[19., 5.], [15., 9.]]);
                geometry.path(&[[5., 19.], [9., 15.]]);
                geometry.path(&[[19., 19.], [15., 15.]]);
                geometry.path(&[[6., 9.], [9., 9.], [9., 6.]]);
                geometry.path(&[[15., 6.], [15., 9.], [18., 9.]]);
                geometry.path(&[[6., 15.], [9., 15.], [9., 18.]]);
                geometry.path(&[[15., 18.], [15., 15.], [18., 15.]]);
                geometry.dot([3., 3.], 1.5);
                geometry.dot([21., 3.], 1.5);
                geometry.dot([3., 21.], 1.5);
                geometry.dot([21., 21.], 1.5);
                geometry.dot([12., 12.], 2.);
            }
            Self::Weld => {
                geometry.path(&[[3., 5.], [9., 10.], [15., 10.], [21., 5.]]);
                geometry.path(&[[3., 19.], [9., 14.], [15., 14.], [21., 19.]]);
                geometry.path(&[[12., 3.], [12., 6.]]);
                geometry.path(&[[12., 18.], [12., 21.]]);
                geometry.circle([12., 12.], 3.);
                geometry.dot([3., 5.], 1.6);
                geometry.dot([21., 5.], 1.6);
                geometry.dot([3., 19.], 1.6);
                geometry.dot([21., 19.], 1.6);
            }
            Self::Bridge => {
                geometry.path(&[[3., 4.], [3., 20.]]);
                geometry.path(&[[21., 4.], [21., 20.]]);
                geometry.path(&[[3., 6.], [8., 8.], [16., 8.], [21., 6.]]);
                geometry.path(&[[3., 12.], [21., 12.]]);
                geometry.path(&[[3., 18.], [8., 16.], [16., 16.], [21., 18.]]);
                geometry.path(&[[8., 8.], [8., 16.]]);
                geometry.path(&[[16., 8.], [16., 16.]]);
            }
            Self::Dissolve => {
                geometry.path(&[[12., 4.], [12., 7.]]);
                geometry.path(&[[12., 10.], [12., 14.]]);
                geometry.path(&[[12., 17.], [12., 20.]]);
                geometry.path(&[[6., 12.], [9., 12.]]);
                geometry.path(&[[15., 12.], [18., 12.]]);
                geometry.closed(&[[3., 4.], [21., 4.], [21., 20.], [3., 20.]]);
            }
            Self::Flatten => {
                geometry.path(&[[2., 20.], [22., 20.]]);
                geometry.path(&[[5., 8.], [5., 17.]]);
                geometry.path(&[[3., 15.], [5., 17.], [7., 15.]]);
                geometry.path(&[[12., 11.], [12., 17.]]);
                geometry.path(&[[10., 15.], [12., 17.], [14., 15.]]);
                geometry.path(&[[19., 7.], [19., 17.]]);
                geometry.path(&[[17., 15.], [19., 17.], [21., 15.]]);
                geometry.dot([5., 5.], 1.7);
                geometry.dot([12., 8.], 1.7);
                geometry.dot([19., 4.], 1.7);
            }
            Self::Straighten => {
                geometry.path(&[[3., 7.], [8., 3.], [15., 9.], [21., 4.]]);
                geometry.path(&[[3., 19.], [21., 19.]]);
                geometry.path(&[[12., 11.], [12., 15.]]);
                geometry.path(&[[10., 13.], [12., 15.], [14., 13.]]);
                geometry.dot([3., 7.], 1.4);
                geometry.dot([8., 3.], 1.4);
                geometry.dot([15., 9.], 1.4);
                geometry.dot([21., 4.], 1.4);
                geometry.dot([3., 19.], 1.4);
                geometry.dot([12., 19.], 1.4);
                geometry.dot([21., 19.], 1.4);
            }
            Self::EqualSpace => {
                geometry.path(&[[3., 12.], [21., 12.]]);
                geometry.path(&[[3., 5.], [21., 5.]]);
                geometry.path(&[[5., 3.], [3., 5.], [5., 7.]]);
                geometry.path(&[[19., 3.], [21., 5.], [19., 7.]]);
                geometry.path(&[[3., 16.], [3., 21.]]);
                geometry.path(&[[9., 16.], [9., 21.]]);
                geometry.path(&[[15., 16.], [15., 21.]]);
                geometry.path(&[[21., 16.], [21., 21.]]);
                geometry.dot([3., 12.], 1.7);
                geometry.dot([9., 12.], 1.7);
                geometry.dot([15., 12.], 1.7);
                geometry.dot([21., 12.], 1.7);
            }
            Self::Relax => {
                geometry.path(&[[3., 6.], [10., 3.], [20., 7.]]);
                geometry.path(&[[3., 6.], [7., 13.], [4., 21.]]);
                geometry.path(&[[10., 3.], [12., 12.], [12., 21.]]);
                geometry.path(&[[20., 7.], [19., 14.], [21., 21.]]);
                geometry.path(&[[7., 13.], [12., 12.], [19., 14.]]);
                geometry.path(&[[4., 21.], [12., 21.], [21., 21.]]);
                geometry.dot([12., 12.], 2.);
                geometry.arc([12., 12.], [5., 5.], 205., 335.);
            }
            Self::ModelSphere => {
                geometry.circle([12., 12.], 9.);
                geometry.arc([12., 12.], [4., 9.], 0., 360.);
                geometry.arc([12., 12.], [9., 3.], 0., 360.);
            }
            Self::ModelCircle => {
                geometry.circle([12., 12.], 8.);
                geometry.dot([12., 4.], 1.6);
                geometry.dot([20., 12.], 1.6);
                geometry.dot([12., 20.], 1.6);
                geometry.dot([4., 12.], 1.6);
            }
            Self::Inflate => {
                geometry.path(&[[12., 12.], [12., 2.]]);
                geometry.path(&[[9., 5.], [12., 2.], [15., 5.]]);
                geometry.path(&[[6., 16.], [2., 9.]]);
                geometry.path(&[[2., 13.], [2., 9.], [6., 10.]]);
                geometry.path(&[[18., 16.], [22., 9.]]);
                geometry.path(&[[18., 10.], [22., 9.], [22., 13.]]);
                geometry.arc([12., 22.], [9., 9.], 200., 340.);
            }
            Self::Twist => {
                geometry.path(&[[6., 6.], [16., 12.], [6., 19.]]);
                geometry.path(&[[18., 6.], [8., 12.], [18., 19.]]);
                geometry.fill(&[[20., 10.], [22., 16.], [17., 14.]]);
                geometry.arc([12., 5.], [7., 3.], 0., 360.);
                geometry.arc([12., 20.], [7., 2.5], 0., 360.);
                geometry.arc([12., 12.], [9., 5.], 280., 390.);
            }
            Self::Subdivide => {
                geometry.path(&[[9., 3.], [9., 21.]]);
                geometry.path(&[[15., 3.], [15., 21.]]);
                geometry.path(&[[3., 9.], [21., 9.]]);
                geometry.path(&[[3., 15.], [21., 15.]]);
                geometry.closed(&[[3., 3.], [21., 3.], [21., 21.], [3., 21.]]);
            }
            Self::InspectMesh => {
                geometry.path(&[[9., 3.], [9., 11.]]);
                geometry.path(&[[3., 9.], [11., 9.]]);
                geometry.path(&[[18., 18.], [22., 22.]]);
                geometry.closed(&[[3., 3.], [16., 3.], [16., 16.], [3., 16.]]);
                geometry.circle([15., 15.], 5.);
            }
            Self::SmoothNormals => {
                geometry.arc([12., 18.], [9., 6.], 180., 360.);
                geometry.arc([12., 18.], [4., 6.], 180., 360.);
                geometry.path(&[[3., 18.], [21., 18.]]);
                geometry.path(&[[12., 9.], [12., 2.]]);
                geometry.path(&[[9., 5.], [12., 2.], [15., 5.]]);
            }
            Self::FlatNormals => {
                geometry.path(&[[2., 20.], [2., 14.], [10., 14.], [10., 9.], [22., 9.]]);
                geometry.path(&[[6., 13.], [6., 5.]]);
                geometry.path(&[[3., 8.], [6., 5.], [9., 8.]]);
                geometry.path(&[[18., 8.], [18., 2.]]);
                geometry.path(&[[15., 5.], [18., 2.], [21., 5.]]);
            }
            Self::DeleteFaces => {
                geometry.path(&[[12., 3.], [12., 21.]]);
                geometry.path(&[[3., 12.], [21., 12.]]);
                geometry.path(&[[15., 15.], [19., 19.]]);
                geometry.path(&[[19., 15.], [15., 19.]]);
                geometry.closed(&[[3., 3.], [21., 3.], [21., 21.], [3., 21.]]);
            }
            Self::Library => {
                for x in [4., 10., 16.] { geometry.closed(&[[x, 4.], [x + 4., 4.], [x + 4., 20.], [x, 20.]]); }
                geometry.path(&[[2., 21.], [22., 21.]]);
            }
            Self::AxisSpace => {
                geometry.path(&[[5., 4.], [5., 19.], [20., 19.]]);
                geometry.path(&[[2., 7.], [5., 4.], [8., 7.]]);
                geometry.path(&[[17., 16.], [20., 19.], [17., 22.]]);
                geometry.path(&[[5., 19.], [17., 7.], [13., 7.]]);
            }
            Self::Curve => {
                let points: Vec<_> = (0..=24).map(|i| {
                    let t = i as f32 / 24.; let u = 1. - t;
                    [3. + 18. * t, 18. * u*u*u + 3.*3.*u*u*t + 21.*3.*u*t*t + 6.*t*t*t]
                }).collect();
                geometry.path(&points);
                geometry.path(&[[3., 18.], [3., 4.]]); geometry.path(&[[21., 6.], [21., 20.]]);
                for p in [[3., 18.], [21., 6.]] { geometry.circle(p, 2.); }
            }
            Self::CurveCircle => { geometry.circle([12., 12.], 8.); geometry.closed(&[[18., 10.], [22., 10.], [22., 14.], [18., 14.]]); }
            Self::ChevronRight => geometry.path(&[[9., 6.], [15., 12.], [9., 18.]]),
            Self::ChevronDown => geometry.path(&[[6., 9.], [12., 15.], [18., 9.]]),
            Self::Fox => {
                geometry.closed(&[
                    [3., 3.],
                    [9., 7.],
                    [15., 7.],
                    [21., 3.],
                    [20., 15.],
                    [12., 22.],
                    [4., 15.],
                ]);
                geometry.path(&[[5., 12.], [12., 19.], [19., 12.]]);
                geometry.dot([8., 12.], 1.);
                geometry.dot([16., 12.], 1.);
            }
            Self::Scene => {
                geometry.closed(&[
                    [3., 9.],
                    [12., 5.],
                    [21., 9.],
                    [21., 18.],
                    [12., 22.],
                    [3., 18.],
                ]);
                geometry.path(&[[3., 9.], [12., 13.], [21., 9.]]);
                geometry.path(&[[12., 13.], [12., 22.]]);
                geometry.path(&[[2., 2.], [6., 2.], [6., 6.]]);
                geometry.path(&[[18., 2.], [22., 2.], [22., 6.]]);
            }
            Self::Vertex | Self::Edge | Self::Face => {
                let corners = [[4., 5.], [20., 5.], [20., 20.], [4., 20.]];
                geometry.closed(&corners);
                match self {
                    Self::Vertex => {
                        for corner in corners {
                            geometry.dot(corner, 2.5);
                        }
                    }
                    Self::Edge => {
                        geometry.fill(&[[2.5, 3.5], [21.5, 3.5], [21.5, 7.], [2.5, 7.]]);
                        geometry.dot(corners[0], 2.5);
                        geometry.dot(corners[1], 2.5);
                    }
                    _ => geometry.fill(&[[7., 8.], [17., 8.], [17., 17.], [7., 17.]]),
                }
            }
            Self::Plane => {
                geometry.closed(&[[2., 15.], [9., 5.], [22., 9.], [15., 19.]]);
                geometry.path(&[[5.5, 10.], [18.5, 14.]]);
                geometry.path(&[[8.5, 17.], [15.5, 7.]]);
            }
            Self::Sphere => {
                geometry.circle([12., 12.], 9.);
                geometry.arc([12., 12.], [4., 9.], 0., 360.);
                geometry.arc([12., 12.], [9., 3.], 0., 360.);
            }
            Self::Cloth => {
                geometry.closed(&[
                    [3., 3.],
                    [10., 6.],
                    [21., 3.],
                    [20., 19.],
                    [12., 22.],
                    [4., 18.],
                ]);
                geometry.path(&[[8., 5.], [8., 12.], [9., 20.]]);
                geometry.path(&[[16., 5.], [14., 13.], [16., 21.]]);
                geometry.path(&[[4., 11.], [12., 14.], [20., 10.]]);
            }
            Self::Sun => {
                geometry.circle([12., 12.], 4.);
                for i in 0..8 {
                    let angle = i as f32 * std::f32::consts::FRAC_PI_4;
                    let d = [angle.cos(), angle.sin()];
                    geometry.path(&[
                        [12. + d[0] * 7., 12. + d[1] * 7.],
                        [12. + d[0] * 10., 12. + d[1] * 10.],
                    ]);
                }
            }
            Self::Spot => {
                geometry.closed(&[[7., 3.], [17., 3.], [15., 8.], [9., 8.]]);
                geometry.path(&[[9., 8.], [3., 21.], [21., 21.], [15., 8.]]);
                geometry.path(&[[12., 11.], [12., 17.]]);
            }
            Self::Area => {
                geometry.closed(&[[3., 3.], [21., 3.], [21., 11.], [3., 11.]]);
                for x in [6., 12., 18.] {
                    geometry.path(&[[x, 15.], [x, 21.]]);
                    geometry.path(&[[x - 2., 19.], [x, 21.], [x + 2., 19.]]);
                }
            }
            Self::Object => {
                geometry.closed(&[
                    [12., 2.],
                    [21., 7.],
                    [21., 17.],
                    [12., 22.],
                    [3., 17.],
                    [3., 7.],
                ]);
                geometry.path(&[[3., 7.], [12., 12.], [21., 7.]]);
                geometry.path(&[[12., 12.], [12., 22.]]);
                geometry.path(&[[12., 2.], [12., 12.]]);
            }
            Self::Material => {
                geometry.circle([12., 12.], 9.);
                geometry.arc([12., 12.], [4., 9.], -90., 90.);
                geometry.arc([12., 12.], [9., 3.], 0., 180.);
                geometry.dot([8., 7.], 1.5);
            }
            Self::Render => {
                geometry.closed(&[[3., 9.], [21., 9.], [21., 21.], [3., 21.]]);
                geometry.closed(&[[2., 5.], [20., 2.], [21., 6.], [3., 9.]]);
                geometry.path(&[[7., 4.], [10., 8.]]);
                geometry.path(&[[14., 3.], [17., 7.]]);
                geometry.fill(&[[10., 12.], [16., 15.], [10., 18.]]);
            }
            Self::Fluid => {
                geometry.closed(&[
                    [12., 2.],
                    [6., 10.],
                    [4., 14.],
                    [5., 18.],
                    [8., 21.],
                    [12., 22.],
                    [16., 21.],
                    [19., 18.],
                    [20., 14.],
                    [18., 10.],
                ]);
                geometry.arc([12., 14.], [5., 5.], 35., 140.);
            }
            Self::File => {
                geometry.closed(&[[5., 2.], [14., 2.], [20., 8.], [20., 22.], [5., 22.]]);
                geometry.path(&[[14., 2.], [14., 8.], [20., 8.]]);
                geometry.path(&[[8., 12.], [16., 12.]]);
                geometry.path(&[[8., 16.], [16., 16.]]);
            }
            Self::Folder | Self::FolderAdd => {
                geometry.closed(&[
                    [2., 20.],
                    [2., 5.],
                    [9., 5.],
                    [12., 8.],
                    [22., 8.],
                    [22., 20.],
                ]);
                if self == Self::FolderAdd {
                    geometry.path(&[[8., 14.], [16., 14.]]);
                    geometry.path(&[[12., 10.], [12., 18.]]);
                }
            }
            Self::Import => {
                geometry.path(&[[4., 15.], [4., 21.], [20., 21.], [20., 15.]]);
                geometry.path(&[[12., 3.], [12., 16.]]);
                geometry.path(&[[7., 11.], [12., 16.], [17., 11.]]);
            }
            Self::Export => {
                geometry.path(&[[4., 15.], [4., 21.], [20., 21.], [20., 15.]]);
                geometry.path(&[[12., 17.], [12., 3.]]);
                geometry.path(&[[7., 8.], [12., 3.], [17., 8.]]);
            }
            Self::Save => {
                geometry.closed(&[[3., 3.], [18., 3.], [21., 6.], [21., 21.], [3., 21.]]);
                geometry.path(&[[7., 3.], [7., 9.], [16., 9.], [16., 3.]]);
                geometry.path(&[[7., 21.], [7., 15.], [17., 15.], [17., 21.]]);
            }
            Self::Sequence => {
                geometry.closed(&[[3., 3.], [21., 3.], [21., 21.], [3., 21.]]);
                geometry.path(&[[7., 3.], [7., 21.]]);
                geometry.path(&[[17., 3.], [17., 21.]]);
                for y in [8., 16.] {
                    geometry.path(&[[3., y], [7., y]]);
                    geometry.path(&[[17., y], [21., y]]);
                }
                geometry.fill(&[[10., 8.], [15., 12.], [10., 16.]]);
            }
            Self::Undo | Self::Redo => {
                let mirror = |p: [f32; 2]| {
                    if self == Self::Redo {
                        [24. - p[0], p[1]]
                    } else {
                        p
                    }
                };
                geometry.path(&[[5., 9.], [14., 9.], [18., 12.], [18., 18.]].map(mirror));
                geometry.path(&[[9., 5.], [5., 9.], [9., 13.]].map(mirror));
            }
            Self::Lock => {
                geometry.path(&[
                    [7., 11.],
                    [7., 7.],
                    [9., 4.],
                    [15., 4.],
                    [17., 7.],
                    [17., 11.],
                ]);
                geometry.path(&[[5., 11.], [19., 11.], [19., 21.], [5., 21.], [5., 11.]]);
                geometry.path(&[[12., 14.], [12., 18.]]);
            }
            Self::Reset => {
                geometry.arc([12., 12.], [7.; 2], -140., 140.);
                geometry.path(&[[5., 3.], [5., 8.], [10., 8.]]);
            }
            Self::View => {
                geometry.closed(&[[4., 7.], [12., 3.], [20., 7.], [20., 17.], [12., 21.], [4., 17.]]);
                geometry.path(&[[4., 7.], [12., 11.], [20., 7.]]);
                geometry.path(&[[12., 11.], [12., 21.]]);
            }
            Self::Perspective => {
                geometry.closed(&[[7., 5.], [17., 5.], [22., 20.], [2., 20.]]);
                geometry.path(&[[12., 5.], [12., 20.]]);
                geometry.path(&[[5., 11.], [19., 11.]]);
                geometry.path(&[[3.5, 16.], [20.5, 16.]]);
            }
            Self::Orthographic => {
                geometry.closed(&[[4., 4.], [20., 4.], [20., 20.], [4., 20.]]);
                geometry.path(&[[12., 4.], [12., 20.]]);
                geometry.path(&[[4., 12.], [20., 12.]]);
            }
            Self::Hand => {
                geometry.closed(&[[7., 13.], [7., 5.], [9., 4.], [10., 5.], [10., 11.],
                    [10., 3.], [12., 2.], [13., 3.], [13., 11.], [13., 4.], [15., 3.], [16., 4.],
                    [16., 12.], [16., 7.], [18., 6.], [19., 7.], [19., 15.], [17., 21.], [9., 21.],
                    [3., 13.], [3., 11.], [5., 11.]]);
            }
            Self::Fracture => {
                geometry.path(&[[10., 3.], [3., 6.], [3., 19.], [10., 22.], [9., 16.], [12., 11.], [8., 8.], [10., 3.]]);
                geometry.path(&[[14., 2.], [21., 5.], [21., 18.], [14., 21.], [13., 16.], [16., 11.], [12., 8.], [14., 2.]]);
            }
            Self::Orbit => {
                geometry.circle([12., 12.], 6.);
                geometry.arc([12., 12.], [10., 3.5], -20., 310.);
                geometry.path(&[[16., 6.], [20., 9.], [17., 12.]]);
            }
            Self::Move => {
                geometry.path(&[[12., 2.], [12., 22.]]);
                geometry.path(&[[2., 12.], [22., 12.]]);
                for points in [
                    [[8., 6.], [12., 2.], [16., 6.]],
                    [[8., 18.], [12., 22.], [16., 18.]],
                    [[6., 8.], [2., 12.], [6., 16.]],
                    [[18., 8.], [22., 12.], [18., 16.]],
                ] {
                    geometry.path(&points);
                }
            }
            Self::Rotate => {
                geometry.arc([12., 12.], [8.; 2], -55., 225.);
                geometry.path(&[[3., 12.], [6., 18.], [11., 15.]]);
            }
            Self::Scale => {
                geometry.closed(&[[3., 15.], [9., 15.], [9., 21.], [3., 21.]]);
                geometry.path(&[[9., 15.], [21., 3.]]);
                geometry.path(&[[14., 3.], [21., 3.], [21., 10.]]);
            }
            Self::Play => geometry.fill(&[[7., 3.], [21., 12.], [7., 21.]]),
            Self::Pause => {
                geometry.fill(&[[5., 4.], [9., 4.], [9., 20.], [5., 20.]]);
                geometry.fill(&[[15., 4.], [19., 4.], [19., 20.], [15., 20.]]);
            }
            Self::Start => {
                geometry.path(&[[4., 4.], [4., 20.]]);
                geometry.fill(&[[20., 4.], [8., 12.], [20., 20.]]);
            }
            Self::Previous => geometry.path(&[[16., 4.], [7., 12.], [16., 20.]]),
            Self::Next => geometry.path(&[[8., 4.], [17., 12.], [8., 20.]]),
            Self::Key => geometry.fill(&[[12., 3.], [21., 12.], [12., 21.], [3., 12.]]),
            Self::Copy | Self::Duplicate => {
                geometry.closed(&[[8., 8.], [21., 8.], [21., 21.], [8., 21.]]);
                geometry.path(&[[5., 17.], [3., 17.], [3., 3.], [17., 3.], [17., 5.]]);
                if self == Self::Duplicate {
                    geometry.path(&[[11., 14.5], [18., 14.5]]);
                    geometry.path(&[[14.5, 11.], [14.5, 18.]]);
                }
            }
            Self::Paste => {
                geometry.path(&[
                    [8., 5.],
                    [5., 5.],
                    [5., 21.],
                    [19., 21.],
                    [19., 5.],
                    [16., 5.],
                ]);
                geometry.closed(&[[8., 3.], [16., 3.], [16., 7.], [8., 7.]]);
                geometry.path(&[[9., 12.], [15., 12.]]);
                geometry.path(&[[9., 16.], [15., 16.]]);
            }
            Self::Delete => {
                geometry.path(&[[4., 7.], [20., 7.]]);
                geometry.path(&[[6., 7.], [7., 21.], [17., 21.], [18., 7.]]);
                geometry.path(&[[9., 7.], [9., 3.], [15., 3.], [15., 7.]]);
                geometry.path(&[[10., 10.], [10., 18.]]);
                geometry.path(&[[14., 10.], [14., 18.]]);
            }
            Self::Light => {
                geometry.arc([12., 8.], [6.; 2], 30., 150.);
                geometry.arc([12., 8.], [6.; 2], 150., 390.);
                geometry.path(&[[7., 12.], [9., 16.], [15., 16.], [17., 12.]]);
                geometry.path(&[[9., 19.], [15., 19.]]);
                geometry.path(&[[10., 22.], [14., 22.]]);
            }
            Self::Eye => {
                geometry.closed(&[
                    [2., 12.],
                    [6., 7.],
                    [12., 5.],
                    [18., 7.],
                    [22., 12.],
                    [18., 17.],
                    [12., 19.],
                    [6., 17.],
                ]);
                geometry.circle([12., 12.], 3.5);
                geometry.dot([12., 12.], 1.);
            }
            Self::Grid => {
                geometry.closed(&[[3., 3.], [21., 3.], [21., 21.], [3., 21.]]);
                for i in [9., 15.] {
                    geometry.path(&[[3., i], [21., i]]);
                    geometry.path(&[[i, 3.], [i, 21.]]);
                }
            }
            Self::Normals => {
                geometry.closed(&[[3., 18.], [10., 13.], [21., 17.], [14., 22.]]);
                geometry.path(&[[12., 17.], [12., 3.]]);
                geometry.path(&[[8., 7.], [12., 3.], [16., 7.]]);
            }
            Self::RigBind => {
                geometry.closed(&[[3., 3.], [21., 3.], [21., 21.], [3., 21.]]);
                geometry.path(&[[7., 17.], [17., 7.]]);
                for c in [[7., 17.], [17., 7.]] {
                    geometry.circle(c, 2.);
                }
                geometry.path(&[[3., 3.], [7., 17.], [21., 21.]]);
            }
            Self::Eyedropper => {
                geometry.closed(&[[5., 15.], [14., 6.], [18., 10.], [9., 19.], [4., 20.]]);
                geometry.path(&[[12., 5.], [19., 12.]]);
                geometry.closed(&[[15., 5.], [18., 2.], [22., 6.], [19., 9.]]);
                geometry.path(&[[4., 20.], [2., 22.]]);
            }
            Self::Pose => {
                geometry.circle([12., 4.], 2.);
                geometry.path(&[[12., 7.], [12., 14.], [6., 21.]]);
                geometry.path(&[[12., 14.], [18., 20.]]);
                geometry.path(&[[3., 8.], [8., 11.], [12., 9.], [17., 11.], [21., 6.]]);
            }
            Self::Uv => {
                geometry.path(&[[2., 5.], [2., 15.]]);
                geometry.arc([6., 15.], [4., 4.], 0., 180.);
                geometry.path(&[[10., 15.], [10., 5.]]);
                geometry.path(&[[13., 5.], [17.5, 19.], [22., 5.]]);
            }
            Self::UvIslands => {
                geometry.closed(&[[3., 4.], [12., 2.], [21., 5.], [19., 21.], [5., 19.]]);
                geometry.path(&[[3., 4.], [11., 11.], [21., 5.]]);
                geometry.path(&[[11., 11.], [19., 21.]]);
                geometry.path(&[[11., 11.], [5., 19.]]);
                geometry.dot([11., 11.], 1.3);
            }
            Self::Bone => {
                geometry.circle([6., 18.], 3.);
                geometry.circle([18., 6.], 3.);
                geometry.path(&[[8.2, 15.8], [15.8, 8.2]]);
            }
            Self::Physics => {
                geometry.circle([12., 8.], 4.5);
                geometry.path(&[[4., 21.], [20., 21.]]);
                geometry.path(&[[12., 14.], [12., 18.], [9., 16.]]);
            }
            Self::Settings => {
                let points: Vec<_> = (0..24)
                    .map(|i| {
                        let angle = i as f32 * std::f32::consts::TAU / 24.;
                        let radius = if i % 3 == 0 { 10. } else { 8. };
                        [12. + radius * angle.cos(), 12. + radius * angle.sin()]
                    })
                    .collect();
                geometry.closed(&points);
                geometry.circle([12., 12.], 3.5);
            }
            Self::Camera => {
                geometry.closed(&[
                    [2., 8.],
                    [7., 8.],
                    [9., 4.],
                    [15., 4.],
                    [17., 8.],
                    [22., 8.],
                    [22., 21.],
                    [2., 21.],
                ]);
                geometry.circle([12., 14.], 4.);
            }
            Self::Search => {
                geometry.circle([10., 10.], 6.);
                geometry.path(&[[14.5, 14.5], [21., 21.]]);
            }
            Self::Filter => {
                geometry.path(&[
                    [3., 5.],
                    [21., 5.],
                    [14., 13.],
                    [14., 20.],
                    [10., 18.],
                    [10., 13.],
                    [3., 5.],
                ]);
            }
            Self::Close => {
                geometry.path(&[[5., 5.], [19., 19.]]);
                geometry.path(&[[19., 5.], [5., 19.]]);
            }
            Self::Help => {
                geometry.circle([12., 12.], 10.);
                geometry.path(&[
                    [8., 8.],
                    [9., 5.],
                    [14., 5.],
                    [16., 7.],
                    [16., 10.],
                    [12., 13.],
                    [12., 15.],
                ]);
                geometry.dot([12., 18.], 0.9);
            }
            Self::Frame => {
                for points in [
                    [[3., 9.], [3., 3.], [9., 3.]],
                    [[15., 3.], [21., 3.], [21., 9.]],
                    [[21., 15.], [21., 21.], [15., 21.]],
                    [[9., 21.], [3., 21.], [3., 15.]],
                ] {
                    geometry.path(&points);
                }
                geometry.dot([12., 12.], 1.7);
            }
            Self::Check => geometry.path(&[[4., 12.], [10., 18.], [21., 5.]]),
        }
        let mut paint = Ui::new(ui.scale);
        let segments: Vec<_> = geometry
            .lines
            .into_iter()
            .map(|(a, b)| (point(a), point(b)))
            .collect();
        paint.smooth_segments(&segments, unit * 2., color);
        for polygon in geometry.fills {
            let points: Vec<_> = polygon.into_iter().map(point).collect();
            paint.convex_polygon(&points, color);
        }
        let physical = rect.map(|value| value * ui.scale);
        ui.rects
            .extend(paint.rects.into_iter().filter_map(|mut primitive| {
                let [x, y, w, h] = primitive.rect;
                let left = x.max(physical[0]);
                let top = y.max(physical[1]);
                let right = (x + w).min(physical[0] + physical[2]);
                let bottom = (y + h).min(physical[1] + physical[3]);
                primitive.rect = [left, top, right - left, bottom - top];
                (right > left && bottom > top).then_some(primitive)
            }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Theme;

    const ALL: &[Icon] = &[
        Icon::SelectPointer,
        Icon::SelectBox,
        Icon::SelectLasso,
        Icon::GrowSelection,
        Icon::ShrinkSelection,
        Icon::Connected,
        Icon::InvertSelection,
        Icon::EdgeLoop,
        Icon::EdgeRing,
        Icon::Extrude,
        Icon::Inset,
        Icon::Bevel,
        Icon::LoopCut,
        Icon::Knife,
        Icon::SlideEdge,
        Icon::Merge,
        Icon::Weld,
        Icon::Bridge,
        Icon::Dissolve,
        Icon::Flatten,
        Icon::Straighten,
        Icon::EqualSpace,
        Icon::Relax,

        Icon::ModelSphere,
        Icon::ModelCircle,
        Icon::Inflate,
        Icon::Twist,
        Icon::Subdivide,
        Icon::InspectMesh,
        Icon::SmoothNormals,
        Icon::FlatNormals,
        Icon::DeleteFaces,
        Icon::Physics,
        Icon::Object,
        Icon::Text,
        Icon::Empty,
        Icon::Force,Icon::Wind,Icon::Vortex,Icon::Magnet,Icon::Cylinder,Icon::Cone,Icon::Torus,Icon::Capsule,Icon::Pyramid,
        Icon::Fx,Icon::Fire,Icon::Smoke,Icon::Fog,
        Icon::Scene,
        Icon::Vertex,
        Icon::Edge,
        Icon::Face,
        Icon::Plane,
        Icon::Sphere,
        Icon::Cloth,
        Icon::Sun,
        Icon::Spot,
        Icon::Area,
        Icon::Material,
        Icon::MaterialAssign,
        Icon::MaterialUnassign,
        Icon::Render,
        Icon::Fluid,
        Icon::File,
        Icon::Folder,
        Icon::FolderAdd,
        Icon::Filter,
        Icon::Import,
        Icon::Save,
        Icon::Export,
        Icon::Sequence,
        Icon::Undo,
        Icon::Redo,
        Icon::Orbit,
        Icon::View,
        Icon::Perspective,
        Icon::Orthographic,
        Icon::Hand,
        Icon::Fracture,
        Icon::AxisSpace, Icon::Curve, Icon::CurveCircle, Icon::Library,
        Icon::Move,
        Icon::Rotate,
        Icon::Reset,
        Icon::Scale,
        Icon::Play,
        Icon::Pause,
        Icon::Start,
        Icon::Previous,
        Icon::Next,
        Icon::Key,
        Icon::Copy,
        Icon::Paste,
        Icon::Duplicate,
        Icon::Delete,
        Icon::Light,
        Icon::Eye,
        Icon::Grid,
        Icon::Bone,
        Icon::RigBind,
        Icon::Eyedropper,
        Icon::Pose,
        Icon::Normals,
        Icon::Uv,
        Icon::UvIslands,
        Icon::Settings,
        Icon::Camera,
        Icon::Lock,
        Icon::Search,
        Icon::Close,
        Icon::Help,
        Icon::Frame,
        Icon::Check,
    ];

    #[test]
    fn every_vector_icon_is_visible_finite_bounded_and_independent_of_font_at_each_dpi() {
        for theme in [Theme::Dark, Theme::Light] {
            for scale in [1., 1.5, 2.] {
                for &icon in ALL {
                    let rect = [10.25, 14.5, 16., 20.];
                    let mut ui = Ui::with_theme(scale, theme, [0., 0.]);
                    icon.draw(&mut ui, rect, theme.palette().text);
                    assert!(!ui.rects.is_empty(), "Missing {icon:?} at {scale}");
                    assert!(ui.rects.len() < 1500);
                    assert!(ui.texts.is_empty() && ui.hits.is_empty());
                    for primitive in ui.rects {
                        let [x, y, w, h] = primitive.rect;
                        assert!(primitive.rect.iter().all(|value| value.is_finite()));
                        assert!(w > 0. && h > 0.);
                        assert!(x >= rect[0] * scale - 0.001 && y >= rect[1] * scale - 0.001);
                        assert!(x + w <= (rect[0] + rect[2]) * scale + 0.001);
                        assert!(y + h <= (rect[1] + rect[3]) * scale + 0.001);
                        assert_eq!(&primitive.color[..3], &theme.palette().text[..3]);
                    }
                }
            }
        }
    }

    #[test]
    fn icons_reject_invalid_or_excessive_geometry_before_allocating_masks() {
        let mut ui = Ui::new(1.);
        for rect in [
            [0., 0., f32::NAN, 16.],
            [0., 0., 16., -1.],
            [0., 0., 10000., 10000.],
        ] {
            Icon::Settings.draw(&mut ui, rect, [1.; 4]);
        }
        Icon::File.draw(&mut ui, [0., 0., 16., 16.], [f32::NAN; 4]);
        assert!(ui.rects.is_empty());
    }
}
