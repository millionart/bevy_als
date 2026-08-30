use std::num::NonZeroU8;

use avian3d::prelude::PhysicsPlugins;
use bevy::asset::AssetPlugin;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::time::Virtual;

use als_core::character::AlsCharacter;
use als_core::input::AlsInputCommand;
use als_core::input_request::AlsFirstPersonView;
use als_core::motion::AlsViewYaw;
use als_core::state::RotationMode;
use bevy_als::head_layer::{AlsHeadLayerSet, AlsHeadState, HeadLayerPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            PhysicsPlugins::new(FixedPostUpdate),
            HeadLayerPlugin,
        ))
        .init_resource::<GateState>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, verify.after(AlsHeadLayerSet))
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    frames: u32,
    finished: bool,
}

fn setup(mut commands: Commands, mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.set_relative_speed(0.1);
    commands.spawn((
        AlsCharacter,
        Transform::IDENTITY,
        AlsViewYaw(0.0),
        RotationMode::ViewDirection,
        AlsInputCommand::default(),
        AlsFirstPersonView,
        AlsHeadState {
            initialization_required: false,
            pitch_angle: 90.0,
            ..default()
        },
    ));
}

fn verify(
    mut state: ResMut<GateState>,
    head: Query<&AlsHeadState, With<AlsCharacter>>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.finished {
        return;
    }
    state.frames += 1;
    let Ok(head) = head.single() else {
        return;
    };
    if state.frames >= 8 {
        let pass =
            head.pitch_angle.abs() < 5.0 && (virtual_time.relative_speed() - 0.1).abs() <= 1e-6;
        println!(
            "[BF102.57][GATE] frames={} virtual_speed={:.3} pitch={:.6} -> {}",
            state.frames,
            virtual_time.relative_speed(),
            head.pitch_angle,
            if pass { "PASS" } else { "FAIL" },
        );
        virtual_time.set_relative_speed(1.0);
        state.finished = true;
        exit.write(if pass {
            AppExit::Success
        } else {
            AppExit::Error(NonZeroU8::new(57).unwrap())
        });
    } else if state.frames >= 300 {
        println!(
            "[BF102.57][GATE] frames={} virtual_speed={:.3} pitch={:.6} -> FAIL",
            state.frames,
            virtual_time.relative_speed(),
            head.pitch_angle,
        );
        virtual_time.set_relative_speed(1.0);
        state.finished = true;
        exit.write(AppExit::Error(NonZeroU8::new(58).unwrap()));
    }
}
