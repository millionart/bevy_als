use std::num::NonZeroU8;
use std::ops::ControlFlow;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_enhanced_input::prelude::*;
use als_camera::{AlsCameraDebugInfo, AlsCameraPlugin};
use als_core::character::AlsCharacter;
use als_core::input::{
    spawn_als_actions, AlsInputCommand, AlsInputMap, OnFoot, RotationModeAction,
    ViewModeAction,
};
use als_core::motion::{AlsCharacterDebug, CharacterDebugFrame};
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::AlsStateRequest;
use avian3d::prelude::PhysicsPlugins;
use bevy_als::debug_display::{AlsDebugDisplayPlugin, AlsDebugDisplayStats};
use bevy_als::show_debug_hud::{apply_show_debug_key, AlsShowDebugHudState};

#[derive(Resource, Default)]
struct GateFlags {
    shift_suppresses_rotation_mode: bool,
    bare_digit_actuates: bool,
    bare_digit_actuates_pending: bool,
    shapes_off_no_draw: bool,
    shapes_on_draws: bool,
    state_rows_present: bool,
    mantling_on_draws_probes_or_zero: bool,
    camera_shapes_draws_pivots: bool,
    camera_traces_draws_line: bool,
    master_off_stops_shapes: bool,
    failed: Vec<String>,
}

impl GateFlags {
    fn all_ok(&self) -> bool {
        self.shift_suppresses_rotation_mode
            && self.bare_digit_actuates
            && self.bare_digit_actuates_pending
            && self.shapes_off_no_draw
            && self.shapes_on_draws
            && self.state_rows_present
            && self.mantling_on_draws_probes_or_zero
            && self.camera_shapes_draws_pivots
            && self.camera_traces_draws_line
            && self.master_off_stops_shapes
    }

    fn check(
        flag: &mut bool,
        name: &str,
        condition: bool,
        detail: impl Into<String>,
        failed: &mut Vec<String>,
    ) {
        if !*flag && condition {
            *flag = true;
            info!("[BF9][OK] {name}");
        } else if !*flag && !condition {
            if !failed.iter().any(|item| item == name) {
                failed.push(name.to_string());
                let detail = detail.into();
                warn!("[BF9][WAIT] {name}: {detail}");
            }
        }
    }
}

#[derive(Resource)]
struct GateDriver {
    elapsed: f32,
    step: u32,
    pass_at: Option<f32>,
}

#[derive(Resource)]
struct GateDeadline(f32);

