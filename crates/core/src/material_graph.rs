//! Bounded material DAG. IDs survive reordering; compilation rejects cycles and dangling links.
use crate::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_MATERIAL_NODES: usize = 32;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MaterialNodeKind {
    Color,
    Value,
    Texture,
    Checker,
    Noise,
    Mix,
    Multiply,
    ColorRamp,
    Bump,
    NormalMap,
    Coordinates,
}
impl MaterialNodeKind {
    pub const ALL: [Self; 11] = [
        Self::Color,
        Self::Value,
        Self::Texture,
        Self::Checker,
        Self::Noise,
        Self::Mix,
        Self::Multiply,
        Self::ColorRamp,
        Self::Bump,
        Self::NormalMap,
        Self::Coordinates,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Color => "Цвет",
            Self::Value => "Значение",
            Self::Texture => "Изображение",
            Self::Checker => "Шахматная",
            Self::Noise => "Шум",
            Self::Mix => "Смешать",
            Self::Multiply => "Умножить",
            Self::ColorRamp => "Цветовая рампа",
            Self::Bump => "Рельеф · Bump",
            Self::NormalMap => "Карта нормалей",
            Self::Coordinates => "Координаты UV",
        }
    }
    pub fn input_count(self) -> usize {
        match self {
            Self::Multiply => 2,
            Self::Checker => 3,
            Self::Texture | Self::Noise | Self::ColorRamp | Self::Bump | Self::NormalMap => 1,
            Self::Mix => 3,
            _ => 0,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialNode {
    pub id: u32,
    pub kind: MaterialNodeKind,
    pub position: [f32; 2],
    pub value: [f32; 4],
    pub inputs: [Option<u32>; 3],
    #[serde(default)]
    pub texture: Option<u64>,
    #[serde(default)]
    pub data_texture: bool,
    #[serde(default = "ramp_white")]
    pub secondary: [f32; 4],
}
fn ramp_white() -> [f32; 4] {
    [1.; 4]
}
impl MaterialNode {
    pub fn new(id: u32, kind: MaterialNodeKind, position: [f32; 2]) -> Self {
        Self {
            id,
            kind,
            position,
            inputs: [None; 3],
            texture: None,
            data_texture: false,
            secondary: [1.; 4],
            value: match kind {
                MaterialNodeKind::Color => [0.8, 0.8, 0.8, 1.],
                MaterialNodeKind::Checker | MaterialNodeKind::Noise => [8., 0., 0., 0.],
                MaterialNodeKind::Mix => [0.5, 0., 0., 0.],
                MaterialNodeKind::ColorRamp => [0., 0., 0., 1.],
                MaterialNodeKind::Bump => [0.1, 0., 0., 0.],
                MaterialNodeKind::Coordinates => [1., 1., 0., 0.],
                _ => [1., 1., 1., 1.],
            },
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialGraph {
    pub enabled: bool,
    pub nodes: Vec<MaterialNode>,
    /// Color, metallic, roughness, emission, opacity, transmission, IOR, tangent normal.
    #[serde(deserialize_with = "read_outputs")]
    pub outputs: [Option<u32>; 8],
    pub output_position: [f32; 2],
}
impl Default for MaterialGraph {
    fn default() -> Self {
        Self {
            enabled: true,
            nodes: Vec::new(),
            outputs: [None; 8],
            output_position: [540., 120.],
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MaterialInstruction {
    pub op_inputs: [i32; 4],
    pub value: [f32; 4],
    pub secondary: [f32; 4],
    pub texture: Option<u64>,
    pub data_texture: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialProgram {
    pub nodes: Vec<MaterialInstruction>,
    pub outputs: [i32; 8],
}
impl MaterialGraph {
    pub fn compile(&self) -> Result<MaterialProgram, CoreError> {
        let fail = |message: &str| CoreError::Invalid(format!("Узлы материала: {message}"));
        if self.nodes.len() > MAX_MATERIAL_NODES {
            return Err(fail("не более 32 узлов"));
        }
        if self
            .output_position
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1e6)
        {
            return Err(fail("положение выхода"));
        }
        let mut lookup = BTreeMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if node.id == 0 || lookup.insert(node.id, i).is_some() {
                return Err(fail("повторяющийся или нулевой ID"));
            }
            if node
                .position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e6)
                || node.value.iter().any(|v| {
                    !v.is_finite()
                        || v.abs() > 10000.
                        || (*v < 0. && node.kind != MaterialNodeKind::Coordinates)
                })
            {
                return Err(fail("неверное значение или положение"));
            }
            if matches!(
                node.kind,
                MaterialNodeKind::Color | MaterialNodeKind::ColorRamp
            ) && node.value.iter().any(|v| *v > 1.)
                || node.kind == MaterialNodeKind::Mix && node.value[0] > 1.
                || matches!(
                    node.kind,
                    MaterialNodeKind::Checker | MaterialNodeKind::Noise
                ) && !(0.01..=1000.).contains(&node.value[0])
            {
                return Err(fail("параметр вне диапазона"));
            }
            if node
                .secondary
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            {
                return Err(fail("цвет рампы"));
            }
            if node
                .inputs
                .iter()
                .skip(node.kind.input_count())
                .any(Option::is_some)
            {
                return Err(fail("у узла нет такого входа"));
            }
        }
        fn visit(
            i: usize,
            graph: &MaterialGraph,
            lookup: &BTreeMap<u32, usize>,
            state: &mut [u8],
            order: &mut Vec<usize>,
        ) -> Result<(), CoreError> {
            if state[i] == 1 {
                return Err(CoreError::Invalid("Узлы материала: цикл соединений".into()));
            }
            if state[i] == 2 {
                return Ok(());
            }
            state[i] = 1;
            for id in graph.nodes[i].inputs.iter().flatten() {
                let &j = lookup.get(id).ok_or_else(|| {
                    CoreError::Invalid("Узлы материала: отсутствует источник".into())
                })?;
                visit(j, graph, lookup, state, order)?;
            }
            state[i] = 2;
            order.push(i);
            Ok(())
        }
        let mut order = Vec::new();
        let mut state = vec![0; self.nodes.len()];
        for i in 0..self.nodes.len() {
            visit(i, self, &lookup, &mut state, &mut order)?;
        }
        let indices: BTreeMap<_, _> = order
            .iter()
            .enumerate()
            .map(|(i, &j)| (self.nodes[j].id, i as i32))
            .collect();
        let index = |id: Option<u32>| -> Result<i32, CoreError> {
            id.map_or(Ok(-1), |id| {
                indices
                    .get(&id)
                    .copied()
                    .ok_or_else(|| fail("отсутствует источник выхода"))
            })
        };
        let mut nodes = Vec::new();
        for i in order {
            let n = &self.nodes[i];
            nodes.push(MaterialInstruction {
                op_inputs: [
                    n.kind as i32,
                    index(n.inputs[0])?,
                    index(n.inputs[1])?,
                    index(n.inputs[2])?,
                ],
                value: n.value,
                secondary: n.secondary,
                texture: n.texture,
                data_texture: n.data_texture,
            });
        }
        Ok(MaterialProgram {
            nodes,
            outputs: [
                index(self.outputs[0])?,
                index(self.outputs[1])?,
                index(self.outputs[2])?,
                index(self.outputs[3])?,
                index(self.outputs[4])?,
                index(self.outputs[5])?,
                index(self.outputs[6])?,
                index(self.outputs[7])?,
            ],
        })
    }
    pub fn connect(
        &mut self,
        target: u32,
        slot: usize,
        source: Option<u32>,
    ) -> Result<(), CoreError> {
        let old = self.clone();
        let input = if target == 0 {
            self.outputs.get_mut(slot)
        } else {
            self.nodes
                .iter_mut()
                .find(|n| n.id == target)
                .and_then(|n| n.inputs.get_mut(slot))
        };
        let Some(input) = input else {
            return Err(CoreError::Invalid("Узел или вход не найден".into()));
        };
        *input = source;
        if let Err(e) = self.compile() {
            *self = old;
            return Err(e);
        }
        Ok(())
    }
    pub fn remove(&mut self, id: u32) {
        self.nodes.retain(|n| n.id != id);
        for input in self
            .outputs
            .iter_mut()
            .chain(self.nodes.iter_mut().flat_map(|n| n.inputs.iter_mut()))
        {
            if *input == Some(id) {
                *input = None;
            }
        }
    }
}

fn read_outputs<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[Option<u32>; 8], D::Error> {
    let input = Vec::<Option<u32>>::deserialize(d)?;
    if input.len() != 4 && input.len() != 8 {
        return Err(serde::de::Error::custom(
            "material outputs: expected 4 legacy or 8 current sockets",
        ));
    }
    let mut outputs = [None; 8];
    outputs[..input.len()].copy_from_slice(&input);
    Ok(outputs)
}
