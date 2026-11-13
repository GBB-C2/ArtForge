//! Editable text with bundled outline tessellation; generated meshes remain portable.
use crate::{CoreError, Editor, Material, Mesh, MeshTopology, Node, Project, Vec3, Vertex};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap,sync::OnceLock};
const MAX_TEXT_VERTICES: usize=262_144;
#[derive(Debug,Clone,Copy,Default,Serialize,Deserialize,PartialEq,Eq)]
pub enum TextFont { #[default] Sans, Serif, Mono }
impl TextFont {
    pub const ALL:[Self;3]=[Self::Sans,Self::Serif,Self::Mono];
    pub fn label(self)->&'static str {match self {Self::Sans=>"DejaVu Sans",Self::Serif=>"DejaVu Serif",Self::Mono=>"DejaVu Sans Mono"}}
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct TextSettings {
    pub content: String,
    pub size: f32,
    pub depth: f32,
    pub spacing: f32,
    pub font: TextFont,
    pub bevel: f32,
    pub bevel_segments: u8,
    pub on_curve: bool,
}
impl Default for TextSettings {
    fn default()->Self {Self {content:"Текст".into(),size:1.,depth:0.08,spacing:1.,font:TextFont::Sans,bevel:0.,bevel_segments:3,on_curve:false}}
}
#[derive(Deserialize)]
struct Glyph {advance:f32,points:Vec<[f32;2]>,contours:Vec<Vec<u32>>,triangles:Vec<[u32;3]>,bevel_offsets:Vec<[f32;2]>,bevel_limit:f32}
#[derive(Deserialize)]
struct Font {glyphs:BTreeMap<char,Glyph>}
fn invalid(s:impl Into<String>)->CoreError {CoreError::Invalid(s.into())}
fn font(kind:TextFont)->Result<&'static Font,CoreError> {
    static SANS:OnceLock<Result<Font,String>>=OnceLock::new();
    static SERIF:OnceLock<Result<Font,String>>=OnceLock::new();
    static MONO:OnceLock<Result<Font,String>>=OnceLock::new();
    let (cache,source)=match kind {
        TextFont::Sans=>(&SANS,include_str!("../assets/text_glyphs.json")),
        TextFont::Serif=>(&SERIF,include_str!("../assets/text_glyphs_serif.json")),
        TextFont::Mono=>(&MONO,include_str!("../assets/text_glyphs_mono.json")),
    };
    cache.get_or_init(||serde_json::from_str(source).map_err(|e|e.to_string()))
        .as_ref().map_err(|e|invalid(format!("Ресурс шрифта: {e}")))
}
impl TextSettings {
    pub fn validate(&self)->Result<(),CoreError> {
        if self.content.trim().is_empty() || self.content.chars().count()>256 || self.content.lines().count()>8 {
            return Err(invalid("Текст: 1–256 символов, не более 8 строк"));
        }
        if !self.size.is_finite() || !(0.01..=100.).contains(&self.size)
            || !self.depth.is_finite() || !(0. ..=10.).contains(&self.depth)
            || !self.spacing.is_finite() || !(0.1..=4.).contains(&self.spacing)
            || !self.bevel.is_finite() || !(0. ..=10.).contains(&self.bevel) || !(1..=6).contains(&self.bevel_segments) {
            return Err(invalid("Текст: размер 0,01–100 м, толщина 0–10 м, интервал 0,1–4"));
        }
        let f=font(self.font)?;
        for c in self.content.chars().filter(|c|*c!='\n') {
            if !f.glyphs.contains_key(&c) {return Err(invalid(format!("Символ «{c}» отсутствует во встроенном шрифте. Доступны кириллица, латиница и цифры.")));}
        }
        Ok(())
    }
    pub fn mesh(&self,id:u64,material:Option<u64>)->Result<Mesh,CoreError> {
        self.validate()?;let f=font(self.font)?;
        let mut vertices=Vec::<Vertex>::new();let mut indices=Vec::<u32>::new();let mut faces=Vec::<Vec<u32>>::new();
        let mut offset=[0_f32;2];let solid=self.depth>0.;
        for c in self.content.chars() {
            if c=='\n' {offset[0]=0.;offset[1]-=1.3*self.size;continue;}
            let g=&f.glyphs[&c];
            let radius=self.bevel.min(self.depth*0.49).min(g.bevel_limit*self.size);
            let segments=if radius>1e-7 {self.bevel_segments as usize}else{0};
            let required=g.points.len()*(if solid{2}else{1})+if solid{g.contours.iter().map(|r|r.len()*4*(1+segments*2)).sum::<usize>()}else{0};
            if vertices.len()+required>MAX_TEXT_VERTICES {return Err(invalid("Текст слишком сложный: сократите надпись"));}
            let point=|p:[f32;2],z:f32|[offset[0]+p[0]*self.size,offset[1]+p[1]*self.size,z];
            for back in 0..(if solid{2}else{1}) {
                let base=vertices.len() as u32;let sign=if back==0 {1.} else {-1.};
                vertices.extend(g.points.iter().map(|&p|Vertex {position:point(p,sign*self.depth*0.5),normal:[0.,0.,sign],uv:p,..Default::default()}));
                for &t in &g.triangles {
                    let t=if back==0 {t}else{[t[0],t[2],t[1]]};let face=t.map(|i|base+i);
                    indices.extend(face);faces.push(face.to_vec());
                }
            }
            if solid {
                for contour in &g.contours {for i in 0..contour.len() {
                    let a=g.points[contour[i] as usize];let b=g.points[contour[(i+1)%contour.len()] as usize];
                    let normal=Vec3::new(b[1]-a[1],a[0]-b[0],0.).normalize_or_zero().to_array();
                    let oa=g.bevel_offsets[contour[i] as usize];let ob=g.bevel_offsets[contour[(i+1)%contour.len()] as usize];
                    let raised=|p:[f32;2],o:[f32;2],r:f32,z:f32|point([p[0]+o[0]*r/self.size,p[1]+o[1]*r/self.size],z);
                    let mut quad=|positions:[[f32;3];4],normals:[[f32;3];4],reverse:bool| {
                        let base=vertices.len() as u32;
                        for k in 0..4 {vertices.push(Vertex {position:positions[k],normal:normals[k],uv:[[0.,0.],[1.,0.],[1.,1.],[0.,1.]][k],..Default::default()});}
                        let face=if reverse {[base+3,base+2,base+1,base]} else {[base,base+1,base+2,base+3]};
                        indices.extend([face[0],face[1],face[2],face[0],face[2],face[3]]);faces.push(face.to_vec());
                    };
                    let shoulder=self.depth*0.5-if segments>0 {radius}else{0.};
                    let r=if segments>0 {radius}else{0.};
                    quad([raised(a,oa,r,-shoulder),raised(b,ob,r,-shoulder),raised(b,ob,r,shoulder),raised(a,oa,r,shoulder)],[normal;4],false);
                    for sign in [1_f32,-1.] {for step in 0..segments {
                        let t0=step as f32/segments as f32*std::f32::consts::FRAC_PI_2;
                        let t1=(step+1) as f32/segments as f32*std::f32::consts::FRAC_PI_2;
                        let ring=|p,o,t:f32| {let (s,c)=if t>=std::f32::consts::FRAC_PI_2 {(1.,0.)}else{t.sin_cos()};raised(p,o,radius*s,sign*(self.depth*0.5-radius*(1.-c)))};
                        let n=|t:f32|[normal[0]*t.sin(),normal[1]*t.sin(),sign*t.cos()];
                        quad([ring(a,oa,t1),ring(b,ob,t1),ring(b,ob,t0),ring(a,oa,t0)],[n(t1),n(t1),n(t0),n(t0)],sign<0.);
                    }}
                }}
            }
            offset[0]+=g.advance*self.size*self.spacing;
        }
        if vertices.is_empty() {return Err(invalid("Надпись должна содержать видимые буквы или знаки"));}
        let min=vertices.iter().map(|v|Vec3::from_array(v.position)).fold(Vec3::splat(f32::INFINITY),Vec3::min);
        let max=vertices.iter().map(|v|Vec3::from_array(v.position)).fold(Vec3::splat(f32::NEG_INFINITY),Vec3::max);
        let center=(min+max)*0.5;
        for v in &mut vertices {v.position=(Vec3::from_array(v.position)-center).to_array();}
        let mesh=Mesh {id,name:"Текст".into(),vertices:vertices.into(),indices:indices.into(),material,morph_targets:vec![],
            topology:Some(MeshTopology {faces:faces.into(),face_materials:Default::default(),uv_seams:vec![],smooth_normals:false,primitive:None})};
        mesh.validate_topology()?;Ok(mesh)
    }
}
impl Editor {
    pub fn create_text(&mut self,settings:TextSettings)->Result<u64,CoreError> {
        if self.has_staged() {return Err(invalid("Сначала запишите или отмените временную позу"));}
        let id=self.project.next_id();let mesh=settings.mesh(id+2,Some(id+1))?;
        let material=Material {id:id+1,name:"Материал текста".into(),double_sided:true,..Default::default()};
        let mut node=Node::new(id,"Текст");node.mesh=Some(mesh.id);node.text=Some(settings);
        self.transact("Создать текст",|p|{p.nodes.push(node);p.meshes.push(mesh);p.materials.push(material);})?;Ok(id)
    }
    pub fn set_text(&mut self,id:u64,settings:TextSettings)->Result<(),CoreError> {
        self.authoring_unlocked(id)?;
        let node=self.project.node(id).ok_or(CoreError::MissingNode(id))?;
        if node.text.is_none() || node.skin.is_some() || node.physics.is_some() || node.fluid.is_some() || node.destruction.is_some() {
            return Err(invalid("Параметры текста доступны до назначения физики и привязки"));
        }
        if node.text.as_ref()==Some(&settings) {return Ok(());}
        let old=self.project.mesh(node.mesh.ok_or_else(||invalid("Геометрия текста отсутствует"))?).ok_or_else(||invalid("Геометрия текста удалена"))?;
        if !old.morph_targets.is_empty() {return Err(invalid("Редактируйте надпись до создания морфов"));}
        let mesh_id=if self.project.nodes.iter().any(|n|n.id!=id&&n.mesh==Some(old.id)) {self.project.next_id()} else {old.id};
        let mut mesh=settings.mesh(mesh_id,old.material)?;mesh.name=old.name.clone();
        self.transact("Изменить текст",|p| {
            if let Some(slot)=p.meshes.iter_mut().find(|m|m.id==mesh_id) {*slot=mesh;} else {p.meshes.push(mesh);}
            let node=p.node_mut(id).unwrap();node.mesh=Some(mesh_id);node.text=Some(settings);
        })
    }
    pub fn convert_text_to_mesh(&mut self,id:u64)->Result<(),CoreError> {
        self.authoring_unlocked(id)?;
        if self.project.node(id).is_none_or(|n|n.text.is_none()) {return Err(invalid("Выберите текст"));}
        let evaluated=self.evaluated_base()?;
        let meshes=evaluated.text_meshes.into_iter().filter(|(node,_)|*node==id).collect();
        self.transact("Преобразовать текст в меш",|p| {p.apply_text_meshes(&meshes);p.node_mut(id).unwrap().text=None;})
    }
}
impl Project {
    pub(crate) fn validate_text_objects(&self)->Result<(),CoreError> {
        for node in &self.nodes {
            if node.empty && (node.mesh.is_some() || node.camera.is_some() || node.curve.is_some() || node.text.is_some() || node.is_joint || node.skin.is_some()) {return Err(invalid("Пустышка не должна содержать геометрию, камеру или кость"));}
            if let Some(text)=&node.text {
            text.validate()?;
            if text.on_curve && node.curve_binding.is_some() && (node.skin.is_some() || node.physics.is_some() || node.fluid.is_some() || node.destruction.is_some() || node.mesh.and_then(|id|self.mesh(id)).is_some_and(|m|!m.morph_targets.is_empty())) {
                return Err(invalid("Перед физикой, морфами или привязкой скелета преобразуйте текст по кривой в меш"));
            }
            if node.mesh.is_none() || node.is_joint || node.camera.is_some() || node.curve.is_some() {return Err(invalid("Текст должен содержать собственный меш"));}
        }}
        Ok(())
    }
}

impl Editor {
    pub fn create_empty(&mut self)->Result<u64,CoreError> {
        if self.has_staged() {return Err(invalid("Сначала запишите или отмените временную позу"));}
        self.create_empty_shape(crate::EmptyShape::Axes)
    }
}
