use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::orientation::world_forward_to_yaw;
use crate::state::{Gait, RotationMode, Stance};

pub const UE_CM_TO_M: f32 = 0.01;
static SOURCE_UNCONFIGURED_GAIT_SPEEDS: std::sync::LazyLock<StanceGaitSpeeds> = std::sync::LazyLock::new(StanceGaitSpeeds::default);

#[derive(Asset, Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[reflect(Default)]
pub struct AlsMovementParams {
    pub speeds: GaitSpeeds,
    pub acceleration: AccelerationParams,
    pub cmc: CmcAdapterParams,
    pub rotation: RotationParams,
    pub crouch: CrouchParams,
    pub jump: JumpParams,
}

impl AlsMovementParams {
    pub fn stance_speeds(&self, stance: crate::state::Stance) -> &StanceGaitSpeeds {
        match stance {
            crate::state::Stance::Standing => &self.speeds.standing,
            crate::state::Stance::Crouching => &self.speeds.crouching,
            _ => &SOURCE_UNCONFIGURED_GAIT_SPEEDS,
        }
    }

    pub fn gait_speeds(&self, stance: Stance, rotation_mode: RotationMode) -> &StanceGaitSpeeds {
        if !Self::has_gait_settings(stance, rotation_mode) { return &SOURCE_UNCONFIGURED_GAIT_SPEEDS; }
        match stance {
            Stance::Crouching => &self.speeds.crouching,
            Stance::Standing if rotation_mode == RotationMode::Aiming => {
                &self.speeds.aiming_standing
            }
            Stance::Standing => &self.speeds.standing,
            _ => &SOURCE_UNCONFIGURED_GAIT_SPEEDS,
        }
    }

    pub fn has_gait_settings(stance: Stance, rotation_mode: RotationMode) -> bool {
        matches!(stance, Stance::Standing | Stance::Crouching)
            && matches!(rotation_mode, RotationMode::VelocityDirection | RotationMode::ViewDirection | RotationMode::Aiming)
    }

    pub fn gait_speed_thresholds(
        &self,
        stance: Stance,
        rotation_mode: RotationMode,
        velocity: Vec3,
        view_yaw: f32,
    ) -> GaitSpeedThresholds {
        let speeds = self.gait_speeds(stance, rotation_mode);
        let forward_amount = if speeds.allow_direction_dependent_movement_speed {
            self.speeds
                .direction_speed_forward_amount(velocity, view_yaw)
        } else {
            1.0
        };
        speeds.gait_speed_thresholds(forward_amount)
    }

    pub fn grounded_dynamics_profile(
        &self,
        stance: Stance,
        rotation_mode: RotationMode,
    ) -> GroundedDynamicsProfile {
        if stance == Stance::Crouching || rotation_mode == RotationMode::VelocityDirection {
            GroundedDynamicsProfile::Responsive
        } else {
            GroundedDynamicsProfile::Normal
        }
    }

    pub fn grounded_dynamics(
        &self,
        stance: Stance,
        rotation_mode: RotationMode,
        gait_amount: f32,
    ) -> GroundedDynamics {
        self.acceleration.grounded.sample(
            self.grounded_dynamics_profile(stance, rotation_mode),
            gait_amount,
        )
    }

