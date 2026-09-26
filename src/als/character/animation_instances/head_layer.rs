use crate::als_camera::AlsCameraCustomTimeDilation;
use crate::als::character::AlsCharacter;
use crate::als::head::AlsHeadUpdateInput as HeadUpdateInput;
#[cfg(test)]
use crate::als::head::update_head_state;
pub use crate::als::head::{
    AlsHeadSettings, AlsHeadState, critical_spring_damper, damper_exact_angle, inv_exp_approx,
    unwind_degrees,
};
use crate::als::input_request::AlsFirstPersonView;
use crate::als::motion::AlsRotationResponseState;
use crate::als::orientation::world_forward_to_yaw;
use crate::als::state::RotationMode;
use crate::als::view::AlsViewPitch;
use crate::extensions::{source_instance::source_player_property, unscaled_delta_seconds};
use bevy::prelude::*;
use bevy::time::{Real, Virtual};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;

use crate::animation_context_projection::{AlsRootAnimationContext, AlsRootAnimationContextSet};
use crate::character_assembly::AlsCharacterAvatar;
use crate::generated_ab_als_provider::GeneratedAbAlsProviderSet;
use crate::layering_layer::AlsLayeringLayerSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlsHeadLayerSet;

pub struct HeadLayerPlugin;

impl Plugin for HeadLayerPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            AlsHeadLayerSet
                .after(AlsLayeringLayerSet)
                .after(GeneratedAbAlsProviderSet)
                .before(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate),
        )
        .add_systems(PostUpdate, head_layer_writer.in_set(AlsHeadLayerSet))
        .add_systems(
            PostUpdate,
            project_head_state
                .after(crate::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate)
                .before(AlsRootAnimationContextSet),
        );
    }
}

fn player_scalar(player: &AnimationGraphPlayer, key: &str, default: f32) -> f32 {
    player
        .get_global_input_data(key)
        .and_then(|value| value.as_f32().ok())
        .unwrap_or(default)
}

fn body_yaw_degrees(transform: &Transform) -> f32 {
    world_forward_to_yaw(*transform.forward()).to_degrees()
}

pub(crate) fn head_blend_amount(context: Option<&AlsRootAnimationContext>) -> f32 {
    let curve = |key| {
        context
            .and_then(|context| context.f32(key))
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    };
    (1.0 - curve("ViewBlock")) * (1.0 - curve("PoseAiming"))
}

#[allow(clippy::too_many_arguments)]
fn head_frame_inputs(
    player: &AnimationGraphPlayer,
    transform: &Transform,
    response: Option<&AlsRotationResponseState>,
    pitch_radians: f32,
    timing: (f32, f32),
    rotation_mode: RotationMode,
    first_person: bool,
    action_active: bool,
) -> HeadUpdateInput {
    let body_yaw = -body_yaw_degrees(transform);
    let previous_body_yaw = player_scalar(player, "locomotion_state.rotation_yaw", body_yaw);
    let pending_update = player
        .get_global_input_data("animation.pending_update")
        .and_then(|value| value.as_bool().ok())
        .unwrap_or(true);
    let forward = transform.forward();
    let body_pitch = forward
        .y
        .atan2(Vec2::new(forward.x, forward.z).length())
        .to_degrees();
    HeadUpdateInput {
        delta_seconds: timing.0,
        delta_seconds_without_time_dilation: timing.1,
        first_person,
        rotation_mode,
        has_input: player
            .get_global_input_data("locomotion_state.b_has_input")
            .and_then(|value| value.as_bool().ok())
            .unwrap_or(false),
        input_yaw_world: player_scalar(player, "locomotion_state.input_yaw_angle_world_space", 0.0),
        target_yaw_world: response
            .filter(|response| response.initialized)
            .map_or(body_yaw, |response| -response.target_yaw.to_degrees()),
        body_yaw_world: body_yaw,
        body_yaw_velocity_world: if !pending_update && timing.0 > 1e-8 {
            unwind_degrees(body_yaw - previous_body_yaw) / timing.0
        } else {
            0.0
        },
        view_yaw_angle: player_scalar(player, "view_state.yaw_angle", 0.0),
        view_pitch_angle: if action_active {
            player_scalar(player, "view_state.pitch_angle", 0.0)
        } else {
            unwind_degrees(pitch_radians.to_degrees() - body_pitch)
        },
        movement_base_relative_yaw: 0.0,
    }
}

