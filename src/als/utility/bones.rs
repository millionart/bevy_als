pub const ALS_KEY_BONES: [&str; 30] = [
    "root",
    "pelvis",
    "spine_01",
    "spine_02",
    "spine_03",
    "neck_01",
    "head",
    "clavicle_l",
    "upperarm_l",
    "lowerarm_l",
    "hand_l",
    "clavicle_r",
    "upperarm_r",
    "lowerarm_r",
    "hand_r",
    "thigh_l",
    "calf_l",
    "foot_l",
    "ball_l",
    "thigh_r",
    "calf_r",
    "foot_r",
    "ball_r",
    "ik_foot_root",
    "ik_foot_l",
    "ik_foot_r",
    "ik_hand_root",
    "ik_hand_gun",
    "ik_hand_l",
    "ik_hand_r",
];

pub const VIRTUAL_BONES: [&str; 11] = [
    "VB foot_root",
    "VB foot_l",
    "VB foot_r",
    "VB hand_l_to_hand_r",
    "VB hand_r",
    "VB hand_l_to_ik_hand_gun",
    "VB ik_hand_r",
    "VB hand_r_to_hand_r",
    "VB hand_l",
    "VB hand_r_to_ik_hand_gun",
    "VB ik_hand_l",
];

#[cfg(test)]
mod tests {
    use super::{ALS_KEY_BONES, VIRTUAL_BONES};
    use std::collections::HashSet;

    #[test]
    fn key_bones_non_empty() {
        assert!(!ALS_KEY_BONES.is_empty());
        assert!(ALS_KEY_BONES.iter().all(|name| !name.is_empty()));
    }

    #[test]
    fn virtual_bones_non_empty() {
        assert!(!VIRTUAL_BONES.is_empty());
        assert!(VIRTUAL_BONES.iter().all(|name| !name.is_empty()));
    }

    #[test]
    fn key_bones_unique() {
        let set: HashSet<_> = ALS_KEY_BONES.iter().collect();
        assert_eq!(set.len(), ALS_KEY_BONES.len());
    }

    #[test]
    fn virtual_bones_unique() {
        let set: HashSet<_> = VIRTUAL_BONES.iter().collect();
        assert_eq!(set.len(), VIRTUAL_BONES.len());
    }

    #[test]
    fn key_and_virtual_bones_disjoint() {
        let key: HashSet<_> = ALS_KEY_BONES.iter().collect();
        for name in VIRTUAL_BONES {
            assert!(!key.contains(&name), "duplicate bone {name} in both arrays");
        }
    }

    #[test]
    fn virtual_bones_prefixed() {
        for name in VIRTUAL_BONES {
            assert!(
                name.starts_with("VB "),
                "virtual bone {name} lacks the ALS 'VB ' prefix"
            );
        }
    }
}
