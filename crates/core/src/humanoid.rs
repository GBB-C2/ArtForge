//! Conservative humanoid role suggestions. Recognition never changes the project,
//! bind transforms, or the imported skeleton; accepted mappings keep stable IDs.
use crate::{CoreError, NodeId, Project};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoneRole {
    Pelvis,
    Spine,
    Chest,
    Neck,
    Head,
    LeftUpperArm,
    LeftForearm,
    LeftHand,
    RightUpperArm,
    RightForearm,
    RightHand,
    LeftThigh,
    LeftShin,
    LeftFoot,
    RightThigh,
    RightShin,
    RightFoot,
}
impl BoneRole {
    pub const ALL: [Self; 17] = [
        Self::Pelvis,
        Self::Spine,
        Self::Chest,
        Self::Neck,
        Self::Head,
        Self::LeftUpperArm,
        Self::LeftForearm,
        Self::LeftHand,
        Self::RightUpperArm,
        Self::RightForearm,
        Self::RightHand,
        Self::LeftThigh,
        Self::LeftShin,
        Self::LeftFoot,
        Self::RightThigh,
        Self::RightShin,
        Self::RightFoot,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Pelvis => "Таз",
            Self::Spine => "Позвоночник",
            Self::Chest => "Грудь",
            Self::Neck => "Шея",
            Self::Head => "Голова",
            Self::LeftUpperArm => "Плечо.L",
            Self::LeftForearm => "Предплечье.L",
            Self::LeftHand => "Кисть.L",
            Self::RightUpperArm => "Плечо.R",
            Self::RightForearm => "Предплечье.R",
            Self::RightHand => "Кисть.R",
            Self::LeftThigh => "Бедро.L",
            Self::LeftShin => "Голень.L",
            Self::LeftFoot => "Стопа.L",
            Self::RightThigh => "Бедро.R",
            Self::RightShin => "Голень.R",
            Self::RightFoot => "Стопа.R",
        }
    }
    pub fn side(self) -> Option<BoneSide> {
        match self {
            Self::LeftUpperArm
            | Self::LeftForearm
            | Self::LeftHand
            | Self::LeftThigh
            | Self::LeftShin
            | Self::LeftFoot => Some(BoneSide::Left),
            Self::RightUpperArm
            | Self::RightForearm
            | Self::RightHand
            | Self::RightThigh
            | Self::RightShin
            | Self::RightFoot => Some(BoneSide::Right),
            _ => None,
        }
    }
    pub fn is_ik_end(self) -> bool {
        matches!(
            self,
            Self::LeftHand | Self::RightHand | Self::LeftFoot | Self::RightFoot
        )
    }
    fn chain_roles(self) -> Option<[Self; 3]> {
        match self {
            Self::LeftHand => Some([Self::LeftUpperArm, Self::LeftForearm, Self::LeftHand]),
            Self::RightHand => Some([Self::RightUpperArm, Self::RightForearm, Self::RightHand]),
            Self::LeftFoot => Some([Self::LeftThigh, Self::LeftShin, Self::LeftFoot]),
            Self::RightFoot => Some([Self::RightThigh, Self::RightShin, Self::RightFoot]),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoneSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoneConfidence {
    High,
    Review,
    Ignored,
}
impl BoneConfidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::High => "Совпадение имени",
            Self::Review => "Нужно подтвердить",
            Self::Ignored => "Служебная кость",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoneCandidate {
    pub node: NodeId,
    pub name: String,
    pub confidence: BoneConfidence,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RigProfile {
    pub root: Option<NodeId>,
    pub assignments: BTreeMap<BoneRole, NodeId>,
}
impl RigProfile {
    /// Change a role only after all checks pass; rejected assignments are atomic.
    pub fn assign(
        &mut self,
        role: BoneRole,
        id: NodeId,
        project: &Project,
    ) -> Result<(), CoreError> {
        let mut proposed = self.clone();
        proposed.assignments.insert(role, id);
        proposed.validate(project)?;
        *self = proposed;
        Ok(())
    }
    pub fn unassign(&mut self, role: BoneRole) {
        self.assignments.remove(&role);
    }
    /// Partial profiles are valid. A mapping is metadata, not a complete IK rig.
    /// Do not call Project::validate here: project validation also checks profiles.
    pub fn validate(&self, project: &Project) -> Result<(), CoreError> {
        if let Some(root) = self.root {
            project.node(root).ok_or(CoreError::MissingNode(root))?;
        }
        let mut used = BTreeSet::new();
        for (role, id) in &self.assignments {
            let node = project.node(*id).ok_or(CoreError::MissingNode(*id))?;
            if !node.is_joint {
                return Err(CoreError::Invalid(format!(
                    "{}: выбранный узел не является суставом",
                    role.label()
                )));
            }
            if !used.insert(*id) {
                return Err(CoreError::Invalid(
                    "Одна кость назначена нескольким ролям".into(),
                ));
            }
            if let Some(root) = self.root {
                if !is_descendant(project, *id, root)? {
                    return Err(CoreError::Invalid(format!(
                        "{}: кость находится вне выбранного персонажа",
                        role.label()
                    )));
                }
            }
        }
        Ok(())
    }
    /// Only immediate parent links are accepted. Helper or twist bones must never
    /// be silently skipped because the existing analytic solver cannot solve them.
    pub fn chain(&self, end_role: BoneRole, project: &Project) -> Result<[NodeId; 3], CoreError> {
        self.validate(project)?;
        let roles = end_role
            .chain_roles()
            .ok_or_else(|| CoreError::Invalid("IK доступен для кисти или стопы".into()))?;
        let mut ids = [0; 3];
        for (slot, role) in ids.iter_mut().zip(roles) {
            *slot = *self.assignments.get(&role).ok_or_else(|| {
                CoreError::Invalid(format!("Не назначена роль: {}", role.label()))
            })?;
        }
        if project.node(ids[1]).and_then(|n| n.parent) != Some(ids[0])
            || project.node(ids[2]).and_then(|n| n.parent) != Some(ids[1])
        {
            return Err(CoreError::Invalid("IK требует прямую цепь из трёх костей; промежуточные управляющие или twist-кости не поддерживаются".into()));
        }
        Ok(ids)
    }
    /// These are anatomical pair suggestions, not proof that local bone axes are
    /// mirror-compatible. The caller must confirm the rest-pose convention before
    /// applying the editor's local-axis mirror operation.
    pub fn mirror_pairs(&self) -> Vec<(NodeId, NodeId)> {
        [
            (BoneRole::LeftUpperArm, BoneRole::RightUpperArm),
            (BoneRole::LeftForearm, BoneRole::RightForearm),
            (BoneRole::LeftHand, BoneRole::RightHand),
            (BoneRole::LeftThigh, BoneRole::RightThigh),
            (BoneRole::LeftShin, BoneRole::RightShin),
            (BoneRole::LeftFoot, BoneRole::RightFoot),
        ]
        .into_iter()
        .filter_map(|(left, right)| {
            Some((
                *self.assignments.get(&left)?,
                *self.assignments.get(&right)?,
            ))
        })
        .collect()
    }
}

#[derive(Debug, Clone)]
pub struct DetectionReport {
    pub root: Option<NodeId>,
    pub candidates: BTreeMap<BoneRole, Vec<BoneCandidate>>,
    pub resolved: RigProfile,
    pub warnings: Vec<String>,
}

/// Candidate character containers, not arbitrary scene roots. A non-joint
/// armature object separates two skeletons under the same scene collection.
/// Only recognizable, non-helper joints contribute candidate containers.
pub fn character_roots(project: &Project) -> Vec<NodeId> {
    // Meshes sharing joints belong to one skin component. This handles clothing
    // and hair skins without splitting one character at unweighted helper nodes.
    let mut components: Vec<BTreeSet<NodeId>> = Vec::new();
    for skin in project.nodes.iter().filter_map(|node| node.skin.as_ref()) {
        let mut joints: BTreeSet<_> = skin.joints.iter().copied().collect();
        if joints.is_empty() {
            continue;
        }
        let mut index = 0;
        while index < components.len() {
            if !components[index].is_disjoint(&joints) {
                joints.extend(components.remove(index));
                index = 0;
            } else {
                index += 1;
            }
        }
        components.push(joints);
    }
    let mut roots = BTreeSet::new();
    for component in components {
        if let Some(root) = common_ancestor(project, &component) {
            let root = if project.node(root).is_some_and(|node| node.is_joint) {
                joint_container(project, root).unwrap_or(root)
            } else {
                root
            };
            roots.insert(root);
        }
    }
    // A scene may also contain an unskinned skeleton next to a skinned one.
    // Discover it without splitting unweighted helper gaps inside known skins.
    let skin_roots = roots.clone();
    for node in project.nodes.iter().filter(|n| n.is_joint) {
        if skin_roots
            .iter()
            .any(|root| is_descendant(project, node.id, *root).unwrap_or(false))
        {
            continue;
        }
        if classify_name(&node.name)
            .is_some_and(|(_, confidence, _)| confidence != BoneConfidence::Ignored)
        {
            if let Ok(root) = joint_container(project, node.id) {
                roots.insert(root);
            }
        }
    }
    roots.into_iter().collect()
}

fn common_ancestor(project: &Project, ids: &BTreeSet<NodeId>) -> Option<NodeId> {
    let mut ancestors = Vec::new();
    let mut cursor = ids.iter().next().copied();
    let mut seen = BTreeSet::new();
    while let Some(id) = cursor {
        if !seen.insert(id) {
            return None;
        }
        let node = project.node(id)?;
        ancestors.push(id);
        cursor = node.parent;
    }
    ancestors.into_iter().find(|ancestor| {
        ids.iter()
            .all(|id| is_descendant(project, *id, *ancestor).unwrap_or(false))
    })
}

/// Suggest exact name aliases within one character. Incomplete names, duplicates,
/// multiple characters and helper/control bones are surfaced for manual review.
/// Name evidence never rewrites transforms or the deforming skeleton.
pub fn detect_humanoid(project: &Project, requested_root: Option<NodeId>) -> DetectionReport {
    let roots = character_roots(project);
    let root = requested_root.or_else(|| (roots.len() == 1).then(|| roots[0]));
    let mut report = DetectionReport {
        root,
        candidates: BoneRole::ALL
            .into_iter()
            .map(|role| (role, Vec::new()))
            .collect(),
        resolved: RigProfile {
            root,
            assignments: BTreeMap::new(),
        },
        warnings: Vec::new(),
    };
    if let Some(id) = root {
        if project.node(id).is_none() {
            report
                .warnings
                .push("Выбранный корень персонажа отсутствует в сцене".into());
            return report;
        }
    }
    let mut scoped_containers = BTreeSet::new();
    let mut invalid_hierarchy = false;
    for node in project.nodes.iter().filter(|n| n.is_joint) {
        if let Some(root) = root {
            match is_descendant(project, node.id, root) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(_) => {
                    invalid_hierarchy = true;
                    continue;
                }
            }
        }
        let Some((role, confidence, reason)) = classify_name(&node.name) else {
            continue;
        };
        if confidence != BoneConfidence::Ignored {
            let owners: Vec<_> = roots
                .iter()
                .copied()
                .filter(|r| is_descendant(project, node.id, *r).unwrap_or(false))
                .collect();
            if owners.len() == 1 {
                scoped_containers.insert(owners[0]);
            } else {
                match joint_container(project, node.id) {
                    Ok(container) => {
                        scoped_containers.insert(container);
                    }
                    Err(_) => invalid_hierarchy = true,
                }
            }
        }
        report
            .candidates
            .get_mut(&role)
            .unwrap()
            .push(BoneCandidate {
                node: node.id,
                name: node.name.clone(),
                confidence,
                reason: reason.into(),
            });
    }
    if invalid_hierarchy {
        report
            .warnings
            .push("Некорректная иерархия: проверьте родителей костей перед назначением".into());
    }
    let multiple_characters = scoped_containers.len() > 1;
    if multiple_characters {
        report.warnings.push("В области поиска несколько скелетов или несвязанных цепей. Выберите корень одного персонажа либо назначьте роли вручную; автоматические назначения отключены".into());
    }
    refine_auto_rig_pro(project, &mut report);
    // A control/driver chain may coexist with the actual deform chain in ARP
    // exports. Only real nonzero vertex weights can break that name tie.
    let ambiguous: BTreeSet<_> = report
        .candidates
        .values()
        .flat_map(|candidates| {
            let high: Vec<_> = candidates
                .iter()
                .filter(|c| c.confidence == BoneConfidence::High)
                .map(|c| c.node)
                .collect();
            if high.len() > 1 {
                high
            } else {
                Vec::new()
            }
        })
        .collect();
    let weighted = weighted_joints(project, &ambiguous);
    let mut duplicate_roles = Vec::new();
    for role in BoneRole::ALL {
        let candidates = report.candidates.get_mut(&role).unwrap();
        let high_count = candidates
            .iter()
            .filter(|c| c.confidence == BoneConfidence::High)
            .count();
        let weighted_high: Vec<_> = candidates
            .iter()
            .filter(|c| c.confidence == BoneConfidence::High && weighted.contains(&c.node))
            .map(|c| c.node)
            .collect();
        if high_count > 1 && weighted_high.is_empty() {
            // An unskinned ARP control-rig/manifest often contains both the
            // ordinary FK chain and detached stretch followers. Retain the
            // established FK alias until weights provide deform-chain evidence.
            let plain: BTreeSet<_> = candidates
                .iter()
                .filter(|c| c.confidence == BoneConfidence::High)
                .map(|c| bare_name(&c.name))
                .collect();
            for candidate in candidates
                .iter_mut()
                .filter(|c| c.confidence == BoneConfidence::High)
            {
                let name = bare_name(&candidate.name);
                if name.contains("_stretch.") && plain.contains(&name.replacen("_stretch.", ".", 1))
                {
                    candidate.confidence = BoneConfidence::Review;
                    candidate.reason = "Auto-Rig Pro: есть основная FK-цепь и stretch-цепь без весов меша; подтвердите выбор деформирующей кости вручную".into();
                }
            }
        }
        if high_count > 1 && weighted_high.len() == 1 {
            for candidate in candidates
                .iter_mut()
                .filter(|c| c.confidence == BoneConfidence::High && c.node != weighted_high[0])
            {
                candidate.confidence = BoneConfidence::Review;
                candidate.reason = "Есть однозначная кость этой роли с ненулевыми весами; этот кандидат не деформирует меш".into();
            }
        }
        candidates.sort_by(|a, b| {
            a.confidence
                .cmp(&b.confidence)
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.node.cmp(&b.node))
        });
        let high: Vec<_> = candidates
            .iter()
            .filter(|c| c.confidence == BoneConfidence::High)
            .collect();
        if high.len() == 1 && !multiple_characters && !invalid_hierarchy {
            report.resolved.assignments.insert(role, high[0].node);
        } else if high.len() > 1 {
            duplicate_roles.push(role.label());
        }
    }
    if !duplicate_roles.is_empty() {
        report.warnings.push(format!(
            "Несколько подходящих костей: {}. Выберите соответствие вручную",
            duplicate_roles.join(", ")
        ));
    }
    if let Err(error) = report.resolved.validate(project) {
        report.resolved.assignments.clear();
        report
            .warnings
            .push(format!("Автоматические назначения отклонены: {error}"));
    }
    // Two uniquely named role candidates can still belong to incompatible chains.
    // Preserve their anatomical suggestions but explicitly prohibit automatic IK.
    for role in [
        BoneRole::LeftHand,
        BoneRole::RightHand,
        BoneRole::LeftFoot,
        BoneRole::RightFoot,
    ] {
        let roles = role.chain_roles().unwrap();
        if roles
            .iter()
            .all(|r| report.resolved.assignments.contains_key(r))
            && report.resolved.chain(role, project).is_err()
        {
            report.warnings.push(format!(
                "{}: имена распознаны, но прямой цепи IK нет",
                role.label()
            ));
        }
    }
    if report.resolved.assignments.is_empty() && !multiple_characters {
        report.warnings.push(
            "Уверенных соответствий нет. Назначьте роли вручную; произвольные имена не угадываются"
                .into(),
        );
    }
    report
}

fn weighted_joints(project: &Project, wanted: &BTreeSet<NodeId>) -> BTreeSet<NodeId> {
    let mut result = BTreeSet::new();
    if wanted.is_empty() {
        return result;
    }
    for node in &project.nodes {
        let (Some(skin), Some(mesh)) = (&node.skin, node.mesh.and_then(|id| project.mesh(id)))
        else {
            continue;
        };
        let mut pending: Vec<_> = skin
            .joints
            .iter()
            .map(|id| wanted.contains(id) && !result.contains(id))
            .collect();
        let mut remaining = pending.iter().filter(|&&value| value).count();
        if remaining == 0 {
            continue;
        }
        for vertex in &mesh.vertices {
            for (&index, &weight) in vertex.joints.iter().zip(&vertex.weights) {
                if weight > 0. && weight.is_finite() {
                    if let Some(slot) = pending.get_mut(index as usize).filter(|slot| **slot) {
                        result.insert(skin.joints[index as usize]);
                        *slot = false;
                        remaining -= 1;
                    }
                }
            }
            if remaining == 0 {
                break;
            }
        }
        if result.len() == wanted.len() {
            break;
        }
    }
    result
}

fn bare_name(name: &str) -> String {
    name.rsplit([':', '|', '/'])
        .next()
        .unwrap_or(name)
        .trim()
        .to_lowercase()
}

/// ARP's spine count is configurable; its top deforming spine is the chest.
/// This convention is applied only inside a recognizable root.x/spine_01.x rig.
fn refine_auto_rig_pro(project: &Project, report: &mut DetectionReport) {
    let Some(root) = report.root else {
        return;
    };
    let joints: Vec<_> = project
        .nodes
        .iter()
        .filter(|n| n.is_joint && is_descendant(project, n.id, root).unwrap_or(false))
        .collect();
    let named: Vec<_> = joints.iter().map(|n| (n.id, bare_name(&n.name))).collect();
    if !named.iter().any(|(_, n)| n == "root.x") || !named.iter().any(|(_, n)| n == "spine_01.x") {
        return;
    }
    let spine: Vec<_> = named
        .iter()
        .filter_map(|(id, name)| {
            let index = name
                .strip_prefix("spine_")?
                .strip_suffix(".x")?
                .parse::<u32>()
                .ok()?;
            (2..=32).contains(&index).then_some((*id, index))
        })
        .collect();
    let Some(last) = spine.iter().map(|(_, index)| *index).max() else {
        return;
    };
    let candidates = report.candidates.get_mut(&BoneRole::Chest).unwrap();
    for &(id, index) in &spine {
        if index != last {
            if let Some(candidate) = candidates.iter_mut().find(|c| c.node == id) {
                candidate.confidence = BoneConfidence::Review;
                candidate.reason = "Auto-Rig Pro: промежуточный сегмент позвоночника".into();
            }
            continue;
        }
        let reason = "Auto-Rig Pro: верхняя деформирующая кость позвоночника; проверьте на модели"
            .to_string();
        if let Some(candidate) = candidates.iter_mut().find(|c| c.node == id) {
            candidate.confidence = BoneConfidence::High;
            candidate.reason = reason;
        } else if let Some(node) = project.node(id) {
            candidates.push(BoneCandidate {
                node: id,
                name: node.name.clone(),
                confidence: BoneConfidence::High,
                reason,
            });
        }
    }
}

pub(crate) fn is_descendant(
    project: &Project,
    id: NodeId,
    root: NodeId,
) -> Result<bool, CoreError> {
    let mut cursor = Some(id);
    let mut visited = BTreeSet::new();
    while let Some(id) = cursor {
        if !visited.insert(id) {
            return Err(CoreError::HierarchyCycle(id));
        }
        let node = project.node(id).ok_or(CoreError::MissingNode(id))?;
        if id == root {
            return Ok(true);
        }
        cursor = node.parent;
    }
    Ok(false)
}

fn joint_container(project: &Project, id: NodeId) -> Result<NodeId, CoreError> {
    let mut current = id;
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(current) {
            return Err(CoreError::HierarchyCycle(current));
        }
        let node = project
            .node(current)
            .ok_or(CoreError::MissingNode(current))?;
        let Some(parent) = node.parent else {
            return Ok(current);
        };
        let parent_node = project.node(parent).ok_or(CoreError::MissingNode(parent))?;
        if !parent_node.is_joint {
            return Ok(parent);
        }
        current = parent;
    }
}

