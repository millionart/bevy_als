use als_camera::AlsCameraCustomTimeDilation;
use als_core::character::AlsCharacter;
use als_core::input::AlsInputCommand;
use als_core::input_request::AlsFirstPersonView;
use als_core::motion::{AlsInputOverride, AlsViewYaw, view_relative_move_direction};
use als_core::orientation::world_forward_to_yaw;
use als_core::state::RotationMode;
use als_core::view::AlsViewSmoothed;
use als_ext::unscaled_delta_seconds;
use bevy::ecs::hierarchy::ChildOf;
use bevy::prelude::*;
use bevy::time::{Real, Virtual};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

use crate::animation_context_projection::AlsRootAnimationContext;
use crate::character_assembly::AlsCharacterAvatar;
use crate::layering_layer::AlsLayeringLayerSet;

const COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD: f32 = 5.0;
const LOCOMOTION_YAW_SPEED_THRESHOLD: f32 = 20.0;
const SWITCH_SIDES_CURRENT_YAW_THRESHOLD: f32 = 90.0;
const SWITCH_SIDES_TARGET_YAW_THRESHOLD: f32 = 160.0;
const SWITCH_SIDES_COMPLETE_YAW_DELTA: f32 = 10.0;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AlsHeadState {
    pub initialization_required: bool,
    pub switching_look_sides: bool,
    pub pitch_angle: f32,
    pub yaw_angle: f32,
    pub yaw_velocity: f32,
    pub yaw_amount: f32,
}

