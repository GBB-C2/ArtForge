//! Optional procedural surface styling; independent from NPR lighting and UV maps.
//! All colors remain linear. This is shading, not geometry or fluid simulation.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ArtisticStyle {
    #[default]
    Plasticine,
    Clay,
    Watercolor,
    Gouache,
    Wax,
    Ceramic,
    Paper,
    Pastel,
}
impl ArtisticStyle {
    pub const ALL: [Self; 8] = [Self::Plasticine, Self::Clay, Self::Watercolor,
        Self::Gouache, Self::Wax, Self::Ceramic, Self::Paper, Self::Pastel];
    pub fn label(self) -> &'static str {
        match self {
            Self::Plasticine => "Пластилин", Self::Clay => "Глина",
            Self::Watercolor => "Акварель", Self::Gouache => "Гуашь",
            Self::Wax => "Воск", Self::Ceramic => "Керамика",
            Self::Paper => "Бумага", Self::Pastel => "Пастель",
        }
    }
    /// Stable GPU IDs, deliberately independent of the Rust enum representation.
    pub fn shader_id(self) -> u32 {
        match self { Self::Plasticine=>1, Self::Clay=>2, Self::Watercolor=>3,
            Self::Gouache=>4, Self::Wax=>5, Self::Ceramic=>6, Self::Paper=>7, Self::Pastel=>8 }
    }
    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ArtisticSettings {
    pub enabled: bool,
    pub style: ArtisticStyle,
    /// Cycles per object-space unit. Rest positions keep the pattern attached to skin.
    pub scale: f32,
    pub bump: f32,
    pub grain: f32,
    pub variation: f32,
    pub tool_marks: f32,
    pub cracks: f32,
    /// Bounded wrapped-light approximation, not volumetric subsurface scattering.
    pub subsurface: f32,
    pub pigment: f32,
    pub paper: f32,
    /// Grazing-angle pigment accumulation; does not expand a silhouette.
    pub edge_darkening: f32,
    /// Procedural wash variation, not neighboring-object color bleeding.
    pub wash: f32,
    pub dry_brush: f32,
    pub seed: u32,
}
impl Default for ArtisticSettings {
    fn default() -> Self {
        Self { enabled: false, style: ArtisticStyle::Plasticine,
            scale: 6., bump: 0.3, grain: 0.2, variation: 0.12, tool_marks: 0.3,
            cracks: 0., subsurface: 0.15, pigment: 0.7, paper: 0.35,
            edge_darkening: 0.25, wash: 0.3, dry_brush: 0., seed: 0 }
    }
}
impl ArtisticSettings {
    pub const FIELD_COUNT: usize = 13;
    pub fn for_style(style: ArtisticStyle) -> Self {
        let mut s = Self { enabled: true, style, ..Self::default() };
        match style {
            ArtisticStyle::Plasticine => {},
            ArtisticStyle::Clay => { s.bump=0.55; s.grain=0.65; s.variation=0.2;
                s.tool_marks=0.45; s.subsurface=0.03; },
            ArtisticStyle::Watercolor => { s.scale=3.; s.bump=0.035; s.grain=0.55;
                s.variation=0.35; s.tool_marks=0.; s.subsurface=0.; s.pigment=0.6;
                s.paper=0.65; s.edge_darkening=0.55; s.wash=0.6; s.dry_brush=0.12; },
            ArtisticStyle::Gouache => { s.scale=4.; s.bump=0.1; s.grain=0.45;
                s.variation=0.25; s.tool_marks=0.5; s.subsurface=0.; s.pigment=0.95;
                s.paper=0.35; s.wash=0.12; s.dry_brush=0.2; },
            ArtisticStyle::Wax => { s.scale=3.; s.bump=0.12; s.grain=0.06;
                s.variation=0.08; s.tool_marks=0.08; s.subsurface=0.7; },
            ArtisticStyle::Ceramic => { s.scale=5.; s.bump=0.14; s.grain=0.15;
                s.variation=0.16; s.tool_marks=0.15; s.cracks=0.28; s.subsurface=0.; },
            ArtisticStyle::Paper => { s.scale=5.; s.bump=0.12; s.grain=0.7;
                s.variation=0.2; s.tool_marks=0.; s.subsurface=0.04; s.paper=0.8; },
            ArtisticStyle::Pastel => { s.scale=4.; s.bump=0.1; s.grain=0.85;
                s.variation=0.32; s.tool_marks=0.35; s.subsurface=0.; s.pigment=0.8;
                s.paper=0.75; s.wash=0.15; s.dry_brush=0.55; },
        }
        s
    }
    pub fn number(&self, field: usize) -> f32 {
        match field { 0=>self.scale, 1=>self.bump, 2=>self.grain, 3=>self.variation,
            4=>self.tool_marks, 5=>self.cracks, 6=>self.subsurface, 7=>self.pigment,
            8=>self.paper, 9=>self.edge_darkening, 10=>self.wash, 11=>self.dry_brush,
            12=>self.seed as f32, _=>0. }
    }
    pub fn bounds(field: usize) -> Option<(f32, f32)> {
        match field { 0=>Some((0.1,64.)), 1..=11=>Some((0.,1.)),
            12=>Some((0.,65535.)), _=>None }
    }
    pub fn set_number(&mut self, field: usize, value: f32) -> bool {
        let Some((min,max)) = Self::bounds(field) else { return false; };
        if !value.is_finite() || !(min..=max).contains(&value)
            || (field==12 && value.fract()!=0.) { return false; }
        match field { 0=>self.scale=value, 1=>self.bump=value, 2=>self.grain=value,
            3=>self.variation=value, 4=>self.tool_marks=value, 5=>self.cracks=value,
            6=>self.subsurface=value, 7=>self.pigment=value, 8=>self.paper=value,
            9=>self.edge_darkening=value, 10=>self.wash=value, 11=>self.dry_brush=value,
            12=>self.seed=value as u32, _=>return false }
        true
    }
    pub fn valid(&self) -> bool {
        (0..Self::FIELD_COUNT).all(|field| {
            let value=self.number(field); let (min,max)=Self::bounds(field).unwrap();
            value.is_finite() && (min..=max).contains(&value)
        }) && self.seed<=65535
    }
}
