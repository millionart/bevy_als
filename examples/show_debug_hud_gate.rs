use std::time::Duration;

use avian3d::prelude::LinearVelocity;
use bevy::app::ScheduleRunnerPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;

use als_core::character::AlsCharacter;
use als_core::input::AlsInputCommand;
use als_core::mantle::AlsMantleState;
use als_core::motion::AlsViewYaw;
use als_core::roll::AlsRollState;
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::AlsStateRequest;
use als_core::view::{AlsViewPitch, AlsViewSmoothed};

use bevy_als::show_debug_hud::{
    AlsShowDebugHudPlugin, AlsShowDebugHudRoot, AlsShowDebugHudSet, AlsShowDebugHudState,
    DebugCategory,
};

fn main() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(8))),
        bevy::log::LogPlugin::default(),
        InputPlugin,
        TransformPlugin,
        AlsShowDebugHudPlugin,
    ))
    .init_resource::<GateFlags>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            script_driver.before(AlsShowDebugHudSet::Input),
        ),
    )
    .add_systems(PostUpdate, verdict.after(AlsShowDebugHudSet::Sync));

    let secs: f64 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6.0);
    info!("[HUDGATE] auto quit armed in {secs:.1}s");
    app.insert_resource(GateDeadline(secs));
    app.run();
}

#[derive(Resource)]
struct GateDeadline(f64);

#[derive(Resource, Default)]
struct GateFlags {
    saw_panel_entity: bool,
    saw_debug_header: bool,
    saw_state_standing: bool,
    saw_state_crouching: bool,
    saw_shapes_rows: bool,
    saw_state_removed_by_retoggle: bool,
    failed_reasons: Vec<String>,
}

fn setup(mut commands: Commands) {
    commands
        .spawn((
            AlsCharacter,
            bevy_als::character_assembly::AlsPlayerCharacter,
            Transform::from_xyz(0.0, 0.9, 0.0),
            GlobalTransform::default(),
            LocomotionState::Grounded,
            RotationMode::ViewDirection,
            Stance::Standing,
            Gait::Walking,
            OverlayState::Default,
            AlsStateRequest::default(),
            AlsInputCommand {
                aim_held: true,
                move_dir: Vec2::new(0.0, 1.0),
                ..default()
            },
            LinearVelocity(Vec3::new(3.0, 0.0, -4.0)),
            AlsRollState::default(),
            AlsMantleState::default(),
            AlsViewYaw(0.35),
            AlsViewPitch(-0.10),
        ))
        .insert(AlsViewSmoothed(-0.10));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(2.4, 1.7, 4.6).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
    ));
}

fn press_with_shift(keys: &mut ButtonInput<KeyCode>, code: KeyCode) {
    keys.press(KeyCode::ShiftLeft);
    keys.release(code);
    keys.press(code);
}

fn script_driver(
    time: Res<Time>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut q_stance: Query<&mut Stance>,
    mut step: Local<usize>,
) {
    let t = time.elapsed_secs_f64();
    let target_step = if t >= 5.0 {
        6
    } else if t >= 4.3 {
        5
    } else if t >= 3.6 {
        4
    } else if t >= 2.6 {
        3
    } else if t >= 1.6 {
        2
    } else if t >= 0.6 {
        1
    } else {
        0
    };
    if target_step <= *step {
        return;
    }
    *step = target_step;
    match target_step {
        1 => {
            press_with_shift(&mut keys, KeyCode::Digit0);
            info!("[HUDGATE] step 1: Shift+0 enable");
        }
        2 => {
            press_with_shift(&mut keys, KeyCode::Digit2);
            info!("[HUDGATE] step 2: Shift+2 state category on");
        }
        3 => {
            for mut stance in q_stance.iter_mut() {
                *stance = Stance::Crouching;
            }
            info!("[HUDGATE] step 3: stance component -> Crouching");
        }
        4 => {
            press_with_shift(&mut keys, KeyCode::Digit3);
            info!("[HUDGATE] step 4: Shift+3 shapes category on");
        }
        5 => {
            press_with_shift(&mut keys, KeyCode::Digit2);
            info!("[HUDGATE] step 5: Shift+2 toggles state category off");
        }
        _ => {}
    }
}

fn collect_texts(texts: &Query<&Text>) -> String {
    let mut joined = String::new();
    for text in texts.iter() {
        joined.push_str(text.as_str());
        joined.push('\n');
    }
    joined
}

fn verdict(
    time: Res<Time>,
    deadline: Res<GateDeadline>,
    mut flags: ResMut<GateFlags>,
    texts: Query<&Text>,
    panels: Query<(Entity, &Visibility), With<AlsShowDebugHudRoot>>,
    hud_state: Res<AlsShowDebugHudState>,
    mut exit: MessageWriter<AppExit>,
    mut done: Local<bool>,
) {
    let t = time.elapsed_secs_f64();
    flags.saw_panel_entity |= !panels.is_empty();
    let content = collect_texts(&texts);
    let visible = panels.iter().any(|(_, v)| *v == Visibility::Visible);

    if (1.0..1.6).contains(&t) && visible {
        flags.saw_debug_header |= content.contains("Debug mode is enabled!");
    }
    if (2.2..2.6).contains(&t) && visible {
        flags.saw_state_standing |= hud_state.is_active(DebugCategory::State)
            && content.contains("Stance")
            && content.contains("Gait")
            && content.contains("Rotation Mode")
            && content.contains("Standing")
            && !content.contains("Crouching");
    }
    if (3.1..3.6).contains(&t) && visible {
        flags.saw_state_crouching |= content.contains("Crouching")
            && content.contains("Stance")
            && !content.contains("Standing");
    }
    if (3.9..4.3).contains(&t) && visible {
        flags.saw_shapes_rows |= content.contains("Input Yaw Angle")
            && content.contains("Velocity Yaw Angle")
            && content.contains("Speed");
    }
    if (4.7..deadline.0 - 0.35).contains(&t) && visible {
        flags.saw_state_removed_by_retoggle |=
            !content.contains("Stance") && content.contains("Input Yaw Angle");
    }

    let window_end = deadline.0 - 0.25;
    if t < window_end || *done {
        return;
    }
    *done = true;

    if !flags.saw_panel_entity {
        flags.failed_reasons.push("panel entity missing".into());
    }
    if !flags.saw_debug_header {
        flags
            .failed_reasons
            .push("debug-mode header not shown after Shift+0".into());
    }
    if !flags.saw_state_standing {
        flags
            .failed_reasons
            .push("state category missing Stance/Gait/Rotation Mode rows".into());
    }
    if !flags.saw_state_crouching {
        flags
            .failed_reasons
            .push("stance value did not flip to Crouching after component change".into());
    }
    if !flags.saw_shapes_rows {
        flags
            .failed_reasons
            .push("shapes category rows not rendered after Shift+3".into());
    }
    if !flags.saw_state_removed_by_retoggle {
        flags
            .failed_reasons
            .push("second Shift+2 did not remove the state category".into());
    }

    if flags.failed_reasons.is_empty() {
        info!(
            "[HUDGATE][VERDICT] panel=ok header=ok state_rows=ok stance_flip=ok shapes_rows=ok retoggle_off=ok -> PASS"
        );
        exit.write(AppExit::Success);
    } else {
        for reason in &flags.failed_reasons {
            error!("[HUDGATE][FAIL] {reason}");
        }
        error!(
            "[HUDGATE][VERDICT] FAIL ({} checks)",
            flags.failed_reasons.len()
        );
        exit.write(AppExit::Error(std::num::NonZeroU8::new(9).unwrap()));
    }
}
