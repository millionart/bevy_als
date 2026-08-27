use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const UE_CM_TO_M: f32 = 0.01;

#[derive(Asset, Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[reflect(Default)]
pub struct AlsMovementParams {
    pub speeds: GaitSpeeds,
    pub acceleration: AccelerationParams,
    pub rotation: RotationParams,
    pub crouch: CrouchParams,
    pub jump: JumpParams,
}

impl AlsMovementParams {
    pub fn stance_speeds(&self, stance: crate::state::Stance) -> &StanceGaitSpeeds {
        match stance {
            crate::state::Stance::Standing => &self.speeds.standing,
            crate::state::Stance::Crouching => &self.speeds.crouching,
        }
    }

    pub fn apply_override(&mut self, ovr: &AlsMovementParamsOverride) {
        if let Some(v) = &ovr.speeds {
            self.speeds.apply_override(v);
        }
        if let Some(v) = &ovr.acceleration {
            self.acceleration.apply_override(v);
        }
        if let Some(v) = &ovr.rotation {
            self.rotation.apply_override(v);
        }
        if let Some(v) = &ovr.crouch {
            self.crouch.apply_override(v);
        }
        if let Some(v) = &ovr.jump {
            self.jump.apply_override(v);
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct GaitSpeeds {
    pub standing: StanceGaitSpeeds,
    pub crouching: StanceGaitSpeeds,
}

impl GaitSpeeds {
    pub fn apply_override(&mut self, ovr: &GaitSpeedsOverride) {
        if let Some(v) = &ovr.standing {
            self.standing.apply_override(v);
        }
        if let Some(v) = &ovr.crouching {
            self.crouching.apply_override(v);
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StanceGaitSpeeds {
    pub max_walk_speed: f32,
    pub max_run_speed: f32,
    pub max_sprint_speed: f32,
    pub animated_walk_speed: f32,
    pub animated_run_speed: f32,
    pub animated_sprint_speed: f32,
}

impl Default for StanceGaitSpeeds {
    fn default() -> Self {
        Self {
            max_walk_speed: 175.0 * UE_CM_TO_M,
            max_run_speed: 375.0 * UE_CM_TO_M,
            max_sprint_speed: 650.0 * UE_CM_TO_M,
            animated_walk_speed: 150.0 * UE_CM_TO_M,
            animated_run_speed: 350.0 * UE_CM_TO_M,
            animated_sprint_speed: 600.0 * UE_CM_TO_M,
        }
    }
}

impl StanceGaitSpeeds {
    pub fn apply_override(&mut self, ovr: &StanceGaitSpeedsOverride) {
        if let Some(v) = ovr.max_walk_speed {
            self.max_walk_speed = v;
        }
        if let Some(v) = ovr.max_run_speed {
            self.max_run_speed = v;
        }
        if let Some(v) = ovr.max_sprint_speed {
            self.max_sprint_speed = v;
        }
        if let Some(v) = ovr.animated_walk_speed {
            self.animated_walk_speed = v;
        }
        if let Some(v) = ovr.animated_run_speed {
            self.animated_run_speed = v;
        }
        if let Some(v) = ovr.animated_sprint_speed {
            self.animated_sprint_speed = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AccelerationParams {
    pub max_acceleration: f32,
    pub braking_deceleration: f32,
    pub ground_friction: f32,
}

impl Default for AccelerationParams {
    fn default() -> Self {
        Self {
            max_acceleration: 2000.0 * UE_CM_TO_M,
            braking_deceleration: 1500.0 * UE_CM_TO_M,
            ground_friction: 4.0,
        }
    }
}

impl AccelerationParams {
    pub fn apply_override(&mut self, ovr: &AccelerationParamsOverride) {
        if let Some(v) = ovr.max_acceleration {
            self.max_acceleration = v;
        }
        if let Some(v) = ovr.braking_deceleration {
            self.braking_deceleration = v;
        }
        if let Some(v) = ovr.ground_friction {
            self.ground_friction = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RotationParams {
    pub grounded_moving_velocity_direction: GroundedModeRotation,
    pub grounded_moving_view_direction: GroundedModeRotation,
    pub grounded_moving_aiming: FixedModeRotation,
    pub grounded_stationary_velocity_direction: FixedModeRotation,
    pub grounded_stationary_view_direction: GroundedModeRotation,
    pub _grounded_stationary_aiming_has_no_damping: (),
    pub in_air_half_life: f32,
    pub in_air_aiming_half_life: f32,
    pub fallback_half_life: f32,
    pub min_interpolation_half_life_multiplier: f32,
    pub reference_view_yaw_speed: f32,
}

impl Default for RotationParams {
    fn default() -> Self {
        Self {
            grounded_moving_velocity_direction: GroundedModeRotation {
                interpolation_half_lives: GaitHalfLives::normal(),
                target_yaw_angle_speed: 800.0,
            },
            grounded_moving_view_direction: GroundedModeRotation {
                interpolation_half_lives: GaitHalfLives::normal(),
                target_yaw_angle_speed: 500.0,
            },
            grounded_moving_aiming: FixedModeRotation {
                interpolation_half_life: 0.1,
                target_yaw_angle_speed: 1000.0,
            },
            grounded_stationary_velocity_direction: FixedModeRotation {
                interpolation_half_life: 0.1,
                target_yaw_angle_speed: 800.0,
            },
            grounded_stationary_view_direction: GroundedModeRotation {
                interpolation_half_lives: GaitHalfLives::normal(),
                target_yaw_angle_speed: 500.0,
            },
            _grounded_stationary_aiming_has_no_damping: (),
            in_air_half_life: 0.2,
            in_air_aiming_half_life: 0.1,
            fallback_half_life: 0.2,
            min_interpolation_half_life_multiplier: 1.0 / 3.0,
            reference_view_yaw_speed: 300.0,
        }
    }
}

impl RotationParams {
    pub fn apply_override(&mut self, ovr: &RotationParamsOverride) {
        if let Some(v) = &ovr.grounded_moving_velocity_direction {
            self.grounded_moving_velocity_direction.apply_override(v);
        }
        if let Some(v) = &ovr.grounded_moving_view_direction {
            self.grounded_moving_view_direction.apply_override(v);
        }
        if let Some(v) = &ovr.grounded_moving_aiming {
            self.grounded_moving_aiming.apply_override(v);
        }
        if let Some(v) = &ovr.grounded_stationary_velocity_direction {
            self.grounded_stationary_velocity_direction
                .apply_override(v);
        }
        if let Some(v) = &ovr.grounded_stationary_view_direction {
            self.grounded_stationary_view_direction.apply_override(v);
        }
        if let Some(v) = ovr.in_air_half_life {
            self.in_air_half_life = v;
        }
        if let Some(v) = ovr.in_air_aiming_half_life {
            self.in_air_aiming_half_life = v;
        }
        if let Some(v) = ovr.fallback_half_life {
            self.fallback_half_life = v;
        }
        if let Some(v) = ovr.min_interpolation_half_life_multiplier {
            self.min_interpolation_half_life_multiplier = v;
        }
        if let Some(v) = ovr.reference_view_yaw_speed {
            self.reference_view_yaw_speed = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroundedModeRotation {
    pub interpolation_half_lives: GaitHalfLives,
    pub target_yaw_angle_speed: f32,
}

impl GroundedModeRotation {
    pub fn apply_override(&mut self, ovr: &GroundedModeRotationOverride) {
        if let Some(v) = &ovr.interpolation_half_lives {
            self.interpolation_half_lives.apply_override(v);
        }
        if let Some(v) = ovr.target_yaw_angle_speed {
            self.target_yaw_angle_speed = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FixedModeRotation {
    pub interpolation_half_life: f32,
    pub target_yaw_angle_speed: f32,
}

impl FixedModeRotation {
    pub fn apply_override(&mut self, ovr: &FixedModeRotationOverride) {
        if let Some(v) = ovr.interpolation_half_life {
            self.interpolation_half_life = v;
        }
        if let Some(v) = ovr.target_yaw_angle_speed {
            self.target_yaw_angle_speed = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GaitHalfLives {
    pub walking: f32,
    pub running: f32,
    pub sprinting: f32,
}

impl GaitHalfLives {
    pub const fn normal() -> Self {
        Self {
            walking: 0.15,
            running: 0.15,
            sprinting: 0.05,
        }
    }

    pub const fn responsive() -> Self {
        Self {
            walking: 0.10,
            running: 0.05,
            sprinting: 0.05,
        }
    }

    pub const fn sluggish() -> Self {
        Self {
            walking: 0.20,
            running: 0.125,
            sprinting: 0.05,
        }
    }

    pub fn half_life(&self, gait: crate::state::Gait) -> f32 {
        match gait {
            crate::state::Gait::Walking => self.walking,
            crate::state::Gait::Running => self.running,
            crate::state::Gait::Sprinting => self.sprinting,
        }
    }

    pub fn apply_override(&mut self, ovr: &GaitHalfLivesOverride) {
        if let Some(v) = ovr.walking {
            self.walking = v;
        }
        if let Some(v) = ovr.running {
            self.running = v;
        }
        if let Some(v) = ovr.sprinting {
            self.sprinting = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CrouchParams {
    pub standing_capsule_half_height: f32,
    pub standing_capsule_radius: f32,
    pub crouched_capsule_half_height: f32,
    pub animated_crouch_speed: f32,
    pub max_walk_speed_crouched: f32,
}

impl Default for CrouchParams {
    fn default() -> Self {
        Self {
            standing_capsule_half_height: 90.0 * UE_CM_TO_M,
            standing_capsule_radius: 30.0 * UE_CM_TO_M,
            crouched_capsule_half_height: 56.0 * UE_CM_TO_M,
            animated_crouch_speed: 150.0 * UE_CM_TO_M,
            max_walk_speed_crouched: 150.0 * UE_CM_TO_M,
        }
    }
}

impl CrouchParams {
    pub fn apply_override(&mut self, ovr: &CrouchParamsOverride) {
        if let Some(v) = ovr.standing_capsule_half_height {
            self.standing_capsule_half_height = v;
        }
        if let Some(v) = ovr.standing_capsule_radius {
            self.standing_capsule_radius = v;
        }
        if let Some(v) = ovr.crouched_capsule_half_height {
            self.crouched_capsule_half_height = v;
        }
        if let Some(v) = ovr.animated_crouch_speed {
            self.animated_crouch_speed = v;
        }
        if let Some(v) = ovr.max_walk_speed_crouched {
            self.max_walk_speed_crouched = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct JumpParams {
    pub jump_z_velocity: f32,
    pub gravity_scale: f32,
    pub coyote_time: f32,
    pub jump_buffer_time: f32,
}

impl Default for JumpParams {
    fn default() -> Self {
        Self {
            jump_z_velocity: 420.0 * UE_CM_TO_M,
            gravity_scale: 1.0,
            coyote_time: 0.10,
            jump_buffer_time: 0.15,
        }
    }
}

impl JumpParams {
    pub fn apply_override(&mut self, ovr: &JumpParamsOverride) {
        if let Some(v) = ovr.jump_z_velocity {
            self.jump_z_velocity = v;
        }
        if let Some(v) = ovr.gravity_scale {
            self.gravity_scale = v;
        }
        if let Some(v) = ovr.coyote_time {
            self.coyote_time = v;
        }
        if let Some(v) = ovr.jump_buffer_time {
            self.jump_buffer_time = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct AlsMovementParamsOverride {
    pub speeds: Option<GaitSpeedsOverride>,
    pub acceleration: Option<AccelerationParamsOverride>,
    pub rotation: Option<RotationParamsOverride>,
    pub crouch: Option<CrouchParamsOverride>,
    pub jump: Option<JumpParamsOverride>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct GaitSpeedsOverride {
    pub standing: Option<StanceGaitSpeedsOverride>,
    pub crouching: Option<StanceGaitSpeedsOverride>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct StanceGaitSpeedsOverride {
    pub max_walk_speed: Option<f32>,
    pub max_run_speed: Option<f32>,
    pub max_sprint_speed: Option<f32>,
    pub animated_walk_speed: Option<f32>,
    pub animated_run_speed: Option<f32>,
    pub animated_sprint_speed: Option<f32>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct AccelerationParamsOverride {
    pub max_acceleration: Option<f32>,
    pub braking_deceleration: Option<f32>,
    pub ground_friction: Option<f32>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct RotationParamsOverride {
    pub grounded_moving_velocity_direction: Option<GroundedModeRotationOverride>,
    pub grounded_moving_view_direction: Option<GroundedModeRotationOverride>,
    pub grounded_moving_aiming: Option<FixedModeRotationOverride>,
    pub grounded_stationary_velocity_direction: Option<FixedModeRotationOverride>,
    pub grounded_stationary_view_direction: Option<GroundedModeRotationOverride>,
    pub in_air_half_life: Option<f32>,
    pub in_air_aiming_half_life: Option<f32>,
    pub fallback_half_life: Option<f32>,
    pub min_interpolation_half_life_multiplier: Option<f32>,
    pub reference_view_yaw_speed: Option<f32>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct GroundedModeRotationOverride {
    pub interpolation_half_lives: Option<GaitHalfLivesOverride>,
    pub target_yaw_angle_speed: Option<f32>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct FixedModeRotationOverride {
    pub interpolation_half_life: Option<f32>,
    pub target_yaw_angle_speed: Option<f32>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct GaitHalfLivesOverride {
    pub walking: Option<f32>,
    pub running: Option<f32>,
    pub sprinting: Option<f32>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct CrouchParamsOverride {
    pub standing_capsule_half_height: Option<f32>,
    pub standing_capsule_radius: Option<f32>,
    pub crouched_capsule_half_height: Option<f32>,
    pub animated_crouch_speed: Option<f32>,
    pub max_walk_speed_crouched: Option<f32>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct JumpParamsOverride {
    pub jump_z_velocity: Option<f32>,
    pub gravity_scale: Option<f32>,
    pub coyote_time: Option<f32>,
    pub jump_buffer_time: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Gait, Stance};

    #[test]
    fn defaults_match_als_source_values() {
        let p = AlsMovementParams::default();

        assert!((p.speeds.standing.max_walk_speed - 1.75).abs() < 1e-6);
        assert!((p.speeds.standing.max_run_speed - 3.75).abs() < 1e-6);
        assert!((p.speeds.standing.max_sprint_speed - 6.50).abs() < 1e-6);
        assert_eq!(p.speeds.standing, p.speeds.crouching);

        assert!((p.acceleration.max_acceleration - 20.0).abs() < 1e-6);
        assert!((p.acceleration.braking_deceleration - 15.0).abs() < 1e-6);
        assert!((p.acceleration.ground_friction - 4.0).abs() < 1e-6);

        assert_eq!(
            p.rotation
                .grounded_moving_velocity_direction
                .interpolation_half_lives,
            GaitHalfLives::normal()
        );
        assert!(
            (p.rotation
                .grounded_moving_velocity_direction
                .target_yaw_angle_speed
                - 800.0)
                .abs()
                < 1e-6
        );
        assert!(
            (p.rotation
                .grounded_moving_view_direction
                .target_yaw_angle_speed
                - 500.0)
                .abs()
                < 1e-6
        );
        assert!((p.rotation.grounded_moving_aiming.interpolation_half_life - 0.1).abs() < 1e-6);
        assert!((p.rotation.in_air_half_life - 0.2).abs() < 1e-6);
        assert!((p.rotation.min_interpolation_half_life_multiplier - 1.0 / 3.0).abs() < 1e-6);

        assert!((p.crouch.standing_capsule_half_height - 0.90).abs() < 1e-6);
        assert!((p.crouch.standing_capsule_radius - 0.30).abs() < 1e-6);
        assert!((p.crouch.crouched_capsule_half_height - 0.56).abs() < 1e-6);

        assert!((p.jump.jump_z_velocity - 4.20).abs() < 1e-6);
        assert!((p.jump.gravity_scale - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ron_roundtrip_preserves_params() {
        let p = AlsMovementParams::default();
        let text = ron::to_string(&p).unwrap();
        let de: AlsMovementParams = ron::from_str(&text).unwrap();
        assert_eq!(de, p);
    }

    #[test]
    fn ron_partial_override_deserializes_with_none_for_missing_fields() {
        let ovr: AlsMovementParamsOverride =
            ron::from_str("(speeds: Some((standing: Some((max_sprint_speed: Some(7.0))))))")
                .unwrap();
        assert!(ovr.jump.is_none());
        assert!(ovr.rotation.is_none());
        let speeds = ovr.speeds.as_ref().unwrap();
        assert!(speeds.crouching.is_none());
        let standing = speeds.standing.as_ref().unwrap();
        assert_eq!(standing.max_sprint_speed, Some(7.0));
        assert_eq!(standing.max_walk_speed, None);
        assert_eq!(standing.max_run_speed, None);
    }

    #[test]
    fn override_deep_merges_without_touching_other_fields() {
        let mut p = AlsMovementParams::default();
        let before = p.clone();

        let ovr: AlsMovementParamsOverride = ron::from_str(
            "(\
                speeds: Some((crouching: Some((max_walk_speed: Some(1.2))))), \
                rotation: Some((in_air_aiming_half_life: Some(0.08))), \
                jump: Some((jump_z_velocity: Some(4.5))), \
            )",
        )
        .unwrap();
        p.apply_override(&ovr);

        assert!((p.speeds.crouching.max_walk_speed - 1.2).abs() < 1e-6);
        assert!((p.rotation.in_air_aiming_half_life - 0.08).abs() < 1e-6);
        assert!((p.jump.jump_z_velocity - 4.5).abs() < 1e-6);

        assert_eq!(p.speeds.standing, before.speeds.standing);
        assert_eq!(
            p.speeds.crouching.max_run_speed,
            before.speeds.crouching.max_run_speed
        );
        assert_eq!(
            p.speeds.crouching.max_sprint_speed,
            before.speeds.crouching.max_sprint_speed
        );
        assert_eq!(p.acceleration, before.acceleration);
        assert_eq!(p.crouch, before.crouch);
        assert_eq!(p.jump.gravity_scale, before.jump.gravity_scale);
        assert_eq!(p.jump.coyote_time, before.jump.coyote_time);
        assert_eq!(
            p.rotation.grounded_moving_velocity_direction,
            before.rotation.grounded_moving_velocity_direction
        );
        assert_eq!(
            p.rotation.in_air_half_life,
            before.rotation.in_air_half_life
        );
    }

    #[test]
    fn empty_override_is_noop() {
        let mut p = AlsMovementParams::default();
        let before = p.clone();
        let ovr: AlsMovementParamsOverride = ron::from_str("()").unwrap();
        p.apply_override(&ovr);
        assert_eq!(p, before);
    }

    #[test]
    fn boundary_gaits_ordered_and_crouch_below_standing() {
        let p = AlsMovementParams::default();
        for s in [p.speeds.standing, p.speeds.crouching] {
            assert!(s.max_sprint_speed > s.max_run_speed);
            assert!(s.max_run_speed > s.max_walk_speed);
            assert!(s.max_walk_speed > 0.0);
        }
        assert!(p.crouch.crouched_capsule_half_height < p.crouch.standing_capsule_half_height);
        assert!(p.jump.jump_z_velocity > 0.0);
        assert!(p.jump.coyote_time >= 0.0 && p.jump.jump_buffer_time >= 0.0);
        assert!(p.acceleration.ground_friction > 0.0);
        assert!(p.acceleration.max_acceleration > 0.0);
        assert!(p.acceleration.braking_deceleration > 0.0);
    }

    #[test]
    fn rotation_preset_samples_ordered_and_positive() {
        let n = GaitHalfLives::normal();
        let r = GaitHalfLives::responsive();
        let s = GaitHalfLives::sluggish();
        for p in [&n, &r, &s] {
            assert!(p.walking > 0.0 && p.running > 0.0 && p.sprinting > 0.0);
            assert!(p.walking >= p.running || p.running <= p.walking);
            assert!(p.sprinting <= p.running.min(p.walking));
        }
        assert!(r.walking < n.walking && n.walking < s.walking);
        for p in [&n, &r, &s] {
            assert!((p.sprinting - 0.05).abs() < 1e-6);
        }
        assert_eq!(n.half_life(Gait::Sprinting), n.sprinting);
        assert_eq!(n.half_life(Gait::Walking), n.walking);
    }

    #[test]
    fn stance_speed_lookup_matches_state_enum() {
        let p = AlsMovementParams::default();
        assert!(std::ptr::eq(
            p.stance_speeds(Stance::Standing),
            &p.speeds.standing
        ));
        assert!(std::ptr::eq(
            p.stance_speeds(Stance::Crouching),
            &p.speeds.crouching
        ));
    }
}
