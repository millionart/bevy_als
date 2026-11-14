use std::num::NonZeroU8;

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_als::als::character::AlsCharacter;
use bevy_als::als::foot_ik::{AlsDynamicTransitionState, AlsFootLockState};
use bevy_als::als::state::Stance;
use bevy_als::als::transitions::{TransitionConfig, TransitionSequence};

#[derive(Resource, Default)]
struct GatePhase(u8);

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .init_resource::<GatePhase>()
        .add_systems(Startup, setup)
        .add_systems(Update, drive_gate)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        AlsCharacter,
        Stance::Standing,
        AlsFootLockState {
            valid: true,
            ..Default::default()
        },
        AlsDynamicTransitionState::default(),
    ));
}

fn drive_gate(
    mut phase: ResMut<GatePhase>,
    mut q: Query<
        (
            &Stance,
            &mut AlsFootLockState,
            &mut AlsDynamicTransitionState,
        ),
        With<AlsCharacter>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((stance, mut foot_lock, mut dynamic)) = q.single_mut() else {
        return;
    };
    let config = TransitionConfig::default();
    match phase.0 {
        0 => {
            foot_lock.left.lock_amount = 1.0;
            foot_lock.left.target_location = bevy::math::DVec3::ZERO;
            foot_lock.left.lock_location = bevy::math::DVec3::ZERO;
            let none = dynamic
                .update(&foot_lock, true, 1.0, *stance, &config)
                .is_none();
            println!(
                "[BCHAR18D][LOCK] threshold={:.3} planted_none={none}",
                config.dynamic_foot_lock_distance
            );
        }
        1 => {
            foot_lock.left.target_location = Vec3::new(0.2, 0.0, 0.0).as_dvec3();
            let selection = dynamic.update(&foot_lock, true, 1.0, *stance, &config);
            let pass = selection.is_some_and(|selection| {
                selection.sequence == TransitionSequence::StandingLeft
                    && (selection.play_rate - 1.5).abs() <= 1e-6
                    && (selection.blend_duration - 0.2).abs() <= 1e-6
            }) && dynamic.frames_remaining == 2
                && dynamic.left_distance_squared > config.dynamic_foot_lock_distance.powi(2);
            println!(
                "[BCHAR18D][TRIGGER] selection={selection:?} d2=({:.4},{:.4}) delay={} -> {}",
                dynamic.left_distance_squared,
                dynamic.right_distance_squared,
                dynamic.frames_remaining,
                if pass { "PASS" } else { "FAIL" }
            );
            if !pass {
                exit.write(AppExit::Error(NonZeroU8::new(18).unwrap()));
            }
        }
        2 => {
            let blocked = dynamic
                .update(&foot_lock, true, 1.0, *stance, &config)
                .is_none()
                && dynamic.frames_remaining == 1;
            println!(
                "[BCHAR18D][BLOCK] first_blocked={blocked} delay={}",
                dynamic.frames_remaining
            );
            if !blocked {
                exit.write(AppExit::Error(NonZeroU8::new(19).unwrap()));
            }
        }
        3 => {
            let blocked = dynamic
                .update(&foot_lock, true, 1.0, *stance, &config)
                .is_none()
                && dynamic.frames_remaining == 0;
            println!(
                "[BCHAR18D][BLOCK] second_blocked={blocked} delay={} -> {}",
                dynamic.frames_remaining,
                if blocked { "PASS" } else { "FAIL" }
            );
            if blocked {
                exit.write(AppExit::Success);
            } else {
                exit.write(AppExit::Error(NonZeroU8::new(20).unwrap()));
            }
        }
        _ => {}
    }
    phase.0 += 1;
}