    pub fn apply_override(&mut self, ovr: &AlsMovementParamsOverride) {
        if let Some(v) = &ovr.speeds {
            self.speeds.apply_override(v);
        }
        if let Some(v) = &ovr.acceleration {
            self.acceleration.apply_override(v);
        }
        if let Some(v) = &ovr.cmc {
            self.cmc.apply_override(v);
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

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct GaitSpeeds {
    pub velocity_angle_to_speed_interpolation_start_degrees: f32,
    pub velocity_angle_to_speed_interpolation_end_degrees: f32,
    pub standing: StanceGaitSpeeds,
    pub aiming_standing: StanceGaitSpeeds,
    pub crouching: StanceGaitSpeeds,
}

impl Default for GaitSpeeds {
    fn default() -> Self {
        Self {
            velocity_angle_to_speed_interpolation_start_degrees: 100.0,
            velocity_angle_to_speed_interpolation_end_degrees: 125.0,
            standing: StanceGaitSpeeds::default(),
            aiming_standing: StanceGaitSpeeds {
                allow_direction_dependent_movement_speed: false,
                max_walk_speed: 165.0 * UE_CM_TO_M,
                max_walk_backward_speed: 165.0 * UE_CM_TO_M,
                max_run_speed: 375.0 * UE_CM_TO_M,
                max_run_backward_speed: 375.0 * UE_CM_TO_M,
                max_sprint_speed: 650.0 * UE_CM_TO_M,
                animated_walk_speed: 150.0 * UE_CM_TO_M,
                animated_run_speed: 350.0 * UE_CM_TO_M,
                animated_sprint_speed: 600.0 * UE_CM_TO_M,
            },
            crouching: StanceGaitSpeeds {
                allow_direction_dependent_movement_speed: false,
                max_walk_speed: 150.0 * UE_CM_TO_M,
                max_walk_backward_speed: 150.0 * UE_CM_TO_M,
                max_run_speed: 200.0 * UE_CM_TO_M,
                max_run_backward_speed: 200.0 * UE_CM_TO_M,
                max_sprint_speed: 300.0 * UE_CM_TO_M,
                animated_walk_speed: 150.0 * UE_CM_TO_M,
                animated_run_speed: 350.0 * UE_CM_TO_M,
                animated_sprint_speed: 600.0 * UE_CM_TO_M,
            },
        }
    }
}

impl GaitSpeeds {
    pub fn direction_speed_forward_amount(&self, velocity: Vec3, view_yaw: f32) -> f32 {
        let horizontal_velocity = Vec2::new(velocity.x, velocity.z);
        if horizontal_velocity.length_squared() <= 1e-6 {
            return 1.0;
        }
        let velocity_yaw = world_forward_to_yaw(velocity);
        let relative_yaw = (velocity_yaw - view_yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let angle = relative_yaw.abs().to_degrees();
        let start = self
            .velocity_angle_to_speed_interpolation_start_degrees
            .clamp(0.0, 180.0);
        let end = self
            .velocity_angle_to_speed_interpolation_end_degrees
            .clamp(start, 180.0);
        let backward_amount = if end <= start + f32::EPSILON {
            if angle >= end { 1.0 } else { 0.0 }
        } else {
            ((angle - start) / (end - start)).clamp(0.0, 1.0)
        };
        1.0 - backward_amount
    }

    pub fn apply_override(&mut self, ovr: &GaitSpeedsOverride) {
        if let Some(v) = ovr.velocity_angle_to_speed_interpolation_start_degrees {
            self.velocity_angle_to_speed_interpolation_start_degrees = v;
        }
        if let Some(v) = ovr.velocity_angle_to_speed_interpolation_end_degrees {
            self.velocity_angle_to_speed_interpolation_end_degrees = v;
        }
        if let Some(v) = &ovr.standing {
            self.standing.apply_override(v);
        }
        if let Some(v) = &ovr.aiming_standing {
            self.aiming_standing.apply_override(v);
        }
        if let Some(v) = &ovr.crouching {
            self.crouching.apply_override(v);
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct StanceGaitSpeeds {
    pub allow_direction_dependent_movement_speed: bool,
    pub max_walk_speed: f32,
    pub max_walk_backward_speed: f32,
    pub max_run_speed: f32,
    pub max_run_backward_speed: f32,
    pub max_sprint_speed: f32,
    pub animated_walk_speed: f32,
    pub animated_run_speed: f32,
    pub animated_sprint_speed: f32,
}

impl Default for StanceGaitSpeeds {
    fn default() -> Self {
        Self {
            allow_direction_dependent_movement_speed: false,
            max_walk_speed: 175.0 * UE_CM_TO_M,
            max_walk_backward_speed: 175.0 * UE_CM_TO_M,
            max_run_speed: 375.0 * UE_CM_TO_M,
            max_run_backward_speed: 375.0 * UE_CM_TO_M,
            max_sprint_speed: 650.0 * UE_CM_TO_M,
            animated_walk_speed: 150.0 * UE_CM_TO_M,
            animated_run_speed: 350.0 * UE_CM_TO_M,
            animated_sprint_speed: 600.0 * UE_CM_TO_M,
        }
    }
}

impl StanceGaitSpeeds {
    pub fn gait_speed_thresholds(&self, forward_amount: f32) -> GaitSpeedThresholds {
        let forward_amount = if self.allow_direction_dependent_movement_speed {
            forward_amount.clamp(0.0, 1.0)
        } else {
            1.0
        };
        GaitSpeedThresholds {
            walk: self
                .max_walk_backward_speed
                .lerp(self.max_walk_speed, forward_amount),
            run: self
                .max_run_backward_speed
                .lerp(self.max_run_speed, forward_amount),
            sprint: self.max_sprint_speed,
        }
    }

    pub fn apply_override(&mut self, ovr: &StanceGaitSpeedsOverride) {
        if let Some(v) = ovr.allow_direction_dependent_movement_speed {
            self.allow_direction_dependent_movement_speed = v;
        }
        if let Some(v) = ovr.max_walk_speed {
            self.max_walk_speed = v;
        }
        if let Some(v) = ovr.max_walk_backward_speed {
            self.max_walk_backward_speed = v;
        }
        if let Some(v) = ovr.max_run_speed {
            self.max_run_speed = v;
        }
        if let Some(v) = ovr.max_run_backward_speed {
            self.max_run_backward_speed = v;
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
    pub grounded: GroundedDynamicsCurves,
}

impl Default for AccelerationParams {
    fn default() -> Self {
        Self {
            max_acceleration: 2000.0 * UE_CM_TO_M,
            braking_deceleration: 1500.0 * UE_CM_TO_M,
            ground_friction: 4.0,
            grounded: GroundedDynamicsCurves::default(),
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
        if let Some(v) = &ovr.grounded {
            self.grounded = v.clone();
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CmcAdapterParams {
    pub aiming_yaw_angle_limit_degrees: f32,
    pub min_analog_walk_speed: f32,
    pub perch_radius_threshold: f32,
    pub perch_additional_height: f32,
    pub jump_off_jump_z_factor: f32,
    pub max_simulation_time_step: f32,
    pub max_simulation_iterations: u32,
    pub braking_substep_time: f32,
}

impl Default for CmcAdapterParams {
    fn default() -> Self {
        Self {
            aiming_yaw_angle_limit_degrees: 70.0,
            min_analog_walk_speed: 25.0 * UE_CM_TO_M,
            perch_radius_threshold: 15.0 * UE_CM_TO_M,
            perch_additional_height: 0.0,
            jump_off_jump_z_factor: 0.0,
            max_simulation_time_step: 0.05,
            max_simulation_iterations: 8,
            braking_substep_time: 1.0 / 33.0,
        }
    }
}

impl CmcAdapterParams {
    pub fn apply_override(&mut self, ovr: &CmcAdapterParamsOverride) {
        if let Some(v) = ovr.aiming_yaw_angle_limit_degrees {
            self.aiming_yaw_angle_limit_degrees = v;
        }
        if let Some(v) = ovr.min_analog_walk_speed {
            self.min_analog_walk_speed = v;
        }
        if let Some(v) = ovr.perch_radius_threshold {
            self.perch_radius_threshold = v;
        }
        if let Some(v) = ovr.perch_additional_height {
            self.perch_additional_height = v;
        }
        if let Some(v) = ovr.jump_off_jump_z_factor {
            self.jump_off_jump_z_factor = v;
        }
        if let Some(v) = ovr.max_simulation_time_step {
            self.max_simulation_time_step = v;
        }
        if let Some(v) = ovr.max_simulation_iterations {
            self.max_simulation_iterations = v;
        }
        if let Some(v) = ovr.braking_substep_time {
            self.braking_substep_time = v;
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroundedDynamicsProfile {
    #[default]
    Normal,
    Responsive,
    Sluggish,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct GroundedDynamics {
    pub acceleration: f32,
    pub braking_deceleration: f32,
    pub ground_friction: f32,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct HermiteKey {
    pub time: f32,
    pub value: f32,
    pub arrive_tangent: f32,
    pub leave_tangent: f32,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct HermiteCurve {
    pub keys: Vec<HermiteKey>,
}

impl HermiteCurve {
    pub fn sample(&self, time: f32) -> f32 {
        let Some(first) = self.keys.first() else {
            return 0.0;
        };
        let Some(last) = self.keys.last() else {
            return 0.0;
        };
        if time <= first.time {
            return first.value;
        }
        if time >= last.time {
            return last.value;
        }
        let upper = self.keys.partition_point(|key| key.time <= time);
        let start = self.keys[upper - 1];
        let end = self.keys[upper];
        let duration = (end.time - start.time).max(f32::EPSILON);
        let normalized = ((time - start.time) / duration).clamp(0.0, 1.0);
        let normalized_squared = normalized * normalized;
        let normalized_cubed = normalized_squared * normalized;
        let h00 = 2.0 * normalized_cubed - 3.0 * normalized_squared + 1.0;
        let h10 = normalized_cubed - 2.0 * normalized_squared + normalized;
        let h01 = -2.0 * normalized_cubed + 3.0 * normalized_squared;
        let h11 = normalized_cubed - normalized_squared;
        h00 * start.value
            + h10 * duration * start.leave_tangent
            + h01 * end.value
            + h11 * duration * end.arrive_tangent
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct GroundedDynamicsCurve {
    pub acceleration: HermiteCurve,
    pub braking_deceleration: HermiteCurve,
    pub ground_friction: HermiteCurve,
}

impl GroundedDynamicsCurve {
    pub fn sample(&self, gait_amount: f32) -> GroundedDynamics {
        GroundedDynamics {
            acceleration: self.acceleration.sample(gait_amount),
            braking_deceleration: self.braking_deceleration.sample(gait_amount),
            ground_friction: self.ground_friction.sample(gait_amount),
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroundedDynamicsCurves {
    pub normal: GroundedDynamicsCurve,
    pub responsive: GroundedDynamicsCurve,
    pub sluggish: GroundedDynamicsCurve,
}

impl GroundedDynamicsCurves {
    pub fn curve(&self, profile: GroundedDynamicsProfile) -> &GroundedDynamicsCurve {
        match profile {
            GroundedDynamicsProfile::Normal => &self.normal,
            GroundedDynamicsProfile::Responsive => &self.responsive,
            GroundedDynamicsProfile::Sluggish => &self.sluggish,
        }
    }

    pub fn sample(&self, profile: GroundedDynamicsProfile, gait_amount: f32) -> GroundedDynamics {
        self.curve(profile).sample(gait_amount)
    }
}

fn source_curve(keys: &[(f32, f32, f32, f32)]) -> HermiteCurve {
    HermiteCurve {
        keys: keys
            .iter()
            .map(|&(time, value, arrive_tangent, leave_tangent)| HermiteKey {
                time,
                value,
                arrive_tangent,
                leave_tangent,
            })
            .collect(),
    }
}

fn source_dynamics_curve(
    acceleration: &[(f32, f32, f32, f32)],
    braking_deceleration: &[(f32, f32, f32, f32)],
    ground_friction: &[(f32, f32, f32, f32)],
) -> GroundedDynamicsCurve {
    GroundedDynamicsCurve {
        acceleration: source_curve(acceleration),
        braking_deceleration: source_curve(braking_deceleration),
        ground_friction: source_curve(ground_friction),
    }
}

impl Default for GroundedDynamicsCurves {
    fn default() -> Self {
        Self {
            normal: source_dynamics_curve(
                &[
                    (0.0, 20.0, 0.0, 0.0),
                    (1.0, 20.0, 0.0, 0.0),
                    (2.0, 20.0, 0.0, -36.423_724),
                    (3.0, 7.5, 0.0, 0.0),
                ],
                &[
                    (0.0, 15.0, 0.0, 0.0),
                    (1.0, 15.0, 0.0, -2.5),
                    (2.0, 12.5, -2.5, -7.5),
                    (3.0, 5.0, -7.5, 0.0),
                ],
                &[
                    (0.0, 5.0, 0.0, 0.0),
                    (1.0, 5.0, 0.0, -1.0),
                    (2.0, 4.0, -1.0, -3.5),
                    (3.0, 0.5, -3.5, 0.0),
                ],
            ),
            responsive: source_dynamics_curve(
                &[
                    (0.0, 25.0, 0.0, 0.0),
                    (1.0, 25.0, 0.0, 0.0),
                    (2.0, 25.0, 0.0, -58.633_94),
                    (3.0, 5.0, 0.0, 0.0),
                ],
                &[
                    (0.0, 20.0, 0.0, 0.0),
                    (1.0, 20.0, 0.0, 0.0),
                    (2.0, 20.0, 0.0, -27.156_406),
                    (3.0, 8.0, 0.0, 0.0),
                ],
                &[
                    (0.0, 10.0, 0.0, 0.0),
                    (1.0, 10.0, 0.0, 0.0),
                    (2.0, 10.0, 0.0, 0.0),
                    (3.0, 5.0, 0.0, 0.0),
                ],
            ),
            sluggish: source_dynamics_curve(
                &[
                    (0.0, 5.0, 0.0, 0.0),
                    (1.0, 5.0, 0.0, 0.0),
                    (2.0, 5.0, 0.0, -8.434_028),
                    (3.0, 2.0, 0.0, 0.0),
                ],
                &[
                    (0.0, 10.0, 0.0, 0.0),
                    (1.0, 10.0, 0.0, 0.0),
                    (2.0, 7.5, -5.853_719, -5.853_719),
                    (3.0, 3.0, 0.0, 0.0),
                ],
                &[
                    (0.0, 4.0, 0.0, 0.0),
                    (1.0, 4.0, 0.0, 0.0),
                    (2.0, 3.0, -2.125_014, -2.125_014),
                    (3.0, 1.0, 0.0, 0.0),
                ],
            ),
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
                interpolation_half_lives: GaitHalfLives::responsive(),
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

const SOURCE_ROTATION_HALF_LIVES: ([f32; 3], [f32; 3]) = include!("../data/rotation_half_lives.rs");

impl GaitHalfLives {
    pub const fn normal() -> Self {
        Self {
            walking: SOURCE_ROTATION_HALF_LIVES.0[0],
            running: SOURCE_ROTATION_HALF_LIVES.0[1],
            sprinting: SOURCE_ROTATION_HALF_LIVES.0[2],
        }
    }

    pub const fn responsive() -> Self {
        Self {
            walking: SOURCE_ROTATION_HALF_LIVES.1[0],
            running: SOURCE_ROTATION_HALF_LIVES.1[1],
            sprinting: SOURCE_ROTATION_HALF_LIVES.1[2],
        }
    }

    pub const fn sluggish() -> Self {
        Self {
            walking: 0.20,
            running: 0.125,
            sprinting: 0.05,
        }
    }

    pub fn sample(&self, gait_amount: f32) -> f32 {
        let gait_amount = gait_amount.clamp(1.0, 3.0);
        if gait_amount <= 1.0 {
            return self.walking;
        }
        if gait_amount >= 3.0 {
            return self.sprinting;
        }
        if gait_amount < 2.0 {
            self.walking.lerp(self.running, gait_amount - 1.0)
        } else {
            self.running.lerp(self.sprinting, gait_amount - 2.0)
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
            coyote_time: 0.0,
            jump_buffer_time: 0.0,
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
    pub cmc: Option<CmcAdapterParamsOverride>,
    pub rotation: Option<RotationParamsOverride>,
    pub crouch: Option<CrouchParamsOverride>,
    pub jump: Option<JumpParamsOverride>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct GaitSpeedsOverride {
    pub velocity_angle_to_speed_interpolation_start_degrees: Option<f32>,
    pub velocity_angle_to_speed_interpolation_end_degrees: Option<f32>,
    pub standing: Option<StanceGaitSpeedsOverride>,
    pub aiming_standing: Option<StanceGaitSpeedsOverride>,
    pub crouching: Option<StanceGaitSpeedsOverride>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct StanceGaitSpeedsOverride {
    pub allow_direction_dependent_movement_speed: Option<bool>,
    pub max_walk_speed: Option<f32>,
    pub max_walk_backward_speed: Option<f32>,
    pub max_run_speed: Option<f32>,
    pub max_run_backward_speed: Option<f32>,
    pub max_sprint_speed: Option<f32>,
    pub animated_walk_speed: Option<f32>,
    pub animated_run_speed: Option<f32>,
    pub animated_sprint_speed: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaitSpeedThresholds {
    pub walk: f32,
    pub run: f32,
    pub sprint: f32,
}

impl GaitSpeedThresholds {
    pub fn for_gait(self, gait: Gait) -> f32 {
        match gait {
            Gait::Walking => self.walk,
            Gait::Running => self.run,
            Gait::Sprinting => self.sprint,
            _ => self.run,
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct AccelerationParamsOverride {
    pub max_acceleration: Option<f32>,
    pub braking_deceleration: Option<f32>,
    pub ground_friction: Option<f32>,
    pub grounded: Option<GroundedDynamicsCurves>,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct CmcAdapterParamsOverride {
    pub aiming_yaw_angle_limit_degrees: Option<f32>,
    pub min_analog_walk_speed: Option<f32>,
    pub perch_radius_threshold: Option<f32>,
    pub perch_additional_height: Option<f32>,
    pub jump_off_jump_z_factor: Option<f32>,
    pub max_simulation_time_step: Option<f32>,
    pub max_simulation_iterations: Option<u32>,
    pub braking_substep_time: Option<f32>,
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
    use crate::state::{RotationMode, Stance};

    #[test]
    fn defaults_match_als_source_values() {
        let p = AlsMovementParams::default();

        assert!((p.speeds.standing.max_walk_speed - 1.75).abs() < 1e-6);
        assert!((p.speeds.standing.max_run_speed - 3.75).abs() < 1e-6);
        assert!((p.speeds.standing.max_sprint_speed - 6.50).abs() < 1e-6);
        assert!((p.speeds.aiming_standing.max_walk_speed - 1.65).abs() < 1e-6);
        assert!((p.speeds.aiming_standing.max_run_speed - 3.75).abs() < 1e-6);
        assert!((p.speeds.aiming_standing.max_sprint_speed - 6.50).abs() < 1e-6);
        assert!((p.speeds.crouching.max_walk_speed - 1.50).abs() < 1e-6);
        assert!((p.speeds.crouching.max_run_speed - 2.00).abs() < 1e-6);
        assert!((p.speeds.crouching.max_sprint_speed - 3.00).abs() < 1e-6);
        assert!(!p.speeds.standing.allow_direction_dependent_movement_speed);
        assert!(
            (p.speeds.velocity_angle_to_speed_interpolation_start_degrees - 100.0).abs() < 1e-6
        );
        assert!((p.speeds.velocity_angle_to_speed_interpolation_end_degrees - 125.0).abs() < 1e-6);

        assert!((p.acceleration.max_acceleration - 20.0).abs() < 1e-6);
        assert!((p.acceleration.braking_deceleration - 15.0).abs() < 1e-6);
        assert!((p.acceleration.ground_friction - 4.0).abs() < 1e-6);
        assert!((p.cmc.aiming_yaw_angle_limit_degrees - 70.0).abs() < 1e-6);
        assert!((p.cmc.min_analog_walk_speed - 0.25).abs() < 1e-6);
        assert!((p.cmc.perch_radius_threshold - 0.15).abs() < 1e-6);
        assert_eq!(p.cmc.perch_additional_height, 0.0);
        assert_eq!(p.cmc.jump_off_jump_z_factor, 0.0);
        assert!((p.cmc.max_simulation_time_step - 0.05).abs() < 1e-6);
        assert_eq!(p.cmc.max_simulation_iterations, 8);
        assert!((p.cmc.braking_substep_time - 1.0 / 33.0).abs() < 1e-6);
        assert_eq!(
            p.grounded_dynamics_profile(Stance::Standing, RotationMode::VelocityDirection),
            GroundedDynamicsProfile::Responsive
        );
        assert_eq!(
            p.grounded_dynamics_profile(Stance::Standing, RotationMode::ViewDirection),
            GroundedDynamicsProfile::Normal
        );
        assert_eq!(
            p.grounded_dynamics_profile(Stance::Crouching, RotationMode::Aiming),
            GroundedDynamicsProfile::Responsive
        );

        assert_eq!(
            p.rotation
                .grounded_moving_velocity_direction
                .interpolation_half_lives,
            GaitHalfLives::responsive()
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
        assert_eq!(p.jump.coyote_time, 0.0);
        assert_eq!(p.jump.jump_buffer_time, 0.0);
    }

    #[test]
    fn direction_dependent_speed_defaults_preserve_forward_thresholds() {
        let p = AlsMovementParams::default();
        let forward = p.gait_speed_thresholds(
            Stance::Standing,
            RotationMode::ViewDirection,
            Vec3::NEG_Z,
            0.0,
        );
        let backward =
            p.gait_speed_thresholds(Stance::Standing, RotationMode::ViewDirection, Vec3::Z, 0.0);
        assert_eq!(forward, backward);
        assert!((forward.walk - 1.75).abs() < 1e-6);
        assert!((forward.run - 3.75).abs() < 1e-6);
        assert!((forward.sprint - 6.5).abs() < 1e-6);
    }

    #[test]
    fn direction_dependent_speed_matches_the_source_interpolation_window() {
        let mut p = AlsMovementParams::default();
        let standing = &mut p.speeds.standing;
        standing.allow_direction_dependent_movement_speed = true;
        standing.max_walk_backward_speed = 1.0;
        standing.max_run_backward_speed = 2.0;
        let forward = p.gait_speed_thresholds(
            Stance::Standing,
            RotationMode::ViewDirection,
            Vec3::NEG_Z,
            0.0,
        );
        let middle_yaw = 112.5_f32.to_radians();
        let middle_velocity = crate::orientation::yaw_to_world_forward(middle_yaw);
        let middle = p.gait_speed_thresholds(
            Stance::Standing,
            RotationMode::ViewDirection,
            middle_velocity,
            0.0,
        );
        let backward =
            p.gait_speed_thresholds(Stance::Standing, RotationMode::ViewDirection, Vec3::Z, 0.0);
        println!(
            "[BF101.5] forward=({:.3},{:.3}) middle=({:.3},{:.3}) backward=({:.3},{:.3})",
            forward.walk, forward.run, middle.walk, middle.run, backward.walk, backward.run
        );
        assert!((forward.walk - 1.75).abs() < 1e-6);
        assert!((forward.run - 3.75).abs() < 1e-6);
        assert!((middle.walk - 1.375).abs() < 1e-6);
        assert!((middle.run - 2.875).abs() < 1e-6);
        assert!((backward.walk - 1.0).abs() < 1e-6);
        assert!((backward.run - 2.0).abs() < 1e-6);
        assert!((backward.sprint - 6.5).abs() < 1e-6);
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
                cmc: Some((min_analog_walk_speed: Some(0.3))), \
                rotation: Some((in_air_aiming_half_life: Some(0.08))), \
                jump: Some((jump_z_velocity: Some(4.5))), \
            )",
        )
        .unwrap();
        p.apply_override(&ovr);

        assert!((p.speeds.crouching.max_walk_speed - 1.2).abs() < 1e-6);
        assert!((p.cmc.min_analog_walk_speed - 0.3).abs() < 1e-6);
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
        assert_eq!(
            p.cmc.aiming_yaw_angle_limit_degrees,
            before.cmc.aiming_yaw_angle_limit_degrees
        );
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
        assert_eq!(n.sample(0.0), n.walking);
        assert_eq!(n.sample(3.0), n.sprinting);
        assert!((n.sample(2.5) - 0.10).abs() < 1e-6);
        assert!((r.sample(1.5) - 0.075).abs() < 1e-6);
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

    #[test]
    fn grounded_dynamics_samples_match_the_live_source_contract() {
        let params = AlsMovementParams::default();
        let normal = params.grounded_dynamics(Stance::Standing, RotationMode::ViewDirection, 2.5);
        assert!((normal.acceleration - 9.197_034).abs() < 1e-4);
        assert!((normal.braking_deceleration - 8.75).abs() < 1e-6);
        assert!((normal.ground_friction - 2.25).abs() < 1e-6);

        let responsive =
            params.grounded_dynamics(Stance::Standing, RotationMode::VelocityDirection, 2.5);
        assert!((responsive.acceleration - 7.670_757).abs() < 1e-4);
        assert!((responsive.braking_deceleration - 10.605_449).abs() < 1e-4);
        assert!((responsive.ground_friction - 7.5).abs() < 1e-6);

        let sluggish = params
            .acceleration
            .grounded
            .sample(GroundedDynamicsProfile::Sluggish, 2.5);
        assert!((sluggish.acceleration - 2.445_746).abs() < 1e-4);
        assert!((sluggish.braking_deceleration - 4.518_285).abs() < 1e-4);
        assert!((sluggish.ground_friction - 1.734_373).abs() < 1e-4);
    }
}
