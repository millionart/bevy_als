use std::num::NonZeroU8;

use avian3d::prelude::{LinearVelocity, PhysicsPlugins};
use bevy::asset::AssetPlugin;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;
use bevy::time::Virtual;

use als_camera::{
    AlsCameraActiveClip, AlsCameraAnimationNotifyDriver, AlsCameraAnimationTiming,
    AlsCameraNotifyClip, AlsCameraNotifyTimeline, AlsCameraPlugin, AlsCameraShakeRequest,
    AlsCameraTarget,
};
use als_core::state::Gait;

const SPRINT_SAMPLES: [f32; 3] = [0.07, 0.35, 0.40];

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            }),
            PhysicsPlugins::new(FixedPostUpdate),
            AlsCameraPlugin,
        ))
        .init_resource::<GateState>()
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, advance_source_animation_time)
        .add_systems(PostUpdate, verify)
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    target: Option<Entity>,
    sample_index: usize,
    events: Vec<(f32, f32)>,
    finished: bool,
}

fn setup(mut commands: Commands, mut state: ResMut<GateState>) {
    let target = commands
        .spawn((
            AlsCameraTarget,
            AlsCameraAnimationNotifyDriver,
            AlsCameraAnimationTiming {
                sprint_time: Some(0.05),
            },
            AlsCameraActiveClip::default(),
            AlsCameraNotifyTimeline {
                previous_clip: Some(AlsCameraNotifyClip::Sprint),
                previous_time: 0.05,
            },
            Gait::Sprinting,
            LinearVelocity(Vec3::new(6.5, 0.0, 0.0)),
        ))
        .id();
    state.target = Some(target);
}

fn advance_source_animation_time(
    mut state: ResMut<GateState>,
    mut timing: Query<&mut AlsCameraAnimationTiming>,
) {
    let Some(sample) = SPRINT_SAMPLES.get(state.sample_index).copied() else {
        return;
    };
    for mut value in &mut timing {
        value.sprint_time = Some(sample);
    }
    state.sample_index += 1;
}

fn verify(
    time: Res<Time<Virtual>>,
    mut state: ResMut<GateState>,
    mut shakes: MessageReader<AlsCameraShakeRequest>,
    clips: Query<&AlsCameraActiveClip>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.finished {
        return;
    }
    let Some(target) = state.target else {
        return;
    };
    let active_time = clips.get(target).map_or(0.0, |clip| clip.time);
    for shake in shakes.read().filter(|shake| shake.target == target) {
        state.events.push((active_time, shake.scale));
    }
    if state.sample_index == SPRINT_SAMPLES.len() {
        let pass = state.events.len() == 2
            && state.events.get(0).is_some_and(|(time, scale)| {
                (time - 0.07).abs() <= 1e-5 && (*scale - 0.5).abs() <= 1e-5
            })
            && state.events.get(1).is_some_and(|(time, scale)| {
                (time - 0.40).abs() <= 1e-5 && (*scale - 0.5).abs() <= 1e-5
            });
        println!(
            "[BF102.53][GATE] source_samples={SPRINT_SAMPLES:?} shake_events={:?} virtual_elapsed={:.6} -> {}",
            state.events,
            time.elapsed_secs(),
            if pass { "PASS" } else { "FAIL" },
        );
        state.finished = true;
        exit.write(if pass {
            AppExit::Success
        } else {
            AppExit::Error(NonZeroU8::new(53).unwrap())
        });
    } else if time.elapsed_secs() >= 5.0 {
        println!(
            "[BF102.53][GATE] source_samples={SPRINT_SAMPLES:?} shake_events={:?} virtual_elapsed={:.6} -> FAIL",
            state.events,
            time.elapsed_secs(),
        );
        state.finished = true;
        exit.write(AppExit::Error(NonZeroU8::new(54).unwrap()));
    }
}
