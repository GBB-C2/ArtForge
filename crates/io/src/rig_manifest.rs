//! Read-only inspection of Blender-style bone manifests. A text manifest contains
//! names and hierarchy, not bind matrices, geometry, constraints, or a usable rig.

use crate::{IoError, Result};
use poseforge_core::{Node, NodeId, Project};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RigBone {
    pub name: String,
    pub parent: Option<String>,
    pub path: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RigManifest {
    pub armature: String,
    pub declared_count: usize,
    pub bones: Vec<RigBone>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RigMatchReport {
    /// Only exact, unique node names are matched. Names are never guessed.
    pub matched: Vec<(String, NodeId)>,
    pub missing: Vec<String>,
    pub ambiguous: Vec<String>,
    pub hierarchy_mismatches: Vec<String>,
}

impl RigMatchReport {
    pub fn summary(&self) -> String {
        format!(
            "Совпало: {} · отсутствуют: {} · неоднозначны: {} · различия иерархии: {}",
            self.matched.len(),
            self.missing.len(),
            self.ambiguous.len(),
            self.hierarchy_mismatches.len()
        )
    }
}

impl RigManifest {
    /// Compare against the current scene without modifying its authored data.
    /// A root bone may be parented to a scene/armature container not in the list.
    pub fn compare(&self, project: &Project) -> RigMatchReport {
        let mut names: HashMap<&str, Vec<&Node>> = HashMap::new();
        let nodes_by_id: HashMap<NodeId, &Node> =
            project.nodes.iter().map(|node| (node.id, node)).collect();
        for node in &project.nodes {
            names.entry(node.name.as_str()).or_default().push(node);
        }
        let manifest_names: std::collections::HashSet<&str> =
            self.bones.iter().map(|bone| bone.name.as_str()).collect();
        let mut report = RigMatchReport::default();
        for bone in &self.bones {
            let Some(matches) = names.get(bone.name.as_str()) else {
                report.missing.push(bone.name.clone());
                continue;
            };
            if matches.len() != 1 {
                report.ambiguous.push(bone.name.clone());
                continue;
            }
            let node = matches[0];
            report.matched.push((bone.name.clone(), node.id));
            let actual_parent = node.parent.and_then(|id| nodes_by_id.get(&id).copied());
            let hierarchy_matches = match bone.parent.as_deref() {
                Some(expected) => actual_parent.is_some_and(|parent| parent.name == expected),
                None => actual_parent
                    .is_none_or(|parent| !manifest_names.contains(parent.name.as_str())),
            };
            if !hierarchy_matches {
                report.hierarchy_mismatches.push(format!(
                    "{}: ожидается {}, в сцене {}",
                    bone.name,
                    bone.parent.as_deref().unwrap_or("корень"),
                    actual_parent
                        .map(|node| node.name.as_str())
                        .unwrap_or("корень")
                ));
            }
        }
        report
    }
}

/// Parse the exported `Armature`/`Bones` header and tab-separated bone rows.
/// Reject malformed input instead of inventing missing parent relationships.
pub fn parse_rig_manifest(text: &str) -> Result<RigManifest> {
    if text.len() > 16 * 1024 * 1024 {
        return Err(invalid("список костей превышает 16 МиБ"));
    }
    let mut lines = text
        .trim_start_matches('\u{feff}')
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty());
    let (_, armature_line) = lines.next().ok_or_else(|| invalid("пустой файл"))?;
    let armature = armature_line
        .trim()
        .strip_prefix("Armature:")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("первая строка должна содержать Armature: имя"))?
        .to_owned();
    let (_, count_line) = lines
        .next()
        .ok_or_else(|| invalid("отсутствует строка Bones: количество"))?;
    let declared_count = count_line
        .trim()
        .strip_prefix("Bones:")
        .map(str::trim)
        .and_then(|count| count.parse::<usize>().ok())
        .filter(|count| *count <= 100_000)
        .ok_or_else(|| invalid("Bones должно содержать число от 0 до 100000"))?;
    let mut bones = Vec::new();
    let mut by_name = HashMap::new();
    for (line_no, line) in lines {
        let columns: Vec<_> = line.trim().split('\t').map(str::trim).collect();
        if columns.len() != 3 {
            return Err(invalid(format!(
                "строка {}: нужны имя, parent: и path:, разделённые табуляцией",
                line_no + 1
            )));
        }
        let name = columns[0];
        if name.is_empty() || name == "-" {
            return Err(invalid(format!(
                "строка {}: пустое или недопустимое имя",
                line_no + 1
            )));
        }
        if by_name.insert(name.to_owned(), bones.len()).is_some() {
            return Err(invalid(format!("повтор имени кости: {name}")));
        }
        let parent = columns[1]
            .strip_prefix("parent:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid(format!("строка {}: отсутствует parent:", line_no + 1)))?;
        let path = columns[2]
            .strip_prefix("path:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid(format!("строка {}: отсутствует path:", line_no + 1)))?
            .split(" > ")
            .map(|part| part.trim().to_owned())
            .collect();
        bones.push(RigBone {
            name: name.to_owned(),
            parent: (parent != "-").then(|| parent.to_owned()),
            path,
        });
    }
    if bones.len() != declared_count {
        return Err(invalid(format!(
            "указано {declared_count} костей, прочитано {}",
            bones.len()
        )));
    }

    // Validate links first; all subsequent index lookups refer to this map.
    let parents: Vec<Option<usize>> = bones
        .iter()
        .map(|bone| {
            bone.parent
                .as_ref()
                .map(|parent| {
                    by_name.get(parent).copied().ok_or_else(|| {
                        invalid(format!("{}: родитель {parent} не найден", bone.name))
                    })
                })
                .transpose()
        })
        .collect::<Result<_>>()?;

    // Iterative traversal avoids stack overflow even for very deep exports.
    let mut marks = vec![0_u8; bones.len()];
    for start in 0..bones.len() {
        if marks[start] == 2 {
            continue;
        }
        let mut chain = Vec::new();
        let mut cursor = Some(start);
        while let Some(index) = cursor {
            match marks[index] {
                1 => return Err(invalid(format!("цикл иерархии у {}", bones[index].name))),
                2 => break,
                _ => {}
            }
            marks[index] = 1;
            chain.push(index);
            cursor = parents[index];
        }
        for index in chain {
            marks[index] = 2;
        }
    }
    for bone in &bones {
        let mut expected_parent: Option<&str> = None;
        for name in &bone.path {
            let path_bone = by_name.get(name).map(|index| &bones[*index]);
            if path_bone.is_none_or(|item| item.parent.as_deref() != expected_parent) {
                return Err(invalid(format!(
                    "{}: path не соответствует иерархии",
                    bone.name
                )));
            }
            expected_parent = Some(name);
        }
        if bone.path.last() != Some(&bone.name) {
            return Err(invalid(format!(
                "{}: path должен завершаться именем кости",
                bone.name
            )));
        }
    }
    Ok(RigManifest {
        armature,
        declared_count,
        bones,
    })
}

fn invalid(message: impl std::fmt::Display) -> IoError {
    IoError::Invalid(format!("Список рига: {message}"))
}