fn write_head_inputs(
    player: &mut AnimationGraphPlayer,
    input: HeadUpdateInput,
    context: Option<&AlsRootAnimationContext>,
) {
    for (key, value) in [
        (
            "animation.delta_seconds_without_time_dilation",
            input.delta_seconds_without_time_dilation,
        ),
        (
            "locomotion_state.input_yaw_angle_world_space",
            input.input_yaw_world,
        ),
        (
            "locomotion_state.target_yaw_angle_world_space",
            input.target_yaw_world,
        ),
        ("locomotion_state.rotation_yaw", input.body_yaw_world),
        (
            "locomotion_state.yaw_velocity_world_space",
            input.body_yaw_velocity_world,
        ),
        ("view_state.pitch_angle", input.view_pitch_angle),
        ("view_state.pitch_amount", 0.5 - input.view_pitch_angle / 180.0),
        ("view_state.head_blend_amount", head_blend_amount(context)),
    ] {
        player.set_global_input_data(key, value);
    }
    player.set_global_input_data("view_mode.first_person", input.first_person);
    player.set_global_input_data(
        "rotation_mode.velocity_direction",
        input.rotation_mode == RotationMode::VelocityDirection,
    );
}

#[allow(clippy::type_complexity)]
fn head_layer_writer(
    source_inputs: Query<&crate::extensions::SourceAnimationInputs>,
    virtual_time: Res<Time<Virtual>>,
    real_time: Res<Time<Real>>,
    characters: Query<
        (
            &Transform,
            Option<&AlsViewPitch>,
            &RotationMode,
            Option<&AlsRotationResponseState>,
            Has<AlsFirstPersonView>,
            Option<&AlsCameraCustomTimeDilation>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    mut players: Query<(&mut AnimationGraphPlayer, Option<&AlsRootAnimationContext>)>,
) {
    for (parent, instance) in &avatars {
        let Ok((
            transform,
            pitch,
            rotation_mode,
            response,
            first_person,
            custom_dilation,
        )) = characters.get(parent.parent())
        else {
            continue;
        };
        let Ok((mut player, context)) = players.get_mut(instance.player_entity()) else {
            continue;
        };
        let mut previous_dilation =
            player_scalar(&player, "animation.previous_global_time_dilation", 1.0);
        let custom_dilation = custom_dilation.map_or(1.0, |value| value.0);
        let scaled_delta = virtual_time.delta_secs() * custom_dilation;
        let unscaled = unscaled_delta_seconds(
            &mut previous_dilation,
            scaled_delta,
            real_time.delta_secs(),
            virtual_time.effective_speed(),
            custom_dilation,
        );
        player.set_global_input_data("animation.previous_global_time_dilation", previous_dilation);
        let action_active = source_inputs.get(instance.player_entity()).ok().and_then(|inputs| inputs.gameplay_tag_input("locomotion_action")).is_some_and(|tag| tag.is_valid());
        let input = head_frame_inputs(
            &player,
            transform,
            response,
            pitch.map_or(0.0, |value| value.0),
            (scaled_delta, unscaled),
            *rotation_mode,
            first_person,
            action_active,
        );
        write_head_inputs(&mut player, input, context);
    }
}

pub(crate) fn head_state_from_player(player: &AnimationGraphPlayer) -> AlsHeadState {
    let scalar = |key, default| {
        source_player_property(player, key)
            .and_then(|value| value.as_f32().ok())
            .unwrap_or(default)
    };
    let boolean = |key, default| {
        source_player_property(player, key)
            .and_then(|value| value.as_bool().ok())
            .unwrap_or(default)
    };
    AlsHeadState {
        initialization_required: boolean("head_state.b_initialization_required", true),
        switching_look_sides: boolean("head_state.b_switching_look_sides", false),
        pitch_angle: scalar("head_state.pitch_angle", 0.0),
        yaw_angle: scalar("head_state.yaw_angle", 0.0),
        yaw_velocity: scalar("head_state.yaw_velocity", 0.0),
        yaw_amount: scalar("head_state.yaw_amount", 0.5),
    }
}

fn project_head_state(
    mut commands: Commands,
    avatars: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
) {
    for (parent, instance) in &avatars {
        if let Ok(player) = players.get(instance.player_entity()) {
            commands
                .entity(parent.parent())
                .insert(head_state_from_player(player));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::{input::AlsInputCommand, motion::AlsViewYaw};

    #[test]
    fn actor_input_publication_does_not_refresh_an_unevaluated_head() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(PostUpdate, head_layer_writer);
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        let state = AlsHeadState::default();
        let entity = app
            .world_mut()
            .spawn((
                AlsCharacter,
                Transform::default(),
                AlsViewYaw(90.0_f32.to_radians()),
                RotationMode::ViewDirection,
                AlsInputCommand::default(),
                state,
            ))
            .id();
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(*app.world().get::<AlsHeadState>(entity).unwrap(), state);
    }

    #[test]
    fn source_view_and_aiming_curves_gate_head_blend() {
        assert_eq!(head_blend_amount(None), 1.0);
        for (view, aiming, expected) in [
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.25, 0.5, 0.375),
            (-2.0, -1.0, 1.0),
            (2.0, 0.0, 0.0),
        ] {
            let context = AlsRootAnimationContext {
                curves: [("ViewBlock".into(), view), ("PoseAiming".into(), aiming)]
                    .into_iter()
                    .collect(),
            };
            assert_eq!(head_blend_amount(Some(&context)), expected);
        }
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
    fn head_actor_inputs_use_source_coordinates_raw_pitch_retained_target_and_action_latch() {
        let mut player = AnimationGraphPlayer::new(default());
        player.set_global_input_data("animation.pending_update", false);
        player.set_global_input_data("locomotion_state.rotation_yaw", -20.0_f32);
        player.set_global_input_data("locomotion_state.b_has_input", true);
        player.set_global_input_data("locomotion_state.input_yaw_angle_world_space", -80.0_f32);
        player.set_global_input_data("view_state.yaw_angle", -45.0_f32);
        let mut response = AlsRotationResponseState {
            initialized: true,
            input_yaw: Some(12.0_f32.to_radians()),
            target_yaw: 55.0_f32.to_radians(),
            ..default()
        };
        let transform = Transform::from_rotation(Quat::from_rotation_y(30.0_f32.to_radians()));
        let input = head_frame_inputs(
            &player,
            &transform,
            Some(&response),
            65.0_f32.to_radians(),
            (0.1, 0.2),
            RotationMode::VelocityDirection,
            false,
            false,
        );
        for (actual, expected) in [
            (input.body_yaw_world, -30.0),
            (input.input_yaw_world, -80.0),
            (input.target_yaw_world, -55.0),
            (input.body_yaw_velocity_world, -100.0),
            (input.view_yaw_angle, -45.0),
            (input.view_pitch_angle, 65.0),
        ] {
            assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
        }
        assert!(input.has_input);
        let context = AlsRootAnimationContext {
            curves: [("ViewBlock".into(), 0.25), ("PoseAiming".into(), 0.5)]
                .into_iter()
                .collect(),
        };
        write_head_inputs(&mut player, input, Some(&context));
        assert_eq!(
            player_scalar(&player, "view_state.head_blend_amount", -1.0),
            0.375
        );
        assert!(
            player
                .get_global_input_data("head_state.pitch_angle")
                .is_none()
        );
        assert!(
            player
                .get_global_input_data("head_state.yaw_amount")
                .is_none()
        );
        assert_eq!(head_state_from_player(&player), AlsHeadState::default());
        response.input_yaw = None;
        player.set_global_input_data("locomotion_state.b_has_input", false);
        let action = head_frame_inputs(
            &player,
            &transform,
            Some(&response),
            -85.0_f32.to_radians(),
            (0.1, 0.2),
            RotationMode::ViewDirection,
            true,
            true,
        );
        assert_eq!(action.view_pitch_angle, input.view_pitch_angle);
        assert_eq!(action.input_yaw_world, input.input_yaw_world);
        assert_eq!(action.target_yaw_world, input.target_yaw_world);
        assert_eq!(action.delta_seconds_without_time_dilation, 0.2);
        let rotated = transform.with_rotation(Quat::from_rotation_y(60.0_f32.to_radians()));
        player.set_global_input_data("animation.pending_update", true);
        let pending = head_frame_inputs(
            &player,
            &rotated,
            Some(&response),
            -85.0_f32.to_radians(),
            (0.1, 0.2),
            RotationMode::ViewDirection,
            true,
            false,
        );
        assert_eq!(pending.body_yaw_velocity_world, 0.0);
        assert!((pending.view_pitch_angle + 85.0).abs() < 1e-5);
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

    #[test]
    fn generated_pitch_amount_uses_the_retained_source_pitch_angle() {
        let mut player = AnimationGraphPlayer::new(Handle::default());
        for angle in [-90.0_f32, 0.0, 65.0, 90.0] {
            let input = head_frame_inputs(&player, &Transform::IDENTITY, None, angle.to_radians(), (0.01, 0.01), RotationMode::ViewDirection, false, false);
            write_head_inputs(&mut player, input, None);
            assert!((player.get_global_input_data("view_state.pitch_amount").unwrap().as_f32().unwrap() - (0.5 - angle / 180.0)).abs() < 1e-6);
            let held = head_frame_inputs(&player, &Transform::IDENTITY, None, -0.7, (0.01, 0.01), RotationMode::ViewDirection, false, true);
            write_head_inputs(&mut player, held, None);
            assert!((player.get_global_input_data("view_state.pitch_amount").unwrap().as_f32().unwrap() - (0.5 - angle / 180.0)).abs() < 1e-6);
        }
    }
}
