use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use crate::character::AlsCharacter;

pub const FOOT_BONE_NAMES: [&str; 2] = ["foot_l", "foot_r"];
pub const PROBE_UP_OFFSET: f32 = 0.15;
pub const PROBE_DEPTH: f32 = 0.85;
pub const ANKLE_TO_SOLE: f32 = 0.09;
pub const MAX_FOOT_REACH: f32 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsGroundHit {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsFootProbe {
    pub left: Option<AlsGroundHit>,
    pub right: Option<AlsGroundHit>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsFootBones {
    pub left: Entity,
    pub right: Entity,
}

pub fn foot_target_offset(ankle_y: f32, hit: &AlsGroundHit) -> f32 {
    let raw = hit.point.y + ANKLE_TO_SOLE - ankle_y;
    raw.clamp(-MAX_FOOT_REACH, MAX_FOOT_REACH)
}

pub fn foot_ik_target(hit: &AlsGroundHit) -> Vec3 {
    hit.point + hit.normal * 0.0 + Vec3::Y * ANKLE_TO_SOLE
}

pub fn slope_is_walkable(normal: Vec3, max_slope_degrees: f32) -> bool {
    let up_dot = normal.normalize_or_zero().dot(Vec3::Y);
    up_dot >= max_slope_degrees.to_radians().cos()
}

fn collect_named_descendants(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
    wanted: &[&str],
    out: &mut Vec<(String, Entity)>,
) {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        let Ok(children) = children_q.get(entity) else {
            continue;
        };
        for child in children.iter() {
            if let Ok(name) = name_q.get(child) {
                let n = name.as_str();
                if wanted.contains(&n) && !out.iter().any(|(s, _)| s == n) {
                    out.push((n.to_string(), child));
                }
            }
            stack.push(child);
        }
    }
}

pub fn build_foot_bone_map(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
) -> Option<AlsFootBones> {
    let mut found = Vec::new();
    collect_named_descendants(root, children_q, name_q, &FOOT_BONE_NAMES, &mut found);
    let left = found.iter().find(|(n, _)| n == "foot_l").map(|(_, e)| *e);
    let right = found.iter().find(|(n, _)| n == "foot_r").map(|(_, e)| *e);
    Some(AlsFootBones {
        left: left?,
        right: right?,
    })
}

pub fn foot_ground_probe_system(
    mut commands: Commands,
    spatial: SpatialQuery,
    characters: Query<(Entity, &GlobalTransform, &AlsFootBones), With<AlsCharacter>>,
    bones_tf: Query<&GlobalTransform, Without<AlsCharacter>>,
) {
    for (entity, char_tf, feet) in &characters {
        let origin_base = char_tf.translation();
        let mut probe = AlsFootProbe::default();
        for (bone, slot) in [(feet.left, 0usize), (feet.right, 1usize)] {
            let Ok(bone_tf) = bones_tf.get(bone) else {
                continue;
            };
            let ankle = bone_tf.translation();
            if (ankle - origin_base).length() > 3.0 {
                continue;
            }
            let origin = ankle + Vec3::Y * PROBE_UP_OFFSET;
            let filter = SpatialQueryFilter::from_excluded_entities([entity, bone]);
            if let Some(hit) = spatial.cast_ray(
                origin,
                Dir3::NEG_Y,
                PROBE_DEPTH,
                true,
                &filter,
            ) {
                let value = AlsGroundHit {
                    point: origin - Vec3::Y * hit.distance,
                    normal: hit.normal,
                };
                if slot == 0 {
                    probe.left = Some(value);
                } else {
                    probe.right = Some(value);
                }
            }
        }
        commands.entity(entity).insert(probe);
    }
}

pub fn foot_bone_map_system(
    mut commands: Commands,
    characters: Query<Entity, (With<AlsCharacter>, Without<AlsFootBones>)>,
    children_q: Query<&Children>,
    name_q: Query<&Name>,
) {
    for entity in &characters {
        if let Some(map) = build_foot_bone_map(entity, &children_q, &name_q) {
            commands.entity(entity).insert(map);
        }
    }
}

pub struct AlsFootProbePlugin;

impl Plugin for AlsFootProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, (foot_bone_map_system, foot_ground_probe_system).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_offset_positive_when_ground_above_ankle() {
        let hit = AlsGroundHit {
            point: Vec3::new(1.0, 0.20, 2.0),
            normal: Vec3::Y,
        };
        let off = foot_target_offset(0.10, &hit);
        assert!((off - (0.20 + ANKLE_TO_SOLE - 0.10)).abs() < 1e-5);
        assert!(off > 0.0);
    }

    #[test]
    fn target_offset_negative_on_step_down() {
        let hit = AlsGroundHit {
            point: Vec3::ZERO,
            normal: Vec3::Y,
        };
        let off = foot_target_offset(0.50, &hit);
        assert!((off - (ANKLE_TO_SOLE - 0.50)).abs() < 1e-5);
        assert!(off < 0.0);
    }

    #[test]
    fn offset_clamps_to_max_reach() {
        let deep = AlsGroundHit {
            point: Vec3::new(0.0, -5.0, 0.0),
            normal: Vec3::Y,
        };
        assert!((foot_target_offset(0.0, &deep) + MAX_FOOT_REACH).abs() < 1e-5);
        let high = AlsGroundHit {
            point: Vec3::new(0.0, 5.0, 0.0),
            normal: Vec3::Y,
        };
        assert!((foot_target_offset(0.0, &high) - MAX_FOOT_REACH).abs() < 1e-5);
    }

    #[test]
    fn ik_target_lifts_hit_by_sole_height() {
        let hit = AlsGroundHit {
            point: Vec3::new(1.0, 0.30, -2.0),
            normal: Vec3::Y,
        };
        let t = foot_ik_target(&hit);
        assert!((t.y - 0.30 - ANKLE_TO_SOLE).abs() < 1e-5);
        assert_eq!(t.x, 1.0);
        assert_eq!(t.z, -2.0);
    }

    #[test]
    fn slope_gate_matches_als_max_slope() {
        assert!(slope_is_walkable(Vec3::Y, 50.0));
        let flat45 = Vec3::new(1.0, 1.0, 0.0);
        assert!(slope_is_walkable(flat45, 50.0));
        let steep = Vec3::new(1.0, 0.5, 0.0);
        assert!(!slope_is_walkable(steep, 50.0));
    }

    #[test]
    fn zero_vector_normal_is_not_walkable() {
        assert!(!slope_is_walkable(Vec3::ZERO, 50.0));
    }
}
