use bevy_animation_graph::core::edge_data::bone_mask::{BoneMask, BoneMaskType};
use bevy_animation_graph::core::id::BoneId;
use bevy_animation_graph::core::skeleton::Skeleton;

pub const TORCH_RIGHT_HAND_BONES: &[&str] = &[
    "spine_02",
    "spine_03",
    "clavicle_r",
    "upperarm_r",
    "lowerarm_r",
    "hand_r",
];

pub const LOOK_SPINE_BONES: &[&str] = &["spine_01", "spine_02", "spine_03", "neck_01"];

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
            parts: parts.iter().map(|p| Name::from(*p)).collect(),
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