impl Default for AlsHeadState {
    fn default() -> Self {
        Self {
            initialization_required: true,
            switching_look_sides: false,
            pitch_angle: 0.0,
            yaw_angle: 0.0,
            yaw_velocity: 0.0,
            yaw_amount: 0.5,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AlsHeadCurveState {
    pub view_block_amount: f32,
    pub pose_aiming_amount: f32,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AlsHeadMovementBaseState {
    pub has_relative_rotation: bool,
    pub delta_yaw_degrees: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct HeadRuntime {
    initialized_body_yaw: bool,
    previous_body_yaw: f32,
    previous_global_time_dilation: f32,
}

impl Default for HeadRuntime {
    fn default() -> Self {
        Self {
            initialized_body_yaw: false,
            previous_body_yaw: 0.0,
            previous_global_time_dilation: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlsHeadSettings {
    pub pitch_half_life: f32,
    pub yaw_half_life: f32,
    pub switch_look_sides_yaw_half_life: f32,
    pub first_person_pitch_half_life: f32,
    pub first_person_yaw_half_life: f32,
}

impl Default for AlsHeadSettings {
    fn default() -> Self {
        Self {
            pitch_half_life: 0.1,
            yaw_half_life: 0.1,
            switch_look_sides_yaw_half_life: 0.2,
            first_person_pitch_half_life: 0.01,
            first_person_yaw_half_life: 0.01,
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsHeadLayerSet;

#[derive(Clone, Copy, Debug)]
struct HeadUpdateInput {
    delta_seconds: f32,
    delta_seconds_without_time_dilation: f32,
    first_person: bool,
    rotation_mode: RotationMode,
    has_input: bool,
    input_yaw_world: f32,
    target_yaw_world: f32,
    body_yaw_world: f32,
    body_yaw_velocity_world: f32,
    view_yaw_angle: f32,
    view_pitch_angle: f32,
    movement_base_relative_yaw: f32,
}

pub struct HeadLayerPlugin;

impl Plugin for HeadLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsHeadLayerSet
                .after(AlsLayeringLayerSet)
                .before(AnimationGraphSet::Final),
        )
        .add_systems(PostUpdate, head_layer_writer.in_set(AlsHeadLayerSet))
        .add_systems(
            FixedPostUpdate,
            bridge_head_curve_states.after(AnimationGraphSet::PrePhysics),
        );
    }
}

pub fn unwind_degrees(mut angle: f32) -> f32 {
    while angle > 180.0 {
        angle -= 360.0;
    }
    while angle < -180.0 {
        angle += 360.0;
    }
    angle
}

pub fn inv_exp_approx(value: f32) -> f32 {
    1.0 / (1.0
        + 1.00746054 * value
        + 0.45053901 * value * value
        + 0.25724632 * value * value * value)
}

pub fn damper_exact_angle(current: f32, target: f32, delta_seconds: f32, half_life: f32) -> f32 {
    let mut delta = unwind_degrees(target - current);
    if delta.abs() <= 1e-4 {
        return target;
    }
    if delta > 175.0 {
        delta -= 360.0;
    }
    let alpha =
        1.0 - inv_exp_approx(std::f32::consts::LN_2 * delta_seconds.max(0.0) / (half_life + 1e-8));
    unwind_degrees(current + delta * alpha)
}

pub fn critical_spring_damper(
    current: f32,
    velocity: &mut f32,
    target: f32,
    delta_seconds: f32,
    half_life: f32,
) -> f32 {
    let smoothing_time = half_life / std::f32::consts::LN_2;
    if smoothing_time < 1e-8 {
        *velocity = 0.0;
        return target;
    }
    let y = 2.0 / smoothing_time.max(1e-8);
    let j0 = current - target;
    let j1 = *velocity + j0 * y;
    let e = inv_exp_approx(y * delta_seconds.max(0.0));
    let result = e * (j0 + j1 * delta_seconds.max(0.0)) + target;
    *velocity = e * (*velocity - j1 * y * delta_seconds.max(0.0));
    result
}

fn body_yaw_degrees(transform: &Transform) -> f32 {
    let forward = transform.rotation * Vec3::NEG_Z;
    world_forward_to_yaw(forward).to_degrees()
}

fn input_yaw_degrees(input: &AlsInputCommand, view_yaw: f32, fallback: f32) -> f32 {
    if input.move_dir.length_squared() <= 1e-6 {
        fallback
    } else {
        world_forward_to_yaw(view_relative_move_direction(input.move_dir, view_yaw)).to_degrees()
    }
}

fn head_blend_amount(curves: AlsHeadCurveState) -> f32 {
    (1.0 - curves.view_block_amount.clamp(0.0, 1.0))
        * (1.0 - curves.pose_aiming_amount.clamp(0.0, 1.0))
}

fn head_curve_state_from_values(
    view_block: Option<f32>,
    pose_aiming: Option<f32>,
) -> AlsHeadCurveState {
    AlsHeadCurveState {
        view_block_amount: view_block.unwrap_or(0.0).clamp(0.0, 1.0),
        pose_aiming_amount: pose_aiming.unwrap_or(0.0).clamp(0.0, 1.0),
    }
}

fn bridge_head_curve_states(
    mut commands: Commands,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<(&AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
    mut characters: Query<Option<&mut AlsHeadCurveState>, With<AlsCharacter>>,
) {
    for (parent, instance) in &avatars {
        let Ok((_, context)) = players.get(instance.player_entity()) else {
            continue;
        };
        let Ok(current) = characters.get_mut(parent.parent()) else {
            continue;
        };
        let curves = head_curve_state_from_values(
            context.and_then(|context| context.f32("ViewBlock")),
            context.and_then(|context| context.f32("PoseAiming")),
        );
        if let Some(mut current) = current {
            *current = curves;
        } else {
            commands.entity(parent.parent()).insert(curves);
        }
    }
}

fn update_head_state(state: &mut AlsHeadState, input: HeadUpdateInput, settings: AlsHeadSettings) {
    if input.movement_base_relative_yaw != 0.0 {
        state.yaw_angle += input.movement_base_relative_yaw;
    }
    state.yaw_angle -= input.body_yaw_velocity_world * input.delta_seconds;
    state.yaw_angle = state.yaw_angle.clamp(-180.0, 180.0);
    let mut ready_to_switch_look_sides = false;
    let (target_pitch, target_yaw) = if input.rotation_mode == RotationMode::VelocityDirection {
        let mut target_yaw = unwind_degrees(
            (if input.has_input {
                input.input_yaw_world
            } else {
                input.target_yaw_world
            }) - input.body_yaw_world,
        );
        if input.body_yaw_velocity_world.abs() > LOCOMOTION_YAW_SPEED_THRESHOLD {
            target_yaw = input.body_yaw_velocity_world.signum() * target_yaw.abs();
        } else if target_yaw.abs() > 180.0 - COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD {
            target_yaw = state.yaw_angle;
        }
        state.switching_look_sides = false;
        (0.0, target_yaw)
    } else {
        let mut target_yaw = input.view_yaw_angle.clamp(
            -180.0 + COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD,
            180.0 - COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD,
        );
        if !input.first_person {
            if state.yaw_angle >= SWITCH_SIDES_CURRENT_YAW_THRESHOLD
                && target_yaw <= -SWITCH_SIDES_TARGET_YAW_THRESHOLD
            {
                target_yaw = 180.0 - COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD;
                ready_to_switch_look_sides = !input.has_input;
            } else if state.yaw_angle <= -SWITCH_SIDES_CURRENT_YAW_THRESHOLD
                && target_yaw >= SWITCH_SIDES_TARGET_YAW_THRESHOLD
            {
                target_yaw = -180.0 + COUNTER_CLOCKWISE_ROTATION_ANGLE_THRESHOLD;
                ready_to_switch_look_sides = !input.has_input;
            }
        }
        (input.view_pitch_angle, target_yaw)
    };
    if state.initialization_required {
        state.initialization_required = false;
        state.pitch_angle = target_pitch;
        state.yaw_angle = target_yaw;
        state.yaw_velocity = 0.0;
        state.switching_look_sides = false;
    } else {
        let effective_delta_seconds = if input.first_person {
            input.delta_seconds_without_time_dilation
        } else {
            input.delta_seconds
        };
        state.pitch_angle = damper_exact_angle(
            state.pitch_angle,
            target_pitch,
            effective_delta_seconds,
            if input.first_person {
                settings.first_person_pitch_half_life
            } else {
                settings.pitch_half_life
            },
        );
        if ready_to_switch_look_sides {
            state.switching_look_sides = true;
        } else if unwind_degrees(target_yaw - state.yaw_angle).abs()
            <= SWITCH_SIDES_COMPLETE_YAW_DELTA
        {
            state.switching_look_sides = false;
        }
        state.yaw_angle = critical_spring_damper(
            state.yaw_angle,
            &mut state.yaw_velocity,
            target_yaw,
            input.delta_seconds,
            if state.switching_look_sides {
                settings.switch_look_sides_yaw_half_life
            } else if input.first_person {
                settings.first_person_yaw_half_life
            } else {
                settings.yaw_half_life
            },
        );
    }
    state.yaw_amount = state.yaw_angle / 360.0 + 0.5;
}

fn write_globals(
    player: &mut AnimationGraphPlayer,
    state: AlsHeadState,
    curves: AlsHeadCurveState,
) {
    for (key, value) in [
        ("als_head_pitch_angle", state.pitch_angle),
        ("als_head_yaw_amount", state.yaw_amount),
        ("als_head_blend_amount", head_blend_amount(curves)),
    ] {
        player.set_global_input_data(key, value);
    }
}

#[allow(clippy::type_complexity)]
fn head_layer_writer(
    virtual_time: Res<Time<Virtual>>,
    real_time: Res<Time<Real>>,
    mut commands: Commands,
    mut characters: Query<
        (
            Entity,
            &Transform,
            &AlsViewYaw,
            Option<&AlsViewSmoothed>,
            &RotationMode,
            &AlsInputCommand,
            Option<&AlsInputOverride>,
            Has<AlsFirstPersonView>,
            Option<&AlsCameraCustomTimeDilation>,
            Option<&AlsHeadCurveState>,
            Option<&AlsHeadMovementBaseState>,
            Option<&mut AlsHeadState>,
            Option<&mut HeadRuntime>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<&mut AnimationGraphPlayer>,
) {
    let delta_seconds = virtual_time.delta_secs();
    for (
        entity,
        transform,
        view_yaw,
        view_pitch,
        rotation_mode,
        command,
        override_command,
        first_person,
        custom_time_dilation,
        curve_state,
        movement_base,
        current_state,
        current_runtime,
    ) in &mut characters
    {
        let effective = override_command.map_or(command, |value| &value.0);
        let mut state = current_state.as_deref().copied().unwrap_or_default();
        let mut runtime = current_runtime.as_deref().copied().unwrap_or_default();
        let body_yaw = body_yaw_degrees(transform);
        let body_yaw_velocity = if runtime.initialized_body_yaw && delta_seconds > 1e-6 {
            unwind_degrees(body_yaw - runtime.previous_body_yaw) / delta_seconds
        } else {
            0.0
        };
        runtime.initialized_body_yaw = true;
        runtime.previous_body_yaw = body_yaw;
        let delta_seconds_without_time_dilation = unscaled_delta_seconds(
            &mut runtime.previous_global_time_dilation,
            delta_seconds,
            real_time.delta_secs(),
            virtual_time.effective_speed(),
            custom_time_dilation.map_or(1.0, |dilation| dilation.0),
        );
        let curves = curve_state.copied().unwrap_or_default();
        let movement_base = movement_base.copied().unwrap_or_default();
        let input_yaw = input_yaw_degrees(effective, view_yaw.0, body_yaw);
        update_head_state(
            &mut state,
            HeadUpdateInput {
                delta_seconds,
                delta_seconds_without_time_dilation,
                first_person,
                rotation_mode: *rotation_mode,
                has_input: effective.move_dir.length_squared() > 1e-6,
                input_yaw_world: input_yaw,
                target_yaw_world: body_yaw,
                body_yaw_world: body_yaw,
                body_yaw_velocity_world: body_yaw_velocity,
                view_yaw_angle: unwind_degrees(view_yaw.0.to_degrees() - body_yaw),
                view_pitch_angle: view_pitch.map_or(0.0, |value| value.0.to_degrees()),
                movement_base_relative_yaw: if movement_base.has_relative_rotation {
                    movement_base.delta_yaw_degrees
                } else {
                    0.0
                },
            },
            AlsHeadSettings::default(),
        );
        if let Some(mut current_state) = current_state {
            *current_state = state;
        } else {
            commands.entity(entity).insert(state);
        }
        if let Some(mut current_runtime) = current_runtime {
            *current_runtime = runtime;
        } else {
            commands.entity(entity).insert(runtime);
        }
        for (parent, instance) in &avatars {
            if parent.parent() != entity {
                continue;
            }
            if let Ok(mut player) = players.get_mut(instance.player_entity()) {
                write_globals(&mut player, state, curves);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_view_and_aiming_curves_gate_head_blend() {
        let neutral = head_curve_state_from_values(None, None);
        let blocked = head_curve_state_from_values(Some(1.0), Some(0.0));
        let aiming = head_curve_state_from_values(Some(0.0), Some(1.0));
        let mixed = head_curve_state_from_values(Some(0.25), Some(0.5));
        assert_eq!(head_blend_amount(neutral), 1.0);
        assert_eq!(head_blend_amount(blocked), 0.0);
        assert_eq!(head_blend_amount(aiming), 0.0);
        assert!((head_blend_amount(mixed) - 0.375).abs() < 1e-6);
    }

    #[test]
    fn exact_angle_damper_uses_source_half_life() {
        let half = damper_exact_angle(0.0, 90.0, 0.1, 0.1);
        assert!((half - 45.0).abs() < 0.1);
    }

    #[test]
    fn first_person_pitch_uses_the_unscaled_delta() {
        let base = AlsHeadState {
            initialization_required: false,
            pitch_angle: 90.0,
            ..default()
        };
        let mut unscaled = base;
        let mut scaled = base;
        let input = HeadUpdateInput {
            delta_seconds: 0.01,
            delta_seconds_without_time_dilation: 0.1,
            first_person: true,
            rotation_mode: RotationMode::ViewDirection,
            has_input: false,
            input_yaw_world: 0.0,
            target_yaw_world: 0.0,
            body_yaw_world: 0.0,
            body_yaw_velocity_world: 0.0,
            view_yaw_angle: 0.0,
            view_pitch_angle: 0.0,
            movement_base_relative_yaw: 0.0,
        };
        update_head_state(&mut unscaled, input, AlsHeadSettings::default());
        update_head_state(
            &mut scaled,
            HeadUpdateInput {
                delta_seconds_without_time_dilation: input.delta_seconds,
                ..input
            },
            AlsHeadSettings::default(),
        );
        let passed = unscaled.pitch_angle < 1.0 && scaled.pitch_angle > 20.0;
        let verdict = if passed { "PASS" } else { "FAIL" };
        println!(
            "[BF102.57][VERDICT] unscaled_pitch={:.6} scaled_pitch={:.6} -> {verdict}",
            unscaled.pitch_angle, scaled.pitch_angle,
        );
        assert!(passed);
    }

    #[test]
    fn head_yaw_uses_the_same_world_forward_convention_as_view_yaw() {
        let transform =
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        assert!((body_yaw_degrees(&transform) - 90.0).abs() < 1e-4);
        let input = AlsInputCommand {
            move_dir: Vec2::Y,
            ..default()
        };
        assert!((input_yaw_degrees(&input, std::f32::consts::FRAC_PI_2, 0.0) - 90.0).abs() < 1e-4);
    }

    #[test]
    fn critical_spring_snaps_for_zero_half_life() {
        let mut velocity = 12.0;
        assert_eq!(
            critical_spring_damper(20.0, &mut velocity, -30.0, 0.1, 0.0),
            -30.0
        );
        assert_eq!(velocity, 0.0);
    }

    #[test]
    fn initialization_matches_source_targets_and_yaw_amount() {
        let mut state = AlsHeadState::default();
        update_head_state(
            &mut state,
            HeadUpdateInput {
                delta_seconds: 1.0 / 60.0,
                delta_seconds_without_time_dilation: 1.0 / 60.0,
                first_person: false,
                rotation_mode: RotationMode::ViewDirection,
                has_input: false,
                input_yaw_world: 0.0,
                target_yaw_world: 0.0,
                body_yaw_world: 0.0,
                body_yaw_velocity_world: 0.0,
                view_yaw_angle: 90.0,
                view_pitch_angle: 30.0,
                movement_base_relative_yaw: 0.0,
            },
            AlsHeadSettings::default(),
        );
        assert_eq!(state.pitch_angle, 30.0);
        assert_eq!(state.yaw_angle, 90.0);
        assert!((state.yaw_amount - 0.75).abs() < 1e-6);
    }

    #[test]
    fn third_person_side_switch_waits_for_input_release() {
        let mut state = AlsHeadState {
            initialization_required: false,
            yaw_angle: 100.0,
            ..default()
        };
        update_head_state(
            &mut state,
            HeadUpdateInput {
                delta_seconds: 1.0 / 60.0,
                delta_seconds_without_time_dilation: 1.0 / 60.0,
                first_person: false,
                rotation_mode: RotationMode::ViewDirection,
                has_input: true,
                input_yaw_world: 0.0,
                target_yaw_world: 0.0,
                body_yaw_world: 0.0,
                body_yaw_velocity_world: 0.0,
                view_yaw_angle: -170.0,
                view_pitch_angle: 0.0,
                movement_base_relative_yaw: 0.0,
            },
            AlsHeadSettings::default(),
        );
        assert!(!state.switching_look_sides);
    }
}
