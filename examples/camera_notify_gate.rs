use std::num::NonZeroU8;

use als_camera::{AlsCameraPlugin, AlsCameraShakeRequest};
use als_core::character::AlsCharacter;
use als_core::mantle::{AlsMantleState, MantleKind};
use als_core::roll::{AlsRollState, ROLL_PLAY_RATE};
use als_ext::AlsExtPlugin;
use avian3d::prelude::PhysicsPlugins;
use bevy::asset::AssetPlugin;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy_als::animation_events::{
    AlsAnimationEvent, AlsAnimationEventPlugin, AlsAnimationEventSet, AlsNotifyEarlyBlendOut,
    AlsNotifyLocomotionAction, AlsNotifyManifest, AlsNotifyRootMotionScale, AlsStandingEventClock,
};
use bevy_animation_graph::AnimationGraphPlugin;
use bevy_animation_graph::core::animation_clip::GraphClip;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "assets".into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                }),
        )
        .add_plugins((
            PhysicsPlugins::new(FixedPostUpdate),
            AnimationGraphPlugin::from_physics_schedule(Update),
            AlsExtPlugin,
            AlsCameraPlugin,
            AlsAnimationEventPlugin,
        ))
        .init_resource::<GateState>()
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, drive)
        .add_systems(
            FixedPostUpdate,
            observe.after(AlsAnimationEventSet::Dispatch),
        )
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    target: Option<Entity>,
    phase: u8,
    ready_frames: u16,
    sprint_scales: Vec<f32>,
    sprint_acceleration_scales: Vec<f32>,
    roll_scales: Vec<f32>,
    marker_names: Vec<String>,
    rolling_observed: bool,
    mantling_observed: bool,
    early_blend_out_observed: bool,
    grounded_from_roll: bool,
    root_scale: f32,
    root_scale_observed: bool,
}

fn setup(mut commands: Commands, mut state: ResMut<GateState>) {
    let target = commands
        .spawn((
            AlsCharacter,
            AlsStandingEventClock {
                sprint_time: Some(0.0),
                sprint_weight: 1.0,
                sprint_acceleration_weight: 0.0,
            },
            AlsRollState::default(),
            AlsMantleState::default(),
        ))
        .id();
    state.target = Some(target);
}

fn set_sprint(clock: &mut AlsStandingEventClock, time: f32) {
    *clock = AlsStandingEventClock {
        sprint_time: Some(time),
        sprint_weight: 1.0,
        sprint_acceleration_weight: 0.0,
    };
}

fn set_sprint_acceleration(clock: &mut AlsStandingEventClock, time: f32) {
    *clock = AlsStandingEventClock {
        sprint_time: Some(time),
        sprint_weight: 1.0,
        sprint_acceleration_weight: 1.0,
    };
}

fn set_roll(roll: &mut AlsRollState, clip_time: f32) {
    roll.active = true;
    roll.time = clip_time / ROLL_PLAY_RATE;
}

fn drive(
    mut state: ResMut<GateState>,
    manifests: Res<Assets<AlsNotifyManifest>>,
    graph_clips: Res<Assets<GraphClip>>,
    mut clocks: Query<&mut AlsStandingEventClock>,
    mut rolls: Query<&mut AlsRollState>,
    mut mantles: Query<&mut AlsMantleState>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(target) = state.target else {
        return;
    };
    let Ok(mut clock) = clocks.get_mut(target) else {
        return;
    };
    if state.phase == 0 {
        set_sprint(&mut clock, 0.0);
        state.ready_frames += 1;
        if !manifests.is_empty() && !graph_clips.is_empty() {
            state.phase = 1;
        } else if state.ready_frames > 600 {
            println!("[BF103][NOTIFY] assets_ready=false -> FAIL");
            exit.write(AppExit::Error(NonZeroU8::new(31).unwrap()));
        }
        return;
    }
    match state.phase {
        1 => set_sprint(&mut clock, 0.0),
        2 => set_sprint(&mut clock, 0.07),
        3 => set_sprint(&mut clock, 0.37),
        4 => set_sprint_acceleration(&mut clock, 0.0),
        5 => set_sprint_acceleration(&mut clock, 0.07),
        6 => set_sprint_acceleration(&mut clock, 0.37),
        7 => {
            *clock = AlsStandingEventClock::default();
            if let Ok(mut roll) = rolls.get_mut(target) {
                set_roll(&mut roll, 0.0);
            }
        }
        8 => {
            if let Ok(mut roll) = rolls.get_mut(target) {
                set_roll(&mut roll, 0.11);
            }
        }
        9 => {
            if let Ok(mut roll) = rolls.get_mut(target) {
                set_roll(&mut roll, 0.47);
            }
        }
        10 => {
            if let Ok(mut roll) = rolls.get_mut(target) {
                set_roll(&mut roll, 0.94);
            }
        }
        11 => {
            if let Ok(mut roll) = rolls.get_mut(target) {
                roll.active = false;
            }
            if let Ok(mut mantle) = mantles.get_mut(target) {
                mantle.begin(MantleKind::High, Vec3::ZERO, Vec3::ONE, 2.0, 0.0, 0.0);
                mantle.time = 0.8;
            }
        }
        12 => {
            if let Ok(mut mantle) = mantles.get_mut(target) {
                mantle.time = 1.9;
            }
        }
        _ => {}
    }
    state.phase += 1;
}