fn classify_name(name: &str) -> Option<(BoneRole, BoneConfidence, &'static str)> {
    let mut stem = bare_name(name);
    // Namespace-free FBX exports sometimes concatenate the Mixamo namespace.
    if let Some(rest) = stem.strip_prefix("mixamorig") {
        stem = rest
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '_' || c == ':')
            .to_owned();
    }
    let rigify = stem.starts_with("def-");
    let mut helper = false;
    for prefix in ["def-", "org-", "mch-", "c_", "c-", "ctrl_", "control_"] {
        if let Some(rest) = stem.strip_prefix(prefix) {
            helper = prefix != "def-";
            stem = rest.to_owned();
            break;
        }
    }
    let mut segmented = false;
    // A numbered segment after a side token may be a B-bone/twist segment.
    for marker in [".l.", ".r.", "_l.", "_r."] {
        if let Some(index) = stem.rfind(marker) {
            let tail = &stem[index + marker.len()..];
            if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                stem.truncate(index + marker.len() - 1);
                segmented = true;
                break;
            }
        }
    }
    let root_x = stem == "root.x";
    if stem.ends_with(".x") {
        stem.truncate(stem.len() - 2);
    }
    let mut side = None;
    for (marker, found) in [
        (".l", BoneSide::Left),
        ("_l", BoneSide::Left),
        ("-l", BoneSide::Left),
        (".r", BoneSide::Right),
        ("_r", BoneSide::Right),
        ("-r", BoneSide::Right),
    ] {
        if stem.ends_with(marker) {
            stem.truncate(stem.len() - marker.len());
            side = Some(found);
            break;
        }
    }
    if side.is_none() {
        for (marker, found) in [
            ("left", BoneSide::Left),
            ("right", BoneSide::Right),
            ("l_", BoneSide::Left),
            ("r_", BoneSide::Right),
        ] {
            if let Some(rest) = stem.strip_prefix(marker) {
                stem = rest.to_owned();
                side = Some(found);
                break;
            }
        }
    }
    if side.is_none() {
        for (marker, found) in [("left", BoneSide::Left), ("right", BoneSide::Right)] {
            if stem.ends_with(marker) {
                stem.truncate(stem.len() - marker.len());
                side = Some(found);
                break;
            }
        }
    }
    let helper_tokens = [
        "ref", "twist", "ik", "fk", "pole", "stretch", "cns", "nostr", "twk", "master", "offset",
        "scale", "fix", "track", "roll", "control", "ctrl",
    ];
    // These exact ARP names are primary deform bones, not stretch controllers.
    // Keep c_*, twist/rotated/numbered variants and arbitrary *_stretch helpers cautious.
    let arp_deform = !helper
        && side.is_some()
        && matches!(
            stem.as_str(),
            "arm_stretch" | "forearm_stretch" | "thigh_stretch" | "leg_stretch"
        );
    let tokens: Vec<_> = stem
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    helper |= tokens
        .iter()
        .any(|token| helper_tokens.contains(token) && !(*token == "stretch" && arp_deform));
    let base = tokens
        .into_iter()
        .filter(|token| !helper_tokens.contains(token))
        .collect::<String>();
    let confidence = if helper {
        BoneConfidence::Ignored
    } else if segmented {
        BoneConfidence::Review
    } else {
        BoneConfidence::High
    };
    let (role, cautious) = if let Some(side) = side {
        let left = side == BoneSide::Left;
        let role = match base.as_str() {
            "upperarm" | "arm" | "humerus" | "плечо" => {
                if left {
                    BoneRole::LeftUpperArm
                } else {
                    BoneRole::RightUpperArm
                }
            }
            "forearm" | "lowerarm" | "elbow" | "локоть" | "предплечье" => {
                if left {
                    BoneRole::LeftForearm
                } else {
                    BoneRole::RightForearm
                }
            }
            "hand" | "wrist" | "кисть" => {
                if left {
                    BoneRole::LeftHand
                } else {
                    BoneRole::RightHand
                }
            }
            "thigh" | "upleg" | "upperleg" | "бедро" => {
                if left {
                    BoneRole::LeftThigh
                } else {
                    BoneRole::RightThigh
                }
            }
            "shin" | "leg" | "lowerleg" | "calf" | "knee" | "колено" | "голень" => {
                if left {
                    BoneRole::LeftShin
                } else {
                    BoneRole::RightShin
                }
            }
            "foot" | "ankle" | "стопа" => {
                if left {
                    BoneRole::LeftFoot
                } else {
                    BoneRole::RightFoot
                }
            }
            _ => return None,
        };
        (role, false)
    } else {
        match base.as_str() {
            "hips" | "hip" | "pelvis" | "таз" => (BoneRole::Pelvis, false),
            "root" => (BoneRole::Pelvis, !root_x),
            "spine" if rigify => (BoneRole::Pelvis, true),
            "spine001" if rigify => (BoneRole::Spine, true),
            "spine002" | "spine003" if rigify => (BoneRole::Chest, true),
            "spine004" if rigify => (BoneRole::Neck, true),
            "spine005" | "spine006" if rigify => (BoneRole::Head, true),
            "spine" | "spine01" | "позвоночник" => (BoneRole::Spine, false),
            "chest" | "upperchest" | "thorax" | "spine2" | "spine03" | "грудь" => {
                (BoneRole::Chest, false)
            }
            "spine1" | "spine02" => (BoneRole::Chest, true),
            "neck" | "neck01" | "шея" => (BoneRole::Neck, false),
            "head" | "голова" => (BoneRole::Head, false),
            _ => return None,
        }
    };
    let confidence = if cautious && confidence == BoneConfidence::High {
        BoneConfidence::Review
    } else {
        confidence
    };
    let reason = match confidence {
        BoneConfidence::High if arp_deform => "Auto-Rig Pro: основная деформирующая stretch-кость; проверьте соответствие на модели",
        BoneConfidence::High => "Точное известное имя и обозначение стороны; проверьте соответствие на модели",
        BoneConfidence::Review => "Имя может обозначать сегмент позвоночника, корень или разделённую кость; требуется ручное подтверждение",
        BoneConfidence::Ignored => "Имя содержит признак управляющей, reference, IK/FK или twist-кости; автоматическое назначение отключено",
    };
    Some((role, confidence, reason))
}
