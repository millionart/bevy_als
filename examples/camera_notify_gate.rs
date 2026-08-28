use std::num::NonZeroU8;

use avian3d::prelude::{LinearVelocity, PhysicsPlugins};
use bevy::asset::AssetPlugin;
use bevy::ecs::message::MessageReader;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::time::Virtual;

use als_camera::{
    AlsCameraActiveClip, AlsCameraAnimationNotifyDriver, AlsCameraNotifyClip, AlsCameraPlugin,
    AlsCameraShakeRequest, AlsCameraTarget,
};
use als_core::roll::{AlsRollState, ROLL_DURATION_SECS};
use als_core::state::{Gait, Stance};

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
        .add_systems(PreUpdate, advance_roll_time)
        .add_systems(PostUpdate, verify)
        .run();
}

#[derive(Resource, Default)]
struct GateState {
    target: Option<Entity>,
    phase: u8,
    sprint_scales: Vec<f32>,
    roll_scales: Vec<f32>,
    roll_active_clip: bool,
}

fn setup(mut commands: Commands, mut state: ResMut<GateState>) {
    let target = commands
        .spawn((
            AlsCameraTarget,
            AlsCameraAnimationNotifyDriver,
            Gait::Sprinting,
            LinearVelocity(Vec3::new(6.5, 0.0, 0.0)),
        ))
        .id();
    state.target = Some(target);
}

fn advance_roll_time(
    time: Res<Time<Virtual>>,
    state: Res<GateState>,
    mut rolls: Query<&mut AlsRollState>,
) {
    if state.phase != 1 {
        return;
    }
    for mut roll in &mut rolls {
        roll.time = (roll.time + time.delta_secs()).min(ROLL_DURATION_SECS);
    }
}

fn verify(
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    mut state: ResMut<GateState>,
    mut shakes: MessageReader<AlsCameraShakeRequest>,
    mut gait: Query<&mut Gait>,
    clips: Query<&AlsCameraActiveClip>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(target) = state.target else {
        return;
    };
    for shake in shakes.read().filter(|shake| shake.target == target) {
        match state.phase {
            0 => state.sprint_scales.push(shake.scale),
            1 => state.roll_scales.push(shake.scale),
            _ => {}
        }
    }
    if state.phase == 0 && time.elapsed_secs() >= 1.35 {
        if let Ok(mut current) = gait.get_mut(target) {
            *current = Gait::Running;
        }
        commands.entity(target).insert(AlsRollState {
            active: true,
            time: 0.0,
            cooldown: 0.0,
            direction_yaw: 0.0,
            last_direction: default(),
            stance_before_roll: Stance::Standing,
        });
        state.phase = 1;
    }
    if state.phase == 1 {
        state.roll_active_clip |= clips
            .get(target)
            .is_ok_and(|clip| clip.clip == Some(AlsCameraNotifyClip::Roll));
        if time.elapsed_secs() >= 2.2 {
            let sprint = state.sprint_scales.len() >= 4
                && state
                    .sprint_scales
                    .iter()
                    .all(|scale| (*scale - 0.5).abs() < 1e-4);
            let roll = state.roll_scales.len() == 2
                && (state.roll_scales[0] - 1.0).abs() < 1e-4
                && (state.roll_scales[1] - 2.0).abs() < 1e-4;
            let pass = sprint && roll && state.roll_active_clip;
            println!(
                "[BCHAR20][NOTIFY] sprint_events={} sprint_scales={:?} roll_events={} roll_scales={:?} active_roll={} input_edge_used=false -> {}",
                state.sprint_scales.len(),
                state.sprint_scales,
                state.roll_scales.len(),
                state.roll_scales,
                state.roll_active_clip,
                if pass { "PASS" } else { "FAIL" }
            );
            exit.write(if pass {
                AppExit::Success
            } else {
                AppExit::Error(NonZeroU8::new(21).unwrap())
            });
            state.phase = 2;
        }
    }
}
