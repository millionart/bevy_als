use bevy_animation_graph::core::edge_data::bone_mask::BoneMask;
use bevy_animation_graph::core::id::BoneId;
use bevy_animation_graph::core::skeleton::Skeleton;

#[cfg(test)]
use bevy_animation_graph::core::edge_data::bone_mask::BoneMaskType;

pub const TORCH_RIGHT_HAND_BONES: &[&str] = &[
    "spine_02",
    "spine_03",
    "clavicle_r",
    "upperarm_r",
    "lowerarm_r",
    "hand_r",
];

pub const TORCH_ARM_RIGHT_BONES: &[&str] = &["clavicle_r", "upperarm_r", "lowerarm_r", "hand_r"];
pub const TORCH_SPINE_UPPER_BONES: &[&str] = &["spine_02", "spine_03"];
pub const HEAD_LAYER_BONES: &[&str] = &["neck_01", "head"];

pub const LOOK_SPINE_BONES: &[&str] = &["spine_01", "spine_02", "spine_03", "neck_01"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BranchFilter {
    pub bone_name: &'static str,
    pub blend_depth: i32,
}

pub fn build_bone_mask(skeleton: &Skeleton, bone_names: &[&str], weight: f32) -> BoneMask {
    let mut mask = BoneMask::none();
    for path in skeleton.iter_paths() {
        let Some(last) = path.parts.last() else {
            continue;
        };
        if !bone_names.contains(&last.as_str()) {
            continue;
        }
        let id: BoneId = path.id();
        mask.add_bone_weight(id, path.clone(), weight);
    }
    mask
}

pub fn build_branch_bone_mask(skeleton: &Skeleton, branch_names: &[&str], weight: f32) -> BoneMask {
    let mut mask = BoneMask::none();
    let weight = weight.clamp(0.0, 1.0);
    for path in skeleton.iter_paths() {
        if !path
            .parts
            .iter()
            .any(|part| branch_names.contains(&part.as_str()))
        {
            continue;
        }
        let id: BoneId = path.id();
        mask.add_bone_weight(id, path.clone(), weight);
    }
    mask
}

pub fn build_branch_filter_bone_mask_by_name(
    skeleton: &Skeleton,
    filters: &[(&str, i32)],
) -> BoneMask {
    let paths = skeleton.iter_paths().cloned().collect::<Vec<_>>();
    let mut mask = BoneMask::none();
    for (bone_name, blend_depth) in filters {
        let Some(root) = paths.iter().find(|path| {
            path.parts
                .last()
                .is_some_and(|name| name.as_str() == *bone_name)
        }) else {
            continue;
        };
        let increase_per_depth = if *blend_depth == 0 {
            1.0
        } else {
            1.0 / *blend_depth as f32
        };
        for path in &paths {
            if path.parts.len() < root.parts.len()
                || path.parts[..root.parts.len()] != root.parts[..]
            {
                continue;
            }
            let depth = path.parts.len() - root.parts.len();
            let id = path.id();
            let weight =
                (mask.bone_weight(&id) + increase_per_depth * (depth + 1) as f32).clamp(0.0, 1.0);
            mask.add_bone_weight(id, path.clone(), weight);
        }
    }
    mask
}

pub fn build_branch_filter_bone_mask(skeleton: &Skeleton, filters: &[BranchFilter]) -> BoneMask {
    build_branch_filter_bone_mask_by_name(
        skeleton,
        &filters
            .iter()
            .map(|filter| (filter.bone_name, filter.blend_depth))
            .collect::<Vec<_>>(),
    )
}

pub fn build_grouped_bone_mask(skeleton: &Skeleton, groups: &[(&[&str], f32)]) -> BoneMask {
    let mut mask = BoneMask::none();
    for (bone_names, weight) in groups {
        if *weight <= 0.0 {
            continue;
        }
        let w = (*weight).min(1.0);
        for path in skeleton.iter_paths() {
            let Some(last) = path.parts.last() else {
                continue;
            };
            if !bone_names.contains(&last.as_str()) {
                continue;
            }
            let id: BoneId = path.id();
            mask.add_bone_weight(id, path.clone(), w);
        }
    }
    mask
}

pub fn mask_coverage(skeleton: &Skeleton, bone_names: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for path in skeleton.iter_paths() {
        let Some(last) = path.parts.last() else {
            continue;
        };
        if bone_names.contains(&last.as_str()) {
            let joined = path
                .parts
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join("/");
            found.push(joined);
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{Name, Transform};
    use bevy_animation_graph::core::animation_clip::EntityPath;

    fn path(parts: &[&str]) -> EntityPath {
        EntityPath {
            parts: parts.iter().map(|part| Name::new((*part).to_owned())).collect(),
        }
    }

    fn skeleton() -> Skeleton {
        let mut sk = Skeleton::default();
        for parts in [
            vec!["root"],
            vec!["root", "pelvis"],
            vec!["root", "pelvis", "spine_01"],
            vec!["root", "pelvis", "spine_01", "hand_r"],
            vec!["root", "pelvis", "thigh_l"],
            vec!["root", "pelvis", "thigh_l", "foot_l"],
        ] {
            sk.add_bone(path(&parts), Transform::IDENTITY, Transform::IDENTITY);
        }
        sk
    }

    #[test]
    fn listed_bones_get_requested_weight() {
        let sk = skeleton();
        let mask = build_bone_mask(&sk, &["spine_01", "hand_r"], 0.75);
        assert_eq!(mask.get_weights().len(), 2);
        let hand_id = path(&["root", "pelvis", "spine_01", "hand_r"]).id();
        assert!((mask.bone_weight(&hand_id) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn unlisted_bones_weight_zero_with_positive_base() {
        let sk = skeleton();
        let mask = build_bone_mask(&sk, &["hand_r"], 1.0);
        let foot_id = path(&["root", "pelvis", "thigh_l", "foot_l"]).id();
        assert!((mask.bone_weight(&foot_id)).abs() < 1e-6);
        assert!(matches!(mask.base(), BoneMaskType::Positive));
    }

    #[test]
    fn matching_is_exact_on_last_segment_only() {
        let sk = skeleton();
        let mask = build_bone_mask(&sk, &["pelvis"], 1.0);
        assert_eq!(mask.get_weights().len(), 1);
        let pelvis_id = path(&["root", "pelvis"]).id();
        assert!(mask.contains_bone(pelvis_id));
    }

    #[test]
    fn branch_mask_includes_descendants_of_the_branch_root() {
        let sk = skeleton();
        let mask = build_branch_bone_mask(&sk, &["thigh_l"], 1.0);
        let thigh = path(&["root", "pelvis", "thigh_l"]).id();
        let foot = path(&["root", "pelvis", "thigh_l", "foot_l"]).id();
        assert!((mask.bone_weight(&thigh) - 1.0).abs() < 1e-6);
        assert!((mask.bone_weight(&foot) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn branch_filter_matches_source_positive_and_negative_depth_rules() {
        let sk = skeleton();
        let mask = build_branch_filter_bone_mask(
            &sk,
            &[
                BranchFilter {
                    bone_name: "pelvis",
                    blend_depth: 0,
                },
                BranchFilter {
                    bone_name: "thigh_l",
                    blend_depth: -1,
                },
            ],
        );
        let spine = path(&["root", "pelvis", "spine_01"]).id();
        let thigh = path(&["root", "pelvis", "thigh_l"]).id();
        let foot = path(&["root", "pelvis", "thigh_l", "foot_l"]).id();
        assert_eq!(mask.bone_weight(&spine), 1.0);
        assert_eq!(mask.bone_weight(&thigh), 0.0);
        assert_eq!(mask.bone_weight(&foot), 0.0);
    }

    #[test]
    fn branch_filter_matches_source_gradient_depth_rule() {
        let sk = skeleton();
        let mask = build_branch_filter_bone_mask(
            &sk,
            &[BranchFilter {
                bone_name: "pelvis",
                blend_depth: 3,
            }],
        );
        let pelvis = path(&["root", "pelvis"]).id();
        let spine = path(&["root", "pelvis", "spine_01"]).id();
        assert!((mask.bone_weight(&pelvis) - 1.0 / 3.0).abs() < 1e-6);
        assert!((mask.bone_weight(&spine) - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn unknown_names_yield_empty_mask_without_panic() {
        let sk = skeleton();
        let mask = build_bone_mask(&sk, &["nonexistent_bone"], 1.0);
        assert_eq!(mask.get_weights().len(), 0);
    }

    #[test]
    fn coverage_reports_full_hierarchy_paths() {
        let sk = skeleton();
        let cov = mask_coverage(&sk, &["foot_l"]);
        assert_eq!(cov, vec!["root/pelvis/thigh_l/foot_l".to_string()]);
    }
}