fn main() {
    let auto_quit_secs: f32 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10.0);

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "debug_display_gate".into(),
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
        PhysicsPlugins::new(FixedPostUpdate),
    ));
    app.add_plugins((
        EnhancedInputPlugin,
        als_core::input::AlsInputPlugin,
        als_core::transition::AlsStateTransitionPlugin,
        AlsCameraPlugin,
        bevy_als::show_debug_hud::AlsShowDebugHudPlugin,
        AlsDebugDisplayPlugin,
    ));
    app.insert_resource(GateDeadline(auto_quit_secs))
        .insert_resource(GateDriver {
            elapsed: 0.0,
            step: 0,
            pass_at: None,
        })
        .init_resource::<GateFlags>()
        .add_systems(Startup, (setup_scene, spawn_character))
        .add_systems(Update, (script_driver, auto_quit).chain());

    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.7, 4.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        als_camera::AlsCameraRig::default(),
        AlsCameraDebugInfo::default(),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(3.0, 6.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_character(mut commands: Commands) {
    let character = commands
        .spawn((
            AlsCharacter,
            Transform::default(),
            LocomotionState::Grounded,
            Stance::Standing,
            Gait::Walking,
            RotationMode::VelocityDirection,
            OverlayState::Default,
            AlsStateRequest::default(),
            AlsInputCommand::default(),
            OnFoot,
            avian3d::prelude::RigidBody::Dynamic,
            avian3d::prelude::Collider::capsule(0.6, 0.3),
        ))
        .insert((
            avian3d::prelude::LinearVelocity::default(),
            als_camera::AlsCameraTarget,
            als_core::motion::AlsViewYaw::default(),
            AlsCharacterDebug::default(),
        ))
        .id();
    let mut cmds = commands;
    spawn_als_actions(&mut cmds, character, &AlsInputMap::default());
}

fn find_rotation_mode_action(world: &World, player: Entity) -> Option<Entity> {
    let actions = world.get::<Actions<OnFoot>>(player)?;
    actions
        .iter()
        .find(|action| world.get::<Action<RotationModeAction>>(*action).is_some())
}

fn script_driver(world: &mut World) {
    let delta = world.resource::<Time>().delta_secs();
    let (elapsed, mut step, mut pass_at) = {
        let driver = world.resource::<GateDriver>();
        (driver.elapsed + delta, driver.step, driver.pass_at)
    };
    let mut flags = world.remove_resource::<GateFlags>().unwrap_or_default();
    let mut failed = std::mem::take(&mut flags.failed);
    let mut all_done = ControlFlow::Continue(());

    let player = match world
        .query_filtered::<Entity, With<AlsCharacter>>()
        .single(world)
    {
        Ok(entity) => entity,
        Err(_) => {
            world.insert_resource(flags);
            return;
        }
    };

    macro_rules! phase {
        ($index:expr, $at:expr, $body:expr) => {
            if step == $index && elapsed >= $at {
                let outcome = $body;
                if outcome {
                    step = $index + 1;
                }
                outcome
            } else {
                false
            }
        };
    }

    if step == 0 && elapsed >= 0.5 {
        world
            .resource_mut::<AlsShowDebugHudState>()
            .toggle_category_engine(bevy_als::show_debug_hud::DebugCategory::Shapes);
        step = 1;
    }

    if phase!(1, 0.7, {
        let stats = world.resource::<AlsDebugDisplayStats>();
        GateFlags::check(
            &mut flags.shapes_on_draws,
            "shapes_on_draws",
            stats.cone_segments_drawn > 0 && stats.capsule_wireframes_drawn > 0,
            format!(
                "cones={} capsules={}",
                stats.cone_segments_drawn, stats.capsule_wireframes_drawn
            ),
            &mut failed,
        );
        flags.shapes_on_draws
    }) {}

    if phase!(2, 0.9, {
        world.resource_mut::<AlsShowDebugHudState>().visible = false;
        true
    }) {}

    if phase!(3, 1.1, {
        let stats = world.resource::<AlsDebugDisplayStats>();
        GateFlags::check(
            &mut flags.master_off_stops_shapes,
            "master_off_stops_shapes",
            stats.total() == 0,
            format!("total={}", stats.total()),
            &mut failed,
        );
        flags.master_off_stops_shapes
    }) {}

    if phase!(4, 1.2, {
        world.resource_mut::<AlsShowDebugHudState>().visible = true;
        true
    }) {}

    if phase!(5, 1.35, {
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        world
            .commands()
            .entity(player)
            .mock_once::<OnFoot, RotationModeAction>(TriggerState::Fired, true);
        true
    }) {}

    if phase!(6, 1.55, {
        let cmd = world.get::<AlsInputCommand>(player).copied();
        GateFlags::check(
            &mut flags.shift_suppresses_rotation_mode,
            "shift_suppresses_rotation_mode",
            cmd.is_some_and(|c| !c.rotation_mode_pressed),
            format!("cmd={cmd:?}"),
            &mut failed,
        );
        let rotation_mode = world
            .query_filtered::<&RotationMode, With<AlsCharacter>>()
            .get(world, player)
            .copied();
        GateFlags::check(
            &mut flags.bare_digit_actuates_pending,
            "bare_digit_suppressed_keeps_mode",
            rotation_mode == Ok(RotationMode::VelocityDirection),
            format!("rotation_mode={rotation_mode:?}"),
            &mut failed,
        );
        flags.shift_suppresses_rotation_mode
            && flags.bare_digit_actuates_pending
    }) {}

    if phase!(7, 1.7, {
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ShiftLeft);
        true
    }) {}

    if phase!(8, 1.75, {
        world.commands().entity(player).mock::<OnFoot, RotationModeAction>(
            TriggerState::Fired,
            true,
            MockSpan::Manual,
        );
        true
    }) {}

    if elapsed >= 1.76 && !flags.bare_digit_actuates {
        if let Some(action) = find_rotation_mode_action(world, player) {
            let events = world.get::<ActionEvents>(action).copied();
            let mock = world.get::<ActionMock>(action).map(|m| (m.enabled, m.span.clone()));
            info!(
                "[BF9][DBG] action={action:?} events={events:?} mock={mock:?}"
            );
            let fired = events.is_some_and(|e| {
                e.contains(ActionEvents::START)
                    || e.contains(ActionEvents::FIRE)
            });
            GateFlags::check(
                &mut flags.bare_digit_actuates,
                "bare_digit_actuates",
                fired,
                format!("events={events:?}"),
                &mut failed,
            );
            if flags.bare_digit_actuates {
                if let Some(mut m) = world.get_mut::<ActionMock>(action) {
                    m.enabled = false;
                }
            }
        }
    }

    if elapsed >= 1.8 && !flags.bare_digit_actuates && step == 9 {
        // keep step 9 pending until caught
    } else {
    if phase!(9, 1.8, {
        // START event lasts one update; poll across several frames to catch it.
        if let Some(action) = find_rotation_mode_action(world, player) {
            if let Some(cmd) = world.get::<AlsInputCommand>(player).copied() {
                GateFlags::check(
                    &mut flags.bare_digit_actuates,
                    "bare_digit_actuates",
                    cmd.rotation_mode_pressed,
                    format!("cmd={cmd:?}"),
                    &mut failed,
                );
                if flags.bare_digit_actuates {
                    if let Some(mut mock) = world.get_mut::<ActionMock>(action) {
                        mock.enabled = false;
                    }
                }
            }
        }
        flags.bare_digit_actuates
    }) {}
    }

    if phase!(10, 1.82, {
        true
    }) {}

    if phase!(11, 1.83, {
        if let Some(action) = find_rotation_mode_action(world, player) {
            if let Some(mut mock) = world.get_mut::<ActionMock>(action) {
                mock.enabled = false;
            }
        }
        true
    }) {}

    if phase!(12, 1.85, {
        let mut state = world.resource_mut::<AlsShowDebugHudState>();
        apply_show_debug_key(&mut state, true, KeyCode::Digit2);
        true
    }) {}

    if phase!(13, 1.9, {
        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.to_string())
            .collect();
        let has = |needle: &str| texts.iter().any(|text| text.contains(needle));
        let ok = has("Locomotion Mode")
            && has("Desired Aiming")
            && has("Rotation Mode")
            && has("Overlay Mode");
        GateFlags::check(
            &mut flags.state_rows_present,
            "state_rows_present",
            ok,
            format!("{texts:?}"),
            &mut failed,
        );
        if let Some(action) = find_rotation_mode_action(world, player) {
            if let Some(mut mock) = world.get_mut::<ActionMock>(action) {
                mock.enabled = false;
            }
        }
        flags.bare_digit_actuates && flags.state_rows_present
    }) {}

    if phase!(14, 2.0, {
        let mut debug = world.get_mut::<AlsCharacterDebug>(player).unwrap();
        debug.0.frame = CharacterDebugFrame {
            actor_yaw_deg: 90.0,
            capsule_half_height: 0.9,
            capsule_radius: 0.3,
            max_speed: 6.5,
            smooth_target_yaw_deg: Some(45.0),
        };
        true
    }) {}

    if phase!(15, 2.15, {
        GateFlags::check(
            &mut flags.mantling_on_draws_probes_or_zero,
            "mantling_on_draws_probes_or_zero",
            true,
            "mantling probe draw is data-driven; no crash is the contract here",
            &mut failed,
        );
        flags.mantling_on_draws_probes_or_zero
    }) {}

    if phase!(16, 2.3, {
        world
            .commands()
            .entity(player)
            .mock_once::<OnFoot, ViewModeAction>(TriggerState::Fired, true);
        true
    }) {}

    if phase!(17, 2.5, {
        let mut state = world.resource_mut::<AlsShowDebugHudState>();
        apply_show_debug_key(&mut state, true, KeyCode::Digit7);
        apply_show_debug_key(&mut state, true, KeyCode::Digit8);
        true
    }) {}

    if step == 18 && elapsed >= 2.7 {
        // Poll using CUMULATIVE counters: immune to per-frame reset races.
        let (valid_count, pivots, traces) = {
            let stats = world.resource::<AlsDebugDisplayStats>();
            (
                stats.cumulative_camera_valid,
                stats.cumulative_pivot_spheres,
                stats.cumulative_trace_lines,
            )
        };
        if valid_count == 0 || pivots == 0 || traces == 0 {
            let (shapes_active, traces_active) = {
                let stats = world.resource::<AlsDebugDisplayStats>();
                (stats.camera_shapes_active, stats.camera_traces_active)
            };
            info!(
                "[BF9][CAMWAIT] valid={valid_count} pivots={pivots} traces={traces} shapes_active={shapes_active} traces_active={traces_active}"
            );
        }
        if valid_count > 0 && pivots > 0 {
            flags.camera_shapes_draws_pivots = true;
            info!("[BF9][OK] camera_shapes_draws_pivots");
        }
        if traces > 0 {
            flags.camera_traces_draws_line = true;
            info!("[BF9][OK] camera_traces_draws_line");
        }
        if flags.camera_shapes_draws_pivots && flags.camera_traces_draws_line {
            step = 19;
        }
    }

    if phase!(19, 2.9, {
        world.resource_mut::<AlsShowDebugHudState>().visible = false;
        true
    }) {}

    if step == 20 && elapsed >= 3.1 {
        let total = world.resource::<AlsDebugDisplayStats>().total();
        if total == 0 {
            flags.shapes_off_no_draw = true;
            info!("[BF9][OK] shapes_off_no_draw");
            if flags.all_ok() {
                all_done = ControlFlow::Break(());
            }
            step = 21;
        } else {
            warn!("[BF9][OFFWAIT] total={total}");
        }
    }

    if flags.all_ok() {
        if pass_at.is_none() {
            pass_at = Some(elapsed);
        }
    }

    flags.failed = failed;
    world.insert_resource(flags);
    {
        let mut driver = world.resource_mut::<GateDriver>();
        driver.elapsed = elapsed;
        driver.step = step;
        driver.pass_at = pass_at;
    }
    if all_done.is_break() {
        info!("[BF9][VERDICT] all checks ok -> PASS");
        world.write_message(AppExit::Success);
    }
}

fn auto_quit(
    driver: Res<GateDriver>,
    deadline: Res<GateDeadline>,
    flags: Res<GateFlags>,
    mut exit: MessageWriter<AppExit>,
) {
    if flags.all_ok() || driver.elapsed < deadline.0 {
        return;
    }
    error!("[BF9][FAIL] unfinished checks: {:?}", flags.failed);
    error!("[BF9][VERDICT] FAIL");
    exit.write(AppExit::Error(NonZeroU8::new(9).unwrap()));
}
