//! Scene cameras are regular transform nodes, so parenting, keys, duplication
//! and undo use the same stable object identity as other scene objects.
use crate::{Camera, CoreError, NodeId, Project, Transform};
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CameraLens {
    pub effects: CameraEffects,
    pub fov_y_radians: f32,
    pub orthographic: bool,
    /// Half-height of the orthographic frame in scene units.
    pub ortho_size: f32,
    pub depth_of_field: DepthOfField,
    pub thirds: bool,
}

impl Default for CameraLens {
    fn default() -> Self {
        Self {
            effects: CameraEffects::default(),
            fov_y_radians: 45_f32.to_radians(),
            orthographic: false,
            ortho_size: 3.,
            depth_of_field: DepthOfField::default(),
            thirds: true,
        }
    }
}

impl CameraLens {
    pub fn validate(&self) -> Result<(), CoreError> {
        self.effects.validate()?;
        self.depth_of_field.validate()?;
        if !self.fov_y_radians.is_finite()
            || !(0.01..3.13).contains(&self.fov_y_radians)
            || !self.ortho_size.is_finite()
            || !(0.001..=1_000_000.).contains(&self.ortho_size)
        {
            return Err(CoreError::Invalid("параметры объектива камеры".into()));
        }
        Ok(())
    }

    pub fn from_camera(camera: &Camera) -> Self {
        Self {
            effects: camera.effects,
            fov_y_radians: camera.fov_y_radians,
            orthographic: camera.orthographic,
            ortho_size: camera.ortho_size,
            depth_of_field: camera.depth_of_field,
            thirds: true,
        }
    }
}

/// Use a right-handed camera with local -Z forward and local +Y up.
pub fn camera_transform(camera: &Camera) -> Transform {
    Transform::from_matrix(Mat4::look_at_rh(camera.eye, camera.target, camera.up).inverse())
}

impl Project {
    pub fn camera_from_node(&self, id: NodeId, globals: &BTreeMap<NodeId, Mat4>) -> Option<Camera> {
        let lens = self.node(id)?.camera?;
        let world = *globals.get(&id)?;
        let eye = world.transform_point3(Vec3::ZERO);
        let forward = world.transform_vector3(Vec3::NEG_Z).try_normalize()?;
        // Gram-Schmidt keeps a sheared/scaled parent from producing a skewed
        // look-at basis. Scale does not change a camera's focal length.
        let hint_up = world.transform_vector3(Vec3::Y);
        let right = forward.cross(hint_up).try_normalize()?;
        let up = right.cross(forward).try_normalize()?;
        if !eye.is_finite() {
            return None;
        }
        Some(Camera {
            effects: lens.effects,
            eye,
            target: eye + forward,
            up,
            fov_y_radians: lens.fov_y_radians,
            orthographic: lens.orthographic,
            ortho_size: lens.ortho_size,
            depth_of_field: lens.depth_of_field,
            ..self.camera.clone()
        })
    }

    /// Evaluate with the current frame's globals, including animated parents.
    pub fn render_camera(&self, globals: &BTreeMap<NodeId, Mat4>) -> Camera {
        self.active_camera
            .and_then(|id| self.camera_from_node(id, globals))
            .unwrap_or_else(|| self.camera.clone())
    }

