use bevy::asset::Assets;
use bevy::prelude::*;

use crate::als::aim::{
    AlsLastFreeRotationMode, ROTATE_TO_VELOCITY_WHEN_SPRINTING, SPRINT_HAS_PRIORITY_OVER_AIMING,
};
use crate::als::character::{AlsCharacter, AlsMovementParamsHandle};
use crate::als::input::AlsInputCommand;
use crate::als::input_request::AlsFirstPersonView;
use crate::als::motion::{AlsActualVelocity, AlsInputOverride, AlsViewYaw};
use crate::als::params::{AlsMovementParams, GaitSpeedThresholds, StanceGaitSpeeds};
use crate::als::state::{Gait, LocomotionState, RotationMode, Stance};
use crate::als::stride_curves::{stride_curve_run, stride_curve_walk};
use crate::als::transition::{
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
            _ => Self { move_w: 1.0, run_w: 1.0, sprint_w: 0.0 },
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
    calculate_actual_gait_for_thresholds(speed, allowed, settings.gait_speed_thresholds(1.0))
}

pub fn calculate_actual_gait_for_thresholds(
    speed: f32,
    allowed: Gait,
    thresholds: GaitSpeedThresholds,
) -> Gait {
    if speed < thresholds.walk + ACTUAL_GAIT_SPEED_OFFSET {
        Gait::Walking
    } else if speed < thresholds.run + ACTUAL_GAIT_SPEED_OFFSET || allowed != Gait::Sprinting {
        Gait::Running
    } else {
        Gait::Sprinting
    }
}

pub fn calculate_gait_amount(speed: f32, settings: &StanceGaitSpeeds) -> f32 {
    calculate_gait_amount_for_thresholds(speed, settings.gait_speed_thresholds(1.0))
}

pub fn calculate_gait_amount_for_thresholds(speed: f32, thresholds: GaitSpeedThresholds) -> f32 {
    if speed > thresholds.run {
        remap_clamped(speed, thresholds.run, thresholds.sprint, 2.0, 3.0)
    } else if speed > thresholds.walk {
        remap_clamped(speed, thresholds.walk, thresholds.run, 1.0, 2.0)
    } else {
        remap_clamped(speed, 0.0, thresholds.walk, 0.0, 1.0)
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
        _ => speeds.animated_run_speed,
    }
}

