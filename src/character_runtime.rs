use als_core::air::AlsAirState;
use als_core::character::{AlsCharacter, AlsMovementParamsHandle};
use als_core::flail::AlsFlailState;
use als_core::params::AlsMovementParams;
use als_core::start_stop::{StartStopState, start_stop_detector_system};
use als_core::state::{Gait, Stance};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::AnimationGraphPlayer;
use bevy_animation_graph::core::plugin::AnimationGraphSet;

use crate::air_jump_layer::AirJumpLayerPlugin;
use crate::character_assembly::{AlsCharacterAvatar, AvatarGroundCompensationPlugin};
use crate::crouching_layer::{AlsCrouchingLayerSet, CrouchingLayerPlugin};
use crate::direction_layer::DirectionLayerPlugin;
use crate::foot_ik_apply::FootIkApplyPlugin;
use crate::gait_layer::{AlsGaitBlend, AlsGaitLayerSet, GaitLayerPlugin};
use crate::grounded_layer::{AlsGroundedLayerSet, GroundedLayerPlugin};
use crate::locomotion_layer::{AlsLocomotionLayerSet, LocomotionLayerPlugin};
use crate::mantle_drive::AlsMantleDrivePlugin;
use crate::motion_graph_link::{
    compute_motion_graph_inputs, compute_motion_graph_inputs_blended, write_air_pins,
    write_motion_graph_pins,
};
use crate::overlay_layer::OverlayLayerPlugin;
use crate::ragdoll_link::AlsRagdollLinkPlugin;
use crate::roll_drive::AlsRollDrivePlugin;
use crate::stance_layer::StanceLayerPlugin;
use crate::standing_layer::{AlsStandingLayerSet, StandingLayerPlugin};
use crate::stop_link::AlsStopLinkPlugin;
use crate::turn_layer::TurnLayerPlugin;
use crate::view_layer::ViewLayerPlugin;

pub struct AlsCharacterRuntimePlugin;

impl Plugin for AlsCharacterRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            AvatarGroundCompensationPlugin,
            als_core::foot_ik::AlsFootProbePlugin,
            FootIkApplyPlugin,
            GaitLayerPlugin,
            DirectionLayerPlugin,
            StandingLayerPlugin,
            CrouchingLayerPlugin,
            GroundedLayerPlugin,
            AirJumpLayerPlugin,
            StanceLayerPlugin,
            OverlayLayerPlugin,
            ViewLayerPlugin,
            TurnLayerPlugin,
            AlsRollDrivePlugin,
            AlsStopLinkPlugin,
        ))
        .add_plugins((
            LocomotionLayerPlugin,
            AlsMantleDrivePlugin,
            AlsRagdollLinkPlugin,
        ))
        .add_systems(
            PostUpdate,
            runtime_graph_router
                .after(AlsGaitLayerSet)
                .before(AlsStandingLayerSet)
                .before(AlsCrouchingLayerSet)
                .before(AlsGroundedLayerSet)
                .before(AlsLocomotionLayerSet)
                .before(AnimationGraphSet::Final),
        )
        .add_systems(
            Update,
            (
                ensure_start_stop_state,
                start_stop_detector_system.after(ensure_start_stop_state),
            ),
        );
    }
}

fn ensure_start_stop_state(
    mut commands: Commands,
    characters: Query<Entity, (With<AlsCharacter>, Without<StartStopState>)>,
) {
    for character in &characters {
        commands.entity(character).insert(StartStopState::default());
    }
}

type RuntimeCharacterQuery = (
    &'static Stance,
    &'static Gait,
    &'static LinearVelocity,
    &'static AlsMovementParamsHandle,
    Option<&'static AlsAirState>,
    Option<&'static AlsFlailState>,
);

fn runtime_graph_router(
    characters: Query<(Entity, RuntimeCharacterQuery), With<AlsCharacter>>,
    instances: Query<(&ChildOf, &AnimatedSceneInstance), With<AlsCharacterAvatar>>,
    blends: Query<&AlsGaitBlend>,
    mut players: Query<&mut AnimationGraphPlayer>,
    params: Res<Assets<AlsMovementParams>>,
) {
    for (character, (stance, gait, velocity, handle, air, flail)) in &characters {
        let Some(params) = params.get(handle.id()) else {
            continue;
        };
        let horizontal_speed = Vec2::new(velocity.x, velocity.z).length();
        let air = air.copied().unwrap_or_default();
        let flail = flail.copied().unwrap_or_default();
        for (child_of, instance) in &instances {
            if child_of.parent() != character {
                continue;
            }
            let Ok(mut player) = players.get_mut(instance.player_entity()) else {
                continue;
            };
            let inputs = match blends.get(instance.player_entity()) {
                Ok(blend) => compute_motion_graph_inputs_blended(
                    params,
                    *stance,
                    *gait,
                    horizontal_speed,
                    blend.speed_ema,
                    blend.weights,
                ),
                Err(_) => compute_motion_graph_inputs(params, *stance, *gait, horizontal_speed),
            };
            write_motion_graph_pins(&mut player, &inputs);
            write_air_pins(&mut player, &air, &flail);
        }
    }
}
