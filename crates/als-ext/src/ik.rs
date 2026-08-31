use bevy::ecs::system::Commands;
use bevy::math::{Quat, Vec3};
use bevy::prelude::{ChildOf, Component, Name, Transform};

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct IkHandTarget;

pub const IK_HAND_GUN_NAME: &str = "ik_hand_gun";

pub fn spawn_ik_hand_gun(
    cmds: &mut Commands,
    parent_bone: bevy::prelude::Entity,
) -> bevy::prelude::Entity {
    cmds.spawn((
        IkHandTarget,
        Name::new(IK_HAND_GUN_NAME),
        Transform::from_xyz(0.10, 0.02, 0.25),
        ChildOf(parent_bone),
    ))
    .id()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoBoneSolve {
    pub hip_rotation_world: Quat,
    pub knee_rotation_world: Quat,
    pub knee_position: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleVector {
    pub projection: Vec3,
    pub direction: Vec3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StableTwoBonePole {
    reference_local: Vec3,
    last_world: Vec3,
    initialized: bool,
}

const MIN_DISTANCE: f32 = 1e-4;

fn clamp_reach(distance: f32, upper_len: f32, lower_len: f32) -> f32 {
    let max = (upper_len + lower_len).max(2.0 * MIN_DISTANCE) - 1e-3;
    let min = ((upper_len - lower_len).abs()).max(MIN_DISTANCE);
    distance.clamp(min, max.max(min))
}

fn perpendicular_fallback(axis: Vec3, pole: Vec3) -> Vec3 {
    let candidate = pole - axis * pole.dot(axis);
    if candidate.length_squared() > 1e-8 {
        candidate.normalize()
    } else {
        let helper = if axis.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        axis.cross(helper).normalize()
    }
}

pub fn calculate_pole_vector(hip: Vec3, knee: Vec3, ankle: Vec3) -> Option<PoleVector> {
    let hip_to_knee = knee - hip;
    if hip_to_knee.length_squared() <= 1e-8 {
        return None;
    }
    let hip_to_ankle = ankle - hip;
    let Some(axis) = hip_to_ankle.try_normalize() else {
        return Some(PoleVector {
            projection: hip,
            direction: hip_to_knee.normalize(),
        });
    };
    let projection = hip + axis * hip_to_knee.dot(axis);
    let direction = (knee - projection).try_normalize()?;
    Some(PoleVector {
        projection,
        direction,
    })
}

fn projected_direction(direction: Vec3, axis: Vec3) -> Option<Vec3> {
    (direction - axis * direction.dot(axis)).try_normalize()
}

impl StableTwoBonePole {
    pub fn resolve(
        &mut self,
        armature_rotation: Quat,
        hip: Vec3,
        knee: Vec3,
        ankle: Vec3,
        target: Vec3,
        fallback_world: Vec3,
    ) -> Vec3 {
        let axis = (target - hip)
            .try_normalize()
            .or_else(|| (ankle - hip).try_normalize())
            .unwrap_or(Vec3::Y);
        let current = calculate_pole_vector(hip, knee, ankle).map(|pole| pole.direction);
        if !self.initialized {
            let reference_world = current
                .and_then(|direction| projected_direction(direction, axis))
                .or_else(|| projected_direction(fallback_world, axis))
                .unwrap_or_else(|| perpendicular_fallback(axis, fallback_world));
            self.reference_local = armature_rotation.inverse() * reference_world;
            self.last_world = reference_world;
            self.initialized = true;
        }
        let reference_world = projected_direction(armature_rotation * self.reference_local, axis);
        let last_world = projected_direction(self.last_world, axis);
        let current_world = current.and_then(|direction| projected_direction(direction, axis));
        let fallback_world = projected_direction(fallback_world, axis);
        let mut resolved = reference_world
            .or(last_world)
            .or(current_world)
            .or(fallback_world)
            .unwrap_or_else(|| perpendicular_fallback(axis, Vec3::Y));
        if let Some(last_world) = last_world
            && resolved.dot(last_world) < 0.0
        {
            resolved = -resolved;
        }
        self.last_world = resolved;
        resolved
    }

    pub fn last_direction_world(&self) -> Vec3 {
        self.last_world
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

pub fn solve_two_bone_ik(
    hip: Vec3,
    knee: Vec3,
    ankle: Vec3,
    target: Vec3,
    upper_len: f32,
    lower_len: f32,
    pole: Vec3,
) -> Option<TwoBoneSolve> {
    if upper_len <= MIN_DISTANCE || lower_len <= MIN_DISTANCE {
        return None;
    }

    let to_target = target - hip;
    let raw_distance = to_target.length();
    if raw_distance <= MIN_DISTANCE || !raw_distance.is_finite() {
        return None;
    }
    let axis = to_target / raw_distance;
    let distance = clamp_reach(raw_distance, upper_len, lower_len);
    let reach_point = hip + axis * distance;

    let cos_hip = (distance * distance + upper_len * upper_len - lower_len * lower_len)
        / (2.0 * distance * upper_len);
    let sin_hip = (1.0 - cos_hip * cos_hip).clamp(0.0, 1.0).sqrt();
    let side = perpendicular_fallback(axis, pole);

    let upper_dir = axis * cos_hip + side * sin_hip;
    let knee_position = hip + upper_dir * upper_len;

    let cur_upper_dir = (knee - hip).normalize_or_zero();
    if cur_upper_dir.length_squared() < 0.5 {
        return None;
    }
    let hip_rotation_world = Quat::from_rotation_arc(cur_upper_dir, upper_dir.normalize());

    let cur_lower_dir = (ankle - knee).normalize_or_zero();
    let new_lower_dir = (reach_point - knee_position).normalize_or_zero();
    if cur_lower_dir.length_squared() < 0.5 || new_lower_dir.length_squared() < 0.5 {
        return None;
    }
    let knee_rotation_world = Quat::from_rotation_arc(cur_lower_dir, new_lower_dir);

    Some(TwoBoneSolve {
        hip_rotation_world,
        knee_rotation_world,
        knee_position,
    })
}

pub fn world_rotation_to_local(parent_world: Quat, world_rotation: Quat) -> Quat {
    parent_world.inverse() * world_rotation
}

pub fn blend_ik_rotation(current: Quat, target: Quat, weight: f32) -> Quat {
    current.slerp(target, weight.clamp(0.0, 1.0))
}

pub fn child_world_rotation_after_parent_rotation(
    parent_rotation_delta: Quat,
    child_world_rotation: Quat,
    current_child_axis_world: Vec3,
    target_child_axis_world: Vec3,
) -> Option<Quat> {
    let current_after_parent =
        (parent_rotation_delta * current_child_axis_world).try_normalize()?;
    let target_child_axis_world = target_child_axis_world.try_normalize()?;
    let correction = Quat::from_rotation_arc(current_after_parent, target_child_axis_world);
    Some((correction * parent_rotation_delta * child_world_rotation).normalize())
}

#[cfg(test)]
mod hand_target_tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::*;

    #[test]
    fn spawn_parents_named_attach_point_under_hand_bone() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let hand = app
            .world_mut()
            .spawn((Name::new("hand_r"), Transform::IDENTITY))
            .id();
        let target = {
            let mut cmds = app.world_mut().commands();
            spawn_ik_hand_gun(&mut cmds, hand)
        };
        app.update();

        assert_eq!(
            app.world().get::<Name>(target).unwrap().as_str(),
            IK_HAND_GUN_NAME
        );
        assert!(app.world().get::<IkHandTarget>(target).is_some());
        let child_of = app.world().get::<ChildOf>(target).unwrap();
        assert_eq!(child_of.0, hand, "attach point parented to hand bone");
        assert!(
            app.world()
                .get::<Transform>(target)
                .unwrap()
                .translation
                .distance(Vec3::new(0.10, 0.02, 0.25))
                < 1e-5
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIP: Vec3 = Vec3::ZERO;
    const UPPER: f32 = 0.45;
    const LOWER: f32 = 0.45;

    fn straight_chain() -> (Vec3, Vec3) {
        (
            Vec3::new(0.0, -UPPER, 0.0),
            Vec3::new(0.0, -(UPPER + LOWER), 0.0),
        )
    }

    #[test]
    fn reachable_target_is_hit_exactly() {
        let (knee, ankle) = straight_chain();
        let target = Vec3::new(0.15, -0.80, 0.05);
        let solve = solve_two_bone_ik(HIP, knee, ankle, target, UPPER, LOWER, Vec3::Z).unwrap();

        let d_knee = (solve.knee_position - HIP).length();
        let d_end = (target - solve.knee_position).length();
        assert!((d_knee - UPPER).abs() < 1e-4, "upper length preserved");
        assert!(
            (d_end - LOWER).abs() < 1e-3,
            "lower must land on target within eps, got {d_end}"
        );
    }

    #[test]
    fn knee_displaces_toward_pole_side() {
        let (knee, ankle) = straight_chain();
        let target = Vec3::new(0.0, -1.60, 0.0);
        let solve_fwd = solve_two_bone_ik(HIP, knee, ankle, target, UPPER, LOWER, Vec3::Z).unwrap();
        assert!(solve_fwd.knee_position.z > 1e-3, "bend toward +Z pole");
        assert!(solve_fwd.knee_position.y > target.y);
    }

    #[test]
    fn stable_poles_preserve_mirrored_reference_planes() {
        let left_hip = Vec3::new(-0.2, 0.0, 0.0);
        let left_knee = Vec3::new(-0.2, -0.4, 0.16);
        let left_ankle = Vec3::new(-0.2, -0.8, 0.0);
        let right_hip = Vec3::new(0.2, 0.0, 0.0);
        let right_knee = Vec3::new(0.2, -0.4, -0.16);
        let right_ankle = Vec3::new(0.2, -0.8, 0.0);
        let mut left = StableTwoBonePole::default();
        let mut right = StableTwoBonePole::default();
        let left_direction = left.resolve(
            Quat::IDENTITY,
            left_hip,
            left_knee,
            left_ankle,
            left_ankle + Vec3::new(0.04, 0.0, 0.02),
            Vec3::Z,
        );
        let right_direction = right.resolve(
            Quat::IDENTITY,
            right_hip,
            right_knee,
            right_ankle,
            right_ankle + Vec3::new(-0.04, 0.0, 0.02),
            Vec3::Z,
        );

        assert!(left.is_initialized() && right.is_initialized());
        assert!(left_direction.dot(right_direction) < -0.8);
    }

    #[test]
    fn stable_pole_reuses_the_last_plane_when_the_chain_is_collinear() {
        let hip = Vec3::ZERO;
        let knee = Vec3::new(0.0, -0.4, 0.14);
        let ankle = Vec3::new(0.0, -0.8, 0.0);
        let mut pole = StableTwoBonePole::default();
        let first = pole.resolve(Quat::IDENTITY, hip, knee, ankle, ankle, Vec3::Z);
        let second = pole.resolve(
            Quat::IDENTITY,
            hip,
            Vec3::new(0.0, -0.4, 0.0),
            ankle,
            ankle,
            Vec3::Z,
        );

        assert!(first.dot(second) > 0.999);
        assert!(pole.last_direction_world().dot(first) > 0.999);
    }

    #[test]
    fn rotations_map_current_dirs_onto_solved_dirs() {
        let (knee, ankle) = straight_chain();
        let target = Vec3::new(-0.12, -0.78, 0.08);
        let solve = solve_two_bone_ik(HIP, knee, ankle, target, UPPER, LOWER, Vec3::NEG_Y).unwrap();
        let cur_upper = (knee - HIP).normalize();
        let new_upper = (solve.knee_position - HIP).normalize();
        let rotated = solve.hip_rotation_world * cur_upper;
        assert!((rotated - new_upper).length() < 1e-4);

        let cur_lower = (ankle - knee).normalize();
        let new_lower = (target - solve.knee_position).normalize();
        let rotated_lower = solve.knee_rotation_world * cur_lower;
        assert!((rotated_lower - new_lower).length() < 1e-3);
    }

    #[test]
    fn unreachable_target_clamps_to_full_extension() {
        let (knee, ankle) = straight_chain();
        let far = Vec3::new(0.0, -50.0, 0.0);
        let solve = solve_two_bone_ik(HIP, knee, ankle, far, UPPER, LOWER, Vec3::Z).unwrap();
        let axis = (far - HIP).normalize();
        let expected_knee = HIP + axis * UPPER;
        assert!(
            (solve.knee_position - expected_knee).length() < 0.05,
            "near-straight chain points at target, got {:?}",
            solve.knee_position
        );
    }

    #[test]
    fn over_close_target_clamps_to_inner_limit() {
        let (knee, ankle) = straight_chain();
        let near = Vec3::new(0.0, -0.01, 0.0);
        let solve = solve_two_bone_ik(HIP, knee, ankle, near, UPPER, LOWER, Vec3::Z).unwrap();
        let d = (solve.knee_position - HIP).length();
        assert!((d - UPPER).abs() < 1e-3);
        let end_dist = (near - HIP).length();
        let min_reach = (UPPER - LOWER).abs().max(MIN_DISTANCE);
        assert!(end_dist >= min_reach - 1e-3);
    }

    #[test]
    fn already_satisfied_pose_yields_identity_rotations() {
        let knee = Vec3::new(0.0, -0.40, 0.2131);
        let ankle = knee + Vec3::new(0.0, -0.30, -0.3354).normalize_or_zero() * LOWER;
        let target = ankle;
        let solve = solve_two_bone_ik(HIP, knee, ankle, target, UPPER, LOWER, Vec3::Z).unwrap();
        assert!(
            solve.hip_rotation_world.angle_between(Quat::IDENTITY) < 0.03,
            "hip rot {:?}",
            solve.hip_rotation_world
        );
        assert!(
            solve.knee_rotation_world.angle_between(Quat::IDENTITY) < 0.03,
            "knee rot {:?}",
            solve.knee_rotation_world
        );
    }

    #[test]
    fn pole_parallel_to_axis_does_not_panic_and_still_solves() {
        let (knee, ankle) = straight_chain();
        let target = Vec3::new(0.0, -1.60, 0.0);
        let solve = solve_two_bone_ik(HIP, knee, ankle, target, UPPER, LOWER, Vec3::NEG_Y).unwrap();
        assert!((solve.knee_position - HIP).length() - UPPER < 1e-4);
    }

    #[test]
    fn degenerate_inputs_return_none() {
        let (_, ankle) = straight_chain();
        assert!(
            solve_two_bone_ik(HIP, Vec3::ZERO, ankle, Vec3::ONE, UPPER, LOWER, Vec3::Z).is_none()
        );
        let clamped = solve_two_bone_ik(
            HIP,
            Vec3::Y,
            ankle,
            Vec3::new(0.001, -0.02, 0.0),
            UPPER,
            LOWER,
            Vec3::Z,
        );
        assert!(clamped.is_some(), "inner clamp keeps the chain solvable");
        assert!(
            solve_two_bone_ik(
                HIP,
                Vec3::Y,
                ankle,
                Vec3::new(0.0, -1.0, 0.0),
                0.0,
                LOWER,
                Vec3::Z
            )
            .is_none()
        );
    }

    #[test]
    fn world_to_local_conversion_round_trips() {
        let parent = Quat::from_rotation_y(0.7);
        let world = Quat::from_rotation_x(0.3);
        let local = world_rotation_to_local(parent, world);
        let rebuilt = parent * local;
        assert!(rebuilt.angle_between(world) < 1e-5);
    }

    #[test]
    fn rotation_weight_blends_continuously_between_animation_and_ik() {
        let animation = Quat::from_rotation_y(-0.4);
        let ik = Quat::from_rotation_y(0.8);
        let zero = blend_ik_rotation(animation, ik, 0.0);
        let half = blend_ik_rotation(animation, ik, 0.5);
        let full = blend_ik_rotation(animation, ik, 1.0);

        assert!(zero.dot(animation).abs() > 0.999_99);
        assert!(full.dot(ik).abs() > 0.999_99);
        assert!(half.angle_between(animation) > 0.1);
        assert!(half.angle_between(ik) > 0.1);
    }

    #[test]
    fn child_rotation_compensates_for_the_parent_solve_before_reaching_target_axis() {
        let parent_delta = Quat::from_rotation_z(0.35);
        let child_rotation = Quat::IDENTITY;
        let local_axis = Vec3::NEG_Y;
        let target_axis = Vec3::new(0.25, -0.9, 0.35).normalize();
        let corrected = child_world_rotation_after_parent_rotation(
            parent_delta,
            child_rotation,
            local_axis,
            target_axis,
        )
        .unwrap();

        let actual_axis = corrected * local_axis;
        assert!(actual_axis.dot(target_axis) > 0.999_99);
    }
}