pub fn als_play_rate(
    params: &AlsMovementParams,
    stance: Stance,
    speed_meters_per_second: f32,
    stride_blend: f32,
    weights: GaitWeights,
) -> AlsPlayRate {
    let speed = speed_meters_per_second.max(0.0);
    let stride_blend = stride_blend.max(f32::EPSILON);
    let play_rate = match stance {
        Stance::Crouching => {
            let animated_speed = params.crouch.animated_crouch_speed.max(f32::EPSILON);
            (speed / animated_speed / stride_blend).clamp(MIN_PLAY_RATE, CROUCH_PLAY_RATE_MAX)
        }
        _ => {
            let speeds = &params.speeds.standing;
            let walk_run = (speed / speeds.animated_walk_speed.max(f32::EPSILON)).lerp(
                speed / speeds.animated_run_speed.max(f32::EPSILON),
                weights.running_amount().clamp(0.0, 1.0),
            );
            let walk_run_sprint = walk_run.lerp(
                speed / speeds.animated_sprint_speed.max(f32::EPSILON),
                weights.sprint_w.clamp(0.0, 1.0),
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
    time: Res<Time<Virtual>>,
    mut q: Query<
        (
            Entity,
            &LocomotionState,
            &Stance,
            &RotationMode,
            (&AlsViewYaw, &crate::actual_motion::AlsActualMotion),
            Option<&AlsLastFreeRotationMode>,
            Has<AlsFirstPersonView>,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            &AlsDesiredGait,
            &mut AlsMaxAllowedGait,
            &mut AlsGaitAmount,
            &mut Gait,
            &AlsActualVelocity,
            &AlsMovementParamsHandle,
        ),
        With<AlsCharacter>,
    >,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut transitions: MessageWriter<StateTransitionEvent>,
) {
    if time.is_paused() { return; }
    for (
        entity,
        locomotion,
        stance,
        rotation_mode,
        (view_yaw, motion),
        last_free,
        first_person,
        command,
        input_override,
        desired,
        mut max_allowed,
        mut gait_amount,
        mut gait,
        actual_velocity,
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
            if motion.has_input {
                crate::als::motion::view_relative_move_axes(Vec2::Y, motion.input_yaw_degrees.to_radians() - view_yaw.0)
            } else {
                Vec2::ZERO
            },
            input.aim_held,
        );
        let speed = actual_velocity.horizontal_speed;
        let thresholds = params.gait_speed_thresholds(
            *stance,
            *rotation_mode,
            actual_velocity.world_velocity,
            view_yaw.0,
        );
        max_allowed.0 = allowed;
        gait_amount.0 = calculate_gait_amount_for_thresholds(speed, thresholds);
        let actual = calculate_actual_gait_for_thresholds(speed, allowed, thresholds);
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
                .after(crate::als::input_request::input_state_request_system)
                .before(state_transition_system),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_sprint_cancel_preserves_the_actual_source_max_allowed_gait() {
        use crate::als::input::{AlsInputMap, AlsInputPlugin, spawn_als_actions};
        use bevy::input::gamepad::{RawGamepadButtonChangedEvent, RawGamepadEvent};
        use bevy::time::TimeUpdateStrategy;
        use bevy_enhanced_input::prelude::EnhancedInputPlugin;
        for gamepad_input in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, bevy::input::InputPlugin, EnhancedInputPlugin, AlsInputPlugin))
                .init_resource::<Assets<AlsMovementParams>>()
                .add_message::<StateTransitionEvent>()
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(20)))
                .add_systems(Update, crate::als::input_request::input_state_request_system)
                .add_systems(PostUpdate, refresh_gait_system);
            app.finish();
            let handle = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
            let character = app.world_mut().spawn((AlsCharacter, LocomotionState::Grounded, Stance::Standing,
                RotationMode::VelocityDirection, AlsViewYaw(0.0), AlsDesiredGait(Gait::Running),
                AlsMaxAllowedGait(Gait::Running), AlsGaitAmount::default(), Gait::Walking,
                AlsActualVelocity::default(), AlsMovementParamsHandle(handle), AlsStateRequest::default(),
                crate::actual_motion::AlsActualMotion { has_input: true, ..default() })).id();
            let gamepad = app.world_mut().spawn(Gamepad::default()).id();
            spawn_als_actions(&mut app.world_mut().commands(), character, &AlsInputMap::default());
            app.update();
            if gamepad_input {
                app.world_mut().write_message(RawGamepadEvent::Button(
                    RawGamepadButtonChangedEvent::new(gamepad, GamepadButton::East, 1.0)));
            } else { app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft); }
            for _ in 0..2 { app.update(); }
            assert_eq!(app.world().get::<AlsDesiredGait>(character).unwrap().0, Gait::Running);
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
            app.update();
            assert_eq!(app.world().get::<AlsDesiredGait>(character).unwrap().0, Gait::Sprinting,
                "Source Sprint Canceled callback preserves its actual positive value during pause");
            assert_eq!(app.world().get::<AlsMaxAllowedGait>(character).unwrap().0, Gait::Running,
                "Source paused Character must retain its last refreshed max allowed gait");
            app.world_mut().resource_mut::<Time<Virtual>>().unpause();
            app.update();
            assert_eq!(app.world().get::<AlsMaxAllowedGait>(character).unwrap().0, Gait::Sprinting,
                "Source resumed Character must consume its saved desired gait");
        }
    }
    #[test]
    fn gait_permission_reads_consumed_character_input_and_world_direction() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<Assets<AlsMovementParams>>()
            .add_message::<StateTransitionEvent>().add_systems(Update, refresh_gait_system);
        let handle = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let entity = app.world_mut().spawn((AlsCharacter, LocomotionState::Grounded, Stance::Standing,
            RotationMode::ViewDirection, AlsViewYaw(0.0), AlsFirstPersonView,
            AlsDesiredGait(Gait::Sprinting), AlsMaxAllowedGait::default(), AlsGaitAmount::default(),
            Gait::Walking, AlsActualVelocity::default(), AlsMovementParamsHandle(handle))).id();
        for (has_input, input_yaw, raw_input, expected) in [
            (false, 0.0_f32, Vec2::Y, Gait::Running),
            (true, 49.0, Vec2::ZERO, Gait::Sprinting),
            (true, 51.0, Vec2::Y, Gait::Running),
            (true, 0.0, Vec2::Y * 0.0002, Gait::Sprinting),
        ] {
            app.world_mut().entity_mut(entity).insert((
                AlsInputCommand { move_dir: raw_input, ..default() },
                crate::actual_motion::AlsActualMotion { has_input, input_yaw_degrees: input_yaw, ..default() },
            ));
            app.update();
            assert_eq!(app.world().get::<AlsMaxAllowedGait>(entity).unwrap().0, expected,
                "Source gait permission differs: has_input={has_input}, yaw={input_yaw}, raw={raw_input:?}");
        }
    }

    fn settings() -> StanceGaitSpeeds {
        StanceGaitSpeeds {
            max_walk_speed: 1.75,
            max_run_speed: 3.75,
            max_sprint_speed: 6.5,
            animated_walk_speed: 1.5,
            animated_run_speed: 3.5,
            animated_sprint_speed: 6.0,
            ..StanceGaitSpeeds::default()
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

    #[test]
    fn als_play_rate_uses_continuous_gait_weights() {
        let params = AlsMovementParams::default();
        let rate = als_play_rate(
            &params,
            Stance::Standing,
            3.0,
            1.0,
            GaitWeights {
                move_w: 1.0,
                run_w: 0.5,
                sprint_w: 0.0,
            },
        );
        println!("[BF101.6] continuous_play_rate={:.6}", rate.play_rate);
        assert!((rate.play_rate - 1.428_571_5).abs() < 1e-6);
    }

    #[test]
    fn refresh_gait_uses_direction_dependent_thresholds() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Assets::<AlsMovementParams>::default())
            .add_message::<StateTransitionEvent>()
            .add_systems(Update, refresh_gait_system);
        let mut params = AlsMovementParams::default();
        params
            .speeds
            .standing
            .allow_direction_dependent_movement_speed = true;
        params.speeds.standing.max_walk_backward_speed = 1.0;
        params.speeds.standing.max_run_backward_speed = 2.0;
        let handle = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(params);
        let entity = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LocomotionState::Grounded,
                Stance::Standing,
                RotationMode::ViewDirection,
                AlsViewYaw::default(),
                AlsInputCommand::default(),
                AlsDesiredGait(Gait::Running),
                AlsMaxAllowedGait(Gait::Running),
                AlsGaitAmount::default(),
                Gait::Walking,
                AlsActualVelocity {
                    world_velocity: Vec3::Z * 1.2,
                    horizontal_speed: 1.2,
                },
                crate::actual_motion::AlsActualMotion::default(),
                AlsMovementParamsHandle(handle),
            ))
            .id();

        app.update();

        let entity = app.world().entity(entity);
        let gait = entity.get::<Gait>().unwrap();
        let amount = entity.get::<AlsGaitAmount>().unwrap();
        println!(
            "[BF101.5] backward_gait={gait:?} gait_amount={:.6}",
            amount.0
        );
        assert_eq!(*gait, Gait::Running);
        assert!((amount.0 - 1.2).abs() < 1e-6);
    }

    #[test]
    fn refresh_gait_uses_final_velocity_and_rechecks_sprint_permission() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Assets::<AlsMovementParams>::default())
            .add_message::<StateTransitionEvent>()
            .add_systems(Update, refresh_gait_system);
        let handle = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        let entity = app
            .world_mut()
            .spawn((
                AlsCharacter,
                LocomotionState::Grounded,
                Stance::Standing,
                RotationMode::VelocityDirection,
                AlsViewYaw::default(),
                AlsInputCommand {
                    move_dir: Vec2::X,
                    ..default()
                },
                AlsDesiredGait(Gait::Sprinting),
                AlsMaxAllowedGait::default(),
                AlsGaitAmount::default(),
                Gait::Walking,
                AlsActualVelocity::default(),
                crate::actual_motion::AlsActualMotion { has_input: true, input_yaw_degrees: -90.0, ..default() },
                AlsMovementParamsHandle(handle),
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<AlsMaxAllowedGait>(entity).unwrap().0,
            Gait::Sprinting
        );
        assert_eq!(*app.world().get::<Gait>(entity).unwrap(), Gait::Walking);

        *app.world_mut()
            .get_mut::<AlsActualVelocity>(entity)
            .unwrap() = AlsActualVelocity {
            world_velocity: Vec3::NEG_Z * 6.5,
            horizontal_speed: 6.5,
        };
        app.update();
        assert_eq!(*app.world().get::<Gait>(entity).unwrap(), Gait::Sprinting);

        app.world_mut()
            .get_mut::<AlsInputCommand>(entity)
            .unwrap()
            .move_dir = Vec2::ZERO;
        app.world_mut().get_mut::<crate::actual_motion::AlsActualMotion>(entity).unwrap().has_input = false;
        app.update();
        assert_eq!(
            app.world().get::<AlsMaxAllowedGait>(entity).unwrap().0,
            Gait::Running
        );
        assert_eq!(*app.world().get::<Gait>(entity).unwrap(), Gait::Running);
    }
}