fn observe(
    state: ResMut<GateState>,
    mut animation_events: MessageReader<AlsAnimationEvent>,
    mut shakes: MessageReader<AlsCameraShakeRequest>,
    actions: Query<&AlsNotifyLocomotionAction>,
    root_scales: Query<&AlsNotifyRootMotionScale>,
    early_blend_outs: Query<&AlsNotifyEarlyBlendOut>,
    entry_modes: Query<&bevy_als::grounded_layer::AlsGroundedEntryMode>,
    mut exit: MessageWriter<AppExit>,
) {
    let state = state.into_inner();
    let Some(target) = state.target else {
        return;
    };
    for event in animation_events
        .read()
        .filter(|event| event.character == target)
    {
        if event.class_name == "AlsAnimNotify_CameraShake" {
            println!(
                "[BF103][EVENT] source={} time={:.6} phase={:?} weight={:.3}",
                event.source_asset, event.time, event.phase, event.weight,
            );
        }
        if event.class_name == "AuthoredSyncMarker" {
            state.marker_names.push(event.name.clone());
        }
    }
    for shake in shakes.read().filter(|shake| shake.target == target) {
        if (3..=4).contains(&state.phase) {
            state.sprint_scales.push(shake.scale);
        } else if (6..=7).contains(&state.phase) {
            state.sprint_acceleration_scales.push(shake.scale);
        } else {
            state.roll_scales.push(shake.scale);
        }
    }
    state.rolling_observed |= actions
        .get(target)
        .is_ok_and(|action| *action == AlsNotifyLocomotionAction::Rolling);
    state.mantling_observed |= actions
        .get(target)
        .is_ok_and(|action| *action == AlsNotifyLocomotionAction::Mantling);
    state.early_blend_out_observed |= early_blend_outs
        .get(target)
        .is_ok_and(|value| value.active && (value.blend_out_duration - 0.4).abs() < 1e-4);
    state.root_scale = root_scales.get(target).map_or(1.0, |scale| scale.0);
    state.root_scale_observed |= (state.root_scale - 1.3).abs() < 1e-4;
    state.grounded_from_roll |= entry_modes
        .get(target)
        .is_ok_and(|mode| *mode == bevy_als::grounded_layer::AlsGroundedEntryMode::FromRoll);
    if state.phase < 14 {
        return;
    }
    let sprint = state.sprint_scales == [0.5, 0.5];
    let sprint_acceleration = state.sprint_acceleration_scales == [1.25, 1.25];
    let roll = state.roll_scales == [1.0, 2.0];
    let markers = state.marker_names.iter().any(|name| name == "Left")
        && state.marker_names.iter().any(|name| name == "Right");
    let pass = sprint
        && sprint_acceleration
        && roll
        && markers
        && state.rolling_observed
        && state.mantling_observed
        && state.early_blend_out_observed
        && state.grounded_from_roll
        && state.root_scale_observed;
    println!(
        "[BF103][NOTIFY] sprint={:?} sprint_acceleration={:?} roll={:?} markers={:?} rolling={} mantling={} early_blend_out={} grounded_from_roll={} root_scale_seen={} root_scale={:.3} -> {}",
        state.sprint_scales,
        state.sprint_acceleration_scales,
        state.roll_scales,
        state.marker_names,
        state.rolling_observed,
        state.mantling_observed,
        state.early_blend_out_observed,
        state.grounded_from_roll,
        state.root_scale_observed,
        state.root_scale,
        if pass { "PASS" } else { "FAIL" },
    );
    exit.write(if pass {
        AppExit::Success
    } else {
        AppExit::Error(NonZeroU8::new(32).unwrap())
    });
}
