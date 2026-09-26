use std::num::NonZeroU8;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_enhanced_input::prelude::*;

use bevy_als::als::character::AlsCharacter;
use bevy_als::als::control_scheme::AlsControlSchemeConfig;
use bevy_als::als::input::{
    AlsInputCommand, AlsInputPlugin, LookAction, LookMouseAction, OnFoot, als_input_bridge_system,
};
use bevy_als::als::motion::AlsViewYaw;
use bevy_als::als::params::AlsMovementParams;
use bevy_als::als::view::{AlsViewInputPlugin, AlsViewInputSet, AlsViewPitch, look_delta_radians};
use bevy_als::character_assembly::{AlsCharacterBlueprint, spawn_als_character_with_materials};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            AnimationGraphPlugin::default(),
            EnhancedInputPlugin,
            AlsInputPlugin,
            AlsViewInputPlugin,
        ))
        .init_asset::<StandardMaterial>()
        .init_asset::<AlsMovementParams>()
        .init_asset::<AlsControlSchemeConfig>()
        .init_resource::<GateState>()
        .add_systems(Startup, spawn_character)
        .add_systems(
            PreUpdate,
            inject_look_input
                .after(EnhancedInputSystems::Apply)
                .before(als_input_bridge_system),
        )
        .add_systems(Update, verify_look_input.after(AlsViewInputSet))
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    injected: bool,
    active_seen: bool,
    active_mouse: Vec2,
    active_rate: Vec2,
    active_delta: Vec2,
    yaw_after_active: f32,
    pitch_after_active: f32,
}

fn spawn_character(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    spawn_als_character_with_materials(
        &mut commands,
        &assets,
        &mut materials,
        &mut params_assets,
        &mut config_assets,
        AlsCharacterBlueprint::new(Transform::IDENTITY),
    );
}

fn inject_look_input(
    mut state: ResMut<GateState>,
    characters: Query<Entity, With<AlsCharacter>>,
    mut commands: Commands,
) {
    if state.injected {
        return;
    }
    let Ok(character) = characters.single() else {
        return;
    };
    commands
        .entity(character)
        .mock_once::<OnFoot, LookMouseAction>(TriggerState::Fired, Vec2::new(4.0, -2.0))
        .mock_once::<OnFoot, LookAction>(TriggerState::Fired, Vec2::new(0.5, -0.25));
    state.injected = true;
}

fn verify_look_input(
    mut state: ResMut<GateState>,
    characters: Query<(&AlsInputCommand, &AlsViewYaw, &AlsViewPitch), With<AlsCharacter>>,
    time: Res<Time>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((command, yaw, pitch)) = characters.single() else {
        return;
    };
    if !state.active_seen {
        if command.look_delta == Vec2::ZERO && command.look_rate == Vec2::ZERO {
            return;
        }
        let delta = look_delta_radians(command, time.delta_secs());
        let active_pass = (yaw.0 + delta.x).abs() < 1e-5
            && (pitch.0 + delta.y).abs() < 1e-5
            && command.look_delta == Vec2::new(4.0, -2.0)
            && command.look_rate == Vec2::new(0.5, -0.25);
        if !active_pass {
            error!(
                "[BCHAR2][VERDICT] phase=active mouse={:?} rate={:?} yaw={:.6} pitch={:.6} delta={delta:?} -> FAIL",
                command.look_delta, command.look_rate, yaw.0, pitch.0
            );
            exit.write(AppExit::Error(NonZeroU8::new(4).unwrap()));
            return;
        }
        state.active_seen = true;
        state.active_mouse = command.look_delta;
        state.active_rate = command.look_rate;
        state.active_delta = delta;
        state.yaw_after_active = yaw.0;
        state.pitch_after_active = pitch.0;
        return;
    }
    let released_pass = command.look_delta == Vec2::ZERO
        && command.look_rate == Vec2::ZERO
        && (yaw.0 - state.yaw_after_active).abs() < 1e-5
        && (pitch.0 - state.pitch_after_active).abs() < 1e-5;
    if released_pass {
        info!(
            "[BCHAR2][VERDICT] active_mouse={:?} active_rate={:?} active_delta={:?} active_yaw={:.6} active_pitch={:.6} released_mouse={:?} released_rate={:?} released=true -> PASS",
            state.active_mouse,
            state.active_rate,
            state.active_delta,
            state.yaw_after_active,
            state.pitch_after_active,
            command.look_delta,
            command.look_rate,
        );
        exit.write(AppExit::Success);
    } else {
        error!(
            "[BCHAR2][VERDICT] mouse={:?} rate={:?} yaw={:.6} pitch={:.6} released=false -> FAIL",
            command.look_delta, command.look_rate, yaw.0, pitch.0
        );
        exit.write(AppExit::Error(NonZeroU8::new(5).unwrap()));
    }
}
