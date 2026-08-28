use avian3d::prelude::LinearVelocity;
use bevy::asset::Assets;
use bevy::prelude::*;

use crate::aim::{
    AlsLastFreeRotationMode, ROTATE_TO_VELOCITY_WHEN_SPRINTING, SPRINT_HAS_PRIORITY_OVER_AIMING,
};
use crate::character::{AlsCharacter, AlsMovementParamsHandle};
use crate::input::AlsInputCommand;
use crate::input_request::AlsFirstPersonView;
use crate::motion::AlsInputOverride;
use crate::params::{AlsMovementParams, StanceGaitSpeeds};
use crate::state::{Gait, LocomotionState, RotationMode, Stance};
use crate::stride_curves::{stride_curve_run, stride_curve_walk};
use crate::transition::{
    AlsStateRequest, Dimension, StateTransitionEvent, StateValue, state_transition_system,
};

pub const ACTUAL_GAIT_SPEED_OFFSET: f32 = 0.1;
pub const FIRST_PERSON_SPRINT_ANGLE_LIMIT_DEGREES: f32 = 50.0;
pub const STRIDE_BLEND_RANGE_MAX: f32 = 1.0;
pub const PLAY_RATE_MAX: f32 = 3.0;
pub const CROUCH_PLAY_RATE_MAX: f32 = 2.0;
pub const MIN_PLAY_RATE: f32 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsPlayRate {
    pub play_rate: f32,
    pub stride_blend: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaitWeights {
    pub move_w: f32,
    pub run_w: f32,
    pub sprint_w: f32,
}

impl Default for GaitWeights {
    fn default() -> Self {
        Self {
            move_w: 0.0,
            run_w: 0.0,
            sprint_w: 0.0,
        }
    }
}

impl GaitWeights {
    pub fn targets(gait: Gait) -> Self {
        match gait {
            Gait::Walking => Self {
                move_w: 1.0,
                run_w: 0.0,
                sprint_w: 0.0,
            },
            Gait::Running => Self {
                move_w: 1.0,
                run_w: 1.0,
                sprint_w: 0.0,
            },
            Gait::Sprinting => Self {
                move_w: 1.0,
                run_w: 1.0,
                sprint_w: 1.0,
            },
        }
    }

    pub fn from_gait_amount(gait_amount: f32) -> Self {
        let gait_amount = gait_amount.clamp(0.0, 3.0);
        Self {
            move_w: gait_amount.clamp(0.0, 1.0),
            run_w: (gait_amount - 1.0).clamp(0.0, 1.0),
            sprint_w: (gait_amount - 2.0).clamp(0.0, 1.0),
        }
    }

    pub fn running_amount(&self) -> f32 {
        self.run_w
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsDesiredGait(pub Gait);

impl Default for AlsDesiredGait {
    fn default() -> Self {
        Self(Gait::Running)
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsMaxAllowedGait(pub Gait);

impl Default for AlsMaxAllowedGait {
    fn default() -> Self {
        Self(Gait::Running)
    }
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsGaitAmount(pub f32);

pub fn input_view_angle_degrees(input: Vec2) -> f32 {
    input.x.atan2(input.y).to_degrees()
}

pub fn can_sprint(
    stance: Stance,
    first_person: bool,
    desired_rotation_mode: RotationMode,
    input: Vec2,
    aiming: bool,
) -> bool {
    if input.length_squared() <= 1e-6 || stance != Stance::Standing {
        return false;
    }
    if aiming && !SPRINT_HAS_PRIORITY_OVER_AIMING {
        return false;
    }
    if !first_person
        && (desired_rotation_mode == RotationMode::VelocityDirection
            || ROTATE_TO_VELOCITY_WHEN_SPRINTING)
    {
        return true;
    }
    input_view_angle_degrees(input).abs() < FIRST_PERSON_SPRINT_ANGLE_LIMIT_DEGREES
}

pub fn calculate_max_allowed_gait(
    desired: Gait,
    stance: Stance,
    first_person: bool,
    desired_rotation_mode: RotationMode,
    input: Vec2,
    aiming: bool,
) -> Gait {
    if desired == Gait::Sprinting
        && !can_sprint(stance, first_person, desired_rotation_mode, input, aiming)
    {
        Gait::Running
    } else {
        desired
    }
}

pub fn calculate_actual_gait(speed: f32, allowed: Gait, settings: &StanceGaitSpeeds) -> Gait {
    if speed < settings.max_walk_speed + ACTUAL_GAIT_SPEED_OFFSET {
        Gait::Walking
    } else if speed < settings.max_run_speed + ACTUAL_GAIT_SPEED_OFFSET
        || allowed != Gait::Sprinting
    {
        Gait::Running
    } else {
        Gait::Sprinting
    }
}

pub fn calculate_gait_amount(speed: f32, settings: &StanceGaitSpeeds) -> f32 {
    if speed > settings.max_run_speed {
        remap_clamped(
            speed,
            settings.max_run_speed,
            settings.max_sprint_speed,
            2.0,
            3.0,
        )
    } else if speed > settings.max_walk_speed {
        remap_clamped(
            speed,
            settings.max_walk_speed,
            settings.max_run_speed,
            1.0,
            2.0,
        )
    } else {
        remap_clamped(speed, 0.0, settings.max_walk_speed, 0.0, 1.0)
    }
}

pub fn stride_blend_amount(running_amount: f32) -> f32 {
    stride_blend_amount_for_speed(running_amount, 0.0)
}

pub fn stride_blend_amount_for_speed(running_amount: f32, speed_meters_per_second: f32) -> f32 {
    let speed_centimeters_per_second = speed_meters_per_second.max(0.0) * 100.0;
    let walk = stride_curve_walk(speed_centimeters_per_second);
    let run = stride_curve_run(speed_centimeters_per_second);
    walk.lerp(run, running_amount.clamp(0.0, STRIDE_BLEND_RANGE_MAX))
}

pub fn animated_gait_speed(params: &AlsMovementParams, stance: Stance, gait: Gait) -> f32 {
    let speeds = params.stance_speeds(stance);
    match gait {
        Gait::Walking => speeds.animated_walk_speed,
        Gait::Running => speeds.animated_run_speed,
        Gait::Sprinting => speeds.animated_sprint_speed,
    }
}

pub fn als_play_rate(
    params: &AlsMovementParams,
    stance: Stance,
    gait: Gait,
    speed_meters_per_second: f32,
    stride_blend: f32,
) -> AlsPlayRate {
    let speed = speed_meters_per_second.max(0.0);
    let stride_blend = stride_blend.max(f32::EPSILON);
    let play_rate = match stance {
        Stance::Crouching => {
            let animated_speed = params.crouch.animated_crouch_speed.max(f32::EPSILON);
            (speed / animated_speed / stride_blend).clamp(MIN_PLAY_RATE, CROUCH_PLAY_RATE_MAX)
        }
        Stance::Standing => {
            let speeds = &params.speeds.standing;
            let running_amount = (gait != Gait::Walking) as u8 as f32;
            let sprinting_amount = (gait == Gait::Sprinting) as u8 as f32;
            let walk_run = (speed / speeds.animated_walk_speed.max(f32::EPSILON)).lerp(
                speed / speeds.animated_run_speed.max(f32::EPSILON),
                running_amount,
            );
            let walk_run_sprint = walk_run.lerp(
                speed / speeds.animated_sprint_speed.max(f32::EPSILON),
                sprinting_amount,
            );
            (walk_run_sprint / stride_blend).clamp(MIN_PLAY_RATE, PLAY_RATE_MAX)
        }
    };
    AlsPlayRate {
        play_rate,
        stride_blend,
    }
}

fn remap_clamped(value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    let alpha = if (from_max - from_min).abs() <= f32::EPSILON {
        0.0
    } else {
        ((value - from_min) / (from_max - from_min)).clamp(0.0, 1.0)
    };
    to_min + (to_max - to_min) * alpha
}

pub(crate) fn gait_request_system(
    mut q: Query<(&mut AlsDesiredGait, &mut AlsStateRequest), With<AlsCharacter>>,
) {
    for (mut desired, mut request) in &mut q {
        if let Some(gait) = request.gait.take() {
            desired.0 = gait;
        }
    }
}

pub fn refresh_gait_system(
    mut q: Query<
        (
            Entity,
            &LocomotionState,
            &Stance,
            &RotationMode,
            Option<&AlsLastFreeRotationMode>,
            Has<AlsFirstPersonView>,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &AlsDesiredGait,
            &mut AlsMaxAllowedGait,
            &mut AlsGaitAmount,
            &mut Gait,
            &LinearVelocity,
            &AlsMovementParamsHandle,
        ),
        With<AlsCharacter>,
    >,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut transitions: MessageWriter<StateTransitionEvent>,
) {
    for (
        entity,
        locomotion,
        stance,
        rotation_mode,
        last_free,
        first_person,
        command,
        input_override,
        desired,
        mut max_allowed,
        mut gait_amount,
        mut gait,
        velocity,
        params_handle,
    ) in &mut q
    {
        if *locomotion != LocomotionState::Grounded {
            continue;
        }
        let Some(params) = params_assets.get(params_handle.id()) else {
            continue;
        };
        let input = input_override.map(|value| &value.0).unwrap_or(command);
        let desired_rotation_mode = last_free
            .map(|value| value.0)
            .filter(|value| *value != RotationMode::Aiming)
            .unwrap_or(*rotation_mode);
        let allowed = calculate_max_allowed_gait(
            desired.0,
            *stance,
            first_person,
            desired_rotation_mode,
            input.move_dir,
            input.aim_held,
        );
        let settings = params.gait_speeds(*stance, *rotation_mode);
        let speed = Vec2::new(velocity.x, velocity.z).length();
        max_allowed.0 = allowed;
        gait_amount.0 = calculate_gait_amount(speed, settings);
        let actual = calculate_actual_gait(speed, allowed, settings);
        if *gait != actual {
            let previous = *gait;
            *gait = actual;
            transitions.write(StateTransitionEvent {
                entity,
                dimension: Dimension::Gait,
                from: StateValue::Gait(previous),
                to: StateValue::Gait(actual),
            });
        }
    }
}

pub struct AlsGaitPlugin;

impl Plugin for AlsGaitPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StateTransitionEvent>().add_systems(
            Update,
            gait_request_system
                .after(crate::input_request::input_state_request_system)
                .before(refresh_gait_system)
                .before(crate::aim::aim_rotation_mode_system)
                .before(state_transition_system),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> StanceGaitSpeeds {
        StanceGaitSpeeds {
            max_walk_speed: 1.75,
            max_run_speed: 3.75,
            max_sprint_speed: 6.5,
            animated_walk_speed: 1.5,
            animated_run_speed: 3.5,
            animated_sprint_speed: 6.0,
        }
    }

    #[test]
    fn third_person_sprint_allows_any_input_when_velocity_rotation_is_enabled() {
        assert!(can_sprint(
            Stance::Standing,
            false,
            RotationMode::ViewDirection,
            Vec2::X,
            true,
        ));
    }

    #[test]
    fn first_person_sprint_requires_forward_input_within_source_angle_limit() {
        assert!(can_sprint(
            Stance::Standing,
            true,
            RotationMode::ViewDirection,
            Vec2::new(0.7, 1.0).normalize(),
            false,
        ));
        assert!(!can_sprint(
            Stance::Standing,
            true,
            RotationMode::ViewDirection,
            Vec2::X,
            false,
        ));
    }

    #[test]
    fn max_allowed_gait_demotes_invalid_sprint_to_running() {
        assert_eq!(
            calculate_max_allowed_gait(
                Gait::Sprinting,
                Stance::Standing,
                true,
                RotationMode::ViewDirection,
                Vec2::NEG_Y,
                false,
            ),
            Gait::Running
        );
        assert_eq!(
            calculate_max_allowed_gait(
                Gait::Sprinting,
                Stance::Standing,
                false,
                RotationMode::ViewDirection,
                Vec2::X,
                true,
            ),
            Gait::Sprinting
        );
    }

    #[test]
    fn actual_gait_uses_source_speed_thresholds() {
        let settings = settings();
        assert_eq!(
            calculate_actual_gait(1.84, Gait::Running, &settings),
            Gait::Walking
        );
        assert_eq!(
            calculate_actual_gait(1.86, Gait::Running, &settings),
            Gait::Running
        );
        assert_eq!(
            calculate_actual_gait(3.84, Gait::Sprinting, &settings),
            Gait::Running
        );
        assert_eq!(
            calculate_actual_gait(3.86, Gait::Sprinting, &settings),
            Gait::Sprinting
        );
    }

    #[test]
    fn gait_amount_matches_the_three_source_ranges() {
        let settings = settings();
        assert!((calculate_gait_amount(0.0, &settings) - 0.0).abs() < 1e-6);
        assert!((calculate_gait_amount(1.75, &settings) - 1.0).abs() < 1e-6);
        assert!((calculate_gait_amount(3.75, &settings) - 2.0).abs() < 1e-6);
        assert!((calculate_gait_amount(6.5, &settings) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn gait_weights_follow_the_source_curve_ranges() {
        assert_eq!(GaitWeights::from_gait_amount(0.0), GaitWeights::default());
        assert_eq!(
            GaitWeights::from_gait_amount(1.5),
            GaitWeights {
                move_w: 1.0,
                run_w: 0.5,
                sprint_w: 0.0,
            }
        );
        assert_eq!(
            GaitWeights::from_gait_amount(2.5),
            GaitWeights {
                move_w: 1.0,
                run_w: 1.0,
                sprint_w: 0.5,
            }
        );
    }
}