    pub(crate) fn validate_scene_cameras(&self) -> Result<(), CoreError> {
        for node in &self.nodes {
            if let Some(lens) = &node.camera {
                lens.validate()?;
            }
        }
        if self
            .active_camera
            .is_some_and(|id| self.node(id).and_then(|node| node.camera).is_none())
        {
            return Err(CoreError::Invalid(
                "активная камера отсутствует в сцене".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DepthOfField {
    pub enabled: bool,
    /// Distance along the camera's forward axis, in metres.
    pub focus_distance: f32,
    pub f_stop: f32,
}
impl Default for DepthOfField {
    fn default() -> Self {
        Self {
            enabled: false,
            focus_distance: 4.,
            f_stop: 2.8,
        }
    }
}
impl DepthOfField {
    pub fn validate(&self) -> Result<(), CoreError> {
        if !self.focus_distance.is_finite()
            || !(0.01..=100000.).contains(&self.focus_distance)
            || !self.f_stop.is_finite()
            || !(0.7..=64.).contains(&self.f_stop)
        {
            return Err(CoreError::Invalid(
                "фокус от 0.01 до 100000 м; диафрагма от f/0.7 до f/64".into(),
            ));
        }
        Ok(())
    }
    /// Circle of confusion radius in output pixels, assuming a 24 mm sensor height.
    pub fn pixel_coefficient(&self, fov_y: f32, height: f32) -> f32 {
        if !self.enabled {
            return 0.;
        }
        let focal = 0.024 / (2. * (fov_y * 0.5).tan());
        0.5 * height * focal * focal
            / (self.f_stop * 0.024 * (self.focus_distance - focal).max(0.001))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedView {
    pub name: String,
    pub camera: Camera,
}
impl Camera {
    pub fn validate(&self) -> Result<(), CoreError> {
        self.effects.validate()?;
        self.depth_of_field.validate()?;
        let c = self;
        if !c.eye.is_finite()
            || !c.target.is_finite()
            || !c.up.is_finite()
            || c.eye.distance_squared(c.target) < 1e-8
            || c.up.length_squared() < 1e-8
            || (c.target - c.eye).normalize().dot(c.up.normalize()).abs() > 0.99999
            || !c.near.is_finite()
            || !c.far.is_finite()
            || c.near <= 0.
            || c.far <= c.near
            || !c.fov_y_radians.is_finite()
            || !(0.01..3.13).contains(&c.fov_y_radians)
            || !c.ortho_size.is_finite()
            || c.ortho_size <= 0.
        {
            return Err(CoreError::Invalid("параметры камеры".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CameraEffects {
    /// None preserves the appearance of files written before the effect list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<CameraEffectList>,
    pub enabled: bool,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub vignette: f32,
    pub grain: f32,
    /// Radial red/blue displacement in pixels at a 1080-pixel frame height.
    pub aberration: f32,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct CameraEffectList {
    pub present: u8,
    pub enabled: u8,
    /// Presentation order shared by the inspector and outliner; zero slots are unused.
    /// Older files without this field retain the original ascending field order.
    #[serde(default)]
    pub order: [u8; 7],
}
impl Default for CameraEffects {
    fn default() -> Self { Self { list: None, enabled: false, exposure: 0., contrast: 1., saturation: 1., temperature: 0., vignette: 0., grain: 0., aberration: 0. } }
}
impl CameraEffects {
    pub const LABELS: [&'static str; 7] = ["Экспозиция", "Контраст", "Насыщенность", "Температура", "Виньетка", "Зерно", "Хроматическая аберрация"];
    fn effect_bit(field: usize) -> u8 { if (4..=10).contains(&field) { 1 << (field - 4) } else { 0 } }
    pub fn effect_list(self) -> CameraEffectList {
        let mut list = self.list.unwrap_or_else(|| {
            let defaults=Self::default();
            let present=(4..=10).filter(|&field| self.field(field).unwrap().0!=defaults.field(field).unwrap().0)
                .fold(0,|mask,field|mask|Self::effect_bit(field));
            CameraEffectList {present,enabled:present,..Default::default()}
        });
        let mut order = [0; 7];
        let mut len = 0;
        for field in list.order.into_iter().chain(4..=10) {
            if list.present & Self::effect_bit(field as usize) != 0 && !order.contains(&field) {
                order[len] = field;
                len += 1;
            }
        }
        list.order = order;
        list
    }
    pub fn ordered_fields(self) -> impl Iterator<Item = usize> {
        self.effect_list().order.into_iter().filter(|&field| field != 0).map(usize::from)
    }
    pub fn contains(self, field: usize) -> bool { self.effect_list().present & Self::effect_bit(field) != 0 }
    pub fn effect_enabled(self, field: usize) -> bool { self.effect_list().enabled & Self::effect_bit(field) != 0 }
    pub fn add(&mut self, field: usize) -> Result<(), CoreError> {
        let value=match field {4=>0.5,5|6=>1.2,7=>0.15,8=>0.3,9=>0.04,10=>1.5,
            _=>return Err(CoreError::Invalid("Неизвестный эффект камеры".into()))};
        if self.contains(field) {return Ok(());}
        self.set(field,value)?;self.enabled=true;Ok(())
    }
    pub fn remove(&mut self, field: usize) {
        if !self.contains(field) {return;}
        let mut list=self.effect_list();let bit=Self::effect_bit(field);
        list.present&=!bit;list.enabled&=!bit;self.list=Some(list);
        self.list=Some(self.effect_list());
        self.set_value(field,Self::default().field(field).unwrap().0);
    }
    pub fn toggle(&mut self, field: usize) {
        if !self.contains(field) {return;}
        let mut list=self.effect_list();list.enabled^=Self::effect_bit(field);self.list=Some(list);
    }
    pub fn move_before(&mut self, field: usize, before: Option<usize>) -> Result<bool, CoreError> {
        if !self.contains(field) || before.is_some_and(|target| !self.contains(target)) {
            return Err(CoreError::Invalid("Эффект отсутствует в списке камеры".into()));
        }
        if before == Some(field) { return Ok(false); }
        let mut list = self.effect_list();
        let mut fields: Vec<_> = self.ordered_fields().filter(|&f| f != field).collect();
        let index = before.and_then(|target| fields.iter().position(|&f| f == target)).unwrap_or(fields.len());
        fields.insert(index, field);
        let mut order = [0; 7];
        for (slot, field) in order.iter_mut().zip(fields) { *slot = field as u8; }
        if list.order == order { return Ok(false); }
        list.order = order;
        self.list = Some(list);
        Ok(true)
    }
    pub fn field(self, field: usize) -> Option<(f32, f32, f32, &'static str)> {
        Some(match field {
            4 => (self.exposure, -8., 8., "Экспозиция, EV"),
            5 => (self.contrast, 0., 2., "Контраст"),
            6 => (self.saturation, 0., 2., "Насыщенность"),
            7 => (self.temperature, -1., 1., "Температура · холод / тепло"),
            8 => (self.vignette, 0., 1., "Виньетка"),
            9 => (self.grain, 0., 0.3, "Зерно"),
            10 => (self.aberration, 0., 8., "Хроматическая аберрация, px"),
            _ => return None,
        })
    }
    pub fn set(&mut self, field: usize, value: f32) -> Result<(), CoreError> {
        let (_, min, max, _) = self.field(field).ok_or_else(|| CoreError::Invalid("Неизвестный эффект камеры".into()))?;
        if !value.is_finite() || !(min..=max).contains(&value) {
            return Err(CoreError::Invalid(format!("Эффект камеры: допустимый диапазон {min}…{max}")));
        }
        let mut list=self.effect_list();let bit=Self::effect_bit(field);
        if list.present&bit==0 {
            list.present|=bit;list.enabled|=bit;
            if let Some(slot)=list.order.iter_mut().find(|f|**f==0) {*slot=field as u8;}
        }
        self.list=Some(list);self.set_value(field,value);
        Ok(())
    }
    fn set_value(&mut self, field:usize, value:f32) {
        match field { 4 => self.exposure = value, 5 => self.contrast = value, 6 => self.saturation = value,
            7 => self.temperature = value, 8 => self.vignette = value, 9 => self.grain = value,
            10 => self.aberration = value, _ => unreachable!() }
    }
    pub fn validate(self) -> Result<(), CoreError> {
        if self.list.is_some_and(|list|list.present&!127!=0 || list.enabled&!list.present!=0) {
            return Err(CoreError::Invalid("Список эффектов камеры".into()));
        }
        if let Some(list) = self.list {
            let mut seen = 0;
            for field in list.order.into_iter().filter(|&field| field != 0) {
                let bit = Self::effect_bit(field as usize);
                if bit == 0 || list.present & bit == 0 || seen & bit != 0 {
                    return Err(CoreError::Invalid("Порядок эффектов камеры".into()));
                }
                seen |= bit;
            }
        }
        for field in 4..=10 { let (value, min, max, _) = self.field(field).unwrap();
            if !value.is_finite() || !(min..=max).contains(&value) { return Err(CoreError::Invalid("Параметры эффектов камеры".into())); }
        } Ok(())
    }
    pub fn uniforms(self) -> ([f32; 4], [f32; 4]) {
        if !self.enabled { return ([0., 1., 1., 0.], [0.; 4]); }
        let mut active=self;
        for field in 4..=10 {if !self.effect_enabled(field) {active.set_value(field,Self::default().field(field).unwrap().0);}}
        ([active.exposure, active.contrast, active.saturation, active.temperature], [active.vignette, active.grain, active.aberration, 1.])
    }
}
