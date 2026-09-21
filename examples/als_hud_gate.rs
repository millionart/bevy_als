use std::num::NonZeroU8;
use std::ops::ControlFlow;

use als_core::character::AlsCharacter;
use als_core::input::{
    AlsInputCommand, AlsInputMap, NextOverlayModeAction, OnFoot, OverlayMenuAction,
    PreviousOverlayModeAction, SlomoAction, ToggleUiAction, spawn_als_actions,
};
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::AlsStateRequest;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::ui::Display;
use bevy::window::WindowResolution;
use bevy_als::als_hud::{
    AlsHudColorTarget, AlsHudDebugSection, AlsHudInputSection, AlsHudPlugin, COLOR_DISABLED,
    COLOR_ENABLED,
};
use bevy_als::overlay_mode_menu::{
    MENU_DILATION, OverlayMenuOption, OverlayMenuState,
    OverlayModeMenuPlugin, menu_display_order, option_label,
};
use bevy_als::show_debug_hud::{AlsShowDebugHudState, apply_show_debug_key};
use bevy_enhanced_input::prelude::*;

#[derive(Resource, Default)]
struct GateFlags {
    hud_rows: bool,
    ui_toggle_off: bool,
    ui_toggle_on: bool,
    slomo_on: bool,
    slomo_off: bool,
    debug_curves_green: bool,
    debug_master_off_gray: bool,
    menu_open: bool,
    menu_order: bool,
    menu_highlight_init: bool,
    menu_dilation_open: bool,
    menu_cycle_next: bool,
    menu_cycle_prev_back: bool,
    menu_cycle_prev_to_box: bool,
    approve_request: bool,
    approve_transition: bool,
    menu_despawned: bool,
    dilation_restored: bool,
    interplay_slomo_on: bool,
    interplay_open_keeps: bool,
    interplay_close_keeps: bool,
    interplay_off_restores: bool,
    failed: Vec<String>,
}

impl GateFlags {
    fn all_ok(&self) -> bool {
        self.hud_rows
            && self.ui_toggle_off
            && self.ui_toggle_on
            && self.slomo_on
            && self.slomo_off
            && self.debug_curves_green
            && self.debug_master_off_gray
            && self.menu_open
            && self.menu_order
            && self.menu_highlight_init
            && self.menu_dilation_open
            && self.menu_cycle_next
            && self.menu_cycle_prev_back
            && self.menu_cycle_prev_to_box
            && self.approve_request
            && self.approve_transition
            && self.menu_despawned
            && self.dilation_restored
            && self.interplay_slomo_on
            && self.interplay_open_keeps
            && self.interplay_close_keeps
            && self.interplay_off_restores
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
            info!("[BF6B][OK] {name}");
        } else if !*flag && !condition {
            if !failed.iter().any(|item| item == name) {
                failed.push(name.to_string());
                let detail = detail.into();
                warn!("[BF6B][WAIT] {name}: {detail}");
            }
        }
    }
}

#[derive(Resource)]
struct GateDriver {
    elapsed: f32,
    step: u32,
    menu_mock_entity: Option<Entity>,
    shot_menu_done: bool,
    shot_hud_done: bool,
    pass_at: Option<f32>,
}

#[derive(Resource)]
struct GateDeadline(f32);

#[derive(Resource, Default)]
struct GateShotPath(pub Option<String>);

#[derive(Component)]
struct GateCamera;

fn main() {
    let auto_quit_secs: f32 = std::env::var("ALS_BOOT_AUTO_QUIT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8.0);
    let shot_path = std::env::var("ALS_SHOT_PATH").ok();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: "assets".into(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "als_hud_gate".into(),
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
    );
    app.add_plugins((
        bevy_gameplay_tag::GameplayTagsPlugin::with_data_path("assets/als_gameplay_tags.json"),
        EnhancedInputPlugin,
        als_core::input::AlsInputPlugin,
        als_core::transition::AlsStateTransitionPlugin,
        AlsHudPlugin,
        OverlayModeMenuPlugin,
        bevy_als::show_debug_hud::AlsShowDebugHudPlugin,
    ));
    app.insert_resource(GateDeadline(auto_quit_secs))
        .insert_resource(GateShotPath(shot_path))
        .insert_resource(GateDriver {
            elapsed: 0.0,
            step: 0,
            menu_mock_entity: None,
            shot_menu_done: false,
            shot_hud_done: false,
            pass_at: None,
        })
        .init_resource::<GateFlags>()
        .add_systems(Startup, (setup_scene, spawn_character))
        .add_systems(Update, (script_driver, auto_quit).chain());

    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        GateCamera,
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.7, 4.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
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
            LocomotionState::Grounded,
            Stance::Standing,
            Gait::Walking,
            RotationMode::VelocityDirection,
            OverlayState::Default,
            AlsStateRequest::default(),
            AlsInputCommand::default(),
            OnFoot,
        ))
        .id();
    let mut cmds = commands;
    spawn_als_actions(&mut cmds, character, &AlsInputMap::default());
}

fn find_menu_action(world: &World, player: Entity) -> Option<Entity> {
    let actions = world.get::<Actions<OnFoot>>(player)?;
    actions
        .iter()
        .find(|action| world.get::<Action<OverlayMenuAction>>(*action).is_some())
}

fn capture(world: &mut World, suffix: &str) {
    let path = world.resource::<GateShotPath>().0.clone();
    let Some(base) = path else {
        return;
    };
    world
        .commands()
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("{base}-{suffix}.png")));
    info!("[BF6B][SHOT] {base}-{suffix}.png scheduled");
}

fn script_driver(world: &mut World) {
    let delta = world.resource::<Time>().delta_secs();
    let (
        elapsed,
        mut step,
        mut menu_mock_entity,
        mut shot_menu_done,
        mut shot_hud_done,
        mut pass_at,
    ) = {
        let driver = world.resource::<GateDriver>();
        (
            driver.elapsed + delta,
            driver.step,
            driver.menu_mock_entity,
            driver.shot_menu_done,
            driver.shot_hud_done,
            driver.pass_at,
        )
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

    {
        if step == 0 && elapsed >= 0.4 {
            let texts: Vec<String> = world
                .query::<&Text>()
                .iter(world)
                .map(|text| text.to_string())
                .collect();
            let has = |needle: &str| texts.iter().any(|text| text.contains(needle));
            let ok = has("Advanced Locomotion System")
                && has("Refactored")
                && has("Toggle UI")
                && has("Toggle Slomo")
                && has("Toggle Camera Traces")
                && has("Toggle Debug")
                && has("Cycle Overlay")
                && has("D-pad U / D")
                && has("Shift + 8")
                && has("Overlay Menu");
            GateFlags::check(
                &mut flags.hud_rows,
                "hud_rows",
                ok,
                format!("{texts:?}"),
                &mut failed,
            );
            if ok {
                step = 1;
            }
        }

        if phase!(1, 0.6, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, ToggleUiAction>(TriggerState::Fired, true);
            true
        }) {}

        if phase!(2, 0.75, {
            let hidden = world
                .query_filtered::<&Node, With<AlsHudDebugSection>>()
                .iter(world)
                .all(|node| node.display == Display::None)
                && world
                    .query_filtered::<&Node, With<AlsHudInputSection>>()
                    .iter(world)
                    .all(|node| node.display == Display::None);
            let gray = world
                .query::<(&AlsHudColorTarget, &TextColor)>()
                .iter(world)
                .filter(|(target, _)| **target == AlsHudColorTarget::UiToggle)
                .all(|(_, color)| color.0 == COLOR_DISABLED);
            GateFlags::check(
                &mut flags.ui_toggle_off,
                "ui_toggle_off",
                hidden && gray,
                format!("hidden={hidden} gray={gray}"),
                &mut failed,
            );
            flags.ui_toggle_off
        }) {}

        if phase!(3, 0.9, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, ToggleUiAction>(TriggerState::Fired, true);
            true
        }) {}

        if phase!(4, 1.05, {
            let shown = world
                .query_filtered::<&Node, With<AlsHudDebugSection>>()
                .iter(world)
                .all(|node| node.display == Display::Flex)
                && world
                    .query_filtered::<&Node, With<AlsHudInputSection>>()
                    .iter(world)
                    .all(|node| node.display == Display::Flex);
            let green = world
                .query::<(&AlsHudColorTarget, &TextColor)>()
                .iter(world)
                .filter(|(target, _)| **target == AlsHudColorTarget::UiToggle)
                .all(|(_, color)| color.0 == COLOR_ENABLED);
            GateFlags::check(
                &mut flags.ui_toggle_on,
                "ui_toggle_on",
                shown && green,
                format!("shown={shown} green={green}"),
                &mut failed,
            );
            flags.ui_toggle_on
        }) {}

        if phase!(5, 1.2, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, SlomoAction>(TriggerState::Fired, true);
            true
        }) {}

        if phase!(6, 1.35, {
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            GateFlags::check(
                &mut flags.slomo_on,
                "slomo_on",
                speed == bevy_als::als_hud::SLMO_TIME_SCALE,
                format!("relative_speed={speed}"),
                &mut failed,
            );
            flags.slomo_on
        }) {}

        if phase!(7, 1.5, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, SlomoAction>(TriggerState::Fired, true);
            true
        }) {}

        if phase!(8, 1.65, {
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            GateFlags::check(
                &mut flags.slomo_off,
                "slomo_off",
                speed == 1.0,
                format!("relative_speed={speed}"),
                &mut failed,
            );
            flags.slomo_off
        }) {}

        if phase!(9, 1.8, {
            let mut state = world.resource_mut::<AlsShowDebugHudState>();
            apply_show_debug_key(&mut state, true, KeyCode::Digit1);
            true
        }) {}

        if phase!(10, 1.95, {
            let curves_green = world
                .query::<(&AlsHudColorTarget, &TextColor)>()
                .iter(world)
                .filter(|(target, _)| {
                    matches!(
                        target,
                        AlsHudColorTarget::Category(
                            bevy_als::show_debug_hud::DebugCategory::Curves
                        )
                    )
                })
                .all(|(_, color)| color.0 == COLOR_ENABLED);
            GateFlags::check(
                &mut flags.debug_curves_green,
                "debug_curves_green",
                curves_green,
                "Shift+1 should turn the curves label green",
                &mut failed,
            );
            flags.debug_curves_green
        }) {}

        if phase!(11, 2.0, {
            let mut state = world.resource_mut::<AlsShowDebugHudState>();
            apply_show_debug_key(&mut state, true, KeyCode::Digit0);
            true
        }) {}

        if phase!(12, 2.15, {
            let all_gray = world
                .query::<(&AlsHudColorTarget, &TextColor)>()
                .iter(world)
                .filter(|(target, _)| {
                    matches!(
                        target,
                        AlsHudColorTarget::Category(_)
                    )
                })
                .all(|(_, color)| color.0 == COLOR_DISABLED);
            GateFlags::check(
                &mut flags.debug_master_off_gray,
                "debug_master_off_gray",
                all_gray,
                "Shift+0 master off should gray every category label",
                &mut failed,
            );
            flags.debug_master_off_gray
        }) {}

        if phase!(13, 2.3, {
            world
                .commands()
                .entity(player)
                .mock::<OnFoot, OverlayMenuAction>(TriggerState::Fired, true, MockSpan::Manual);
            menu_mock_entity = find_menu_action(world, player);
            true
        }) {}

        if phase!(14, 2.5, {
            let (menu_is_open, menu_rooted) = {
                let state = world.resource::<OverlayMenuState>();
                (state.open, state.root.is_some())
            };
            let option_count = world.query::<&OverlayMenuOption>().iter(world).count();
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            let open_ok =
                menu_is_open && menu_rooted && option_count == menu_display_order(world.resource::<bevy_gameplay_tag::GameplayTagsManager>()).len();
            GateFlags::check(
                &mut flags.menu_open,
                "menu_open",
                open_ok,
                format!("open={} options={option_count}", menu_is_open),
                &mut failed,
            );
            let mut labels: Vec<(usize, String)> = world
                .query::<(&OverlayMenuOption, &Children)>()
                .iter(world)
                .map(|(option, children)| {
                    let text = children
                        .iter()
                        .filter_map(|child| world.get::<Text>(child))
                        .map(|text| text.to_string())
                        .collect::<String>();
                    (option.index, text)
                })
                .collect();
            labels.sort_by_key(|(index, _)| *index);
            let expected: Vec<String> = menu_display_order(world.resource::<bevy_gameplay_tag::GameplayTagsManager>())
                .iter()
                .map(|state| option_label(*state))
                .collect();
            let actual: Vec<String> = labels.into_iter().map(|(_, text)| text).collect();
            GateFlags::check(
                &mut flags.menu_order,
                "menu_order",
                actual == expected,
                format!("actual={actual:?} expected={expected:?}"),
                &mut failed,
            );
            let highlight_ok = world
                .query::<(&OverlayMenuOption, &BackgroundColor)>()
                .iter(world)
                .all(|(option, background)| {
                    let want = if option.index == 8 {
                        bevy_als::overlay_mode_menu::COLOR_OPTION_SELECTED
                    } else {
                        bevy_als::overlay_mode_menu::COLOR_OPTION_NOT_SELECTED
                    };
                    background.0 == want
                });
            GateFlags::check(
                &mut flags.menu_highlight_init,
                "menu_highlight_init",
                highlight_ok,
                "Default (index 8) should be highlighted on open",
                &mut failed,
            );
            GateFlags::check(
                &mut flags.menu_dilation_open,
                "menu_dilation_open",
                speed == MENU_DILATION,
                format!("relative_speed={speed}"),
                &mut failed,
            );
            open_ok && flags.menu_order && flags.menu_highlight_init && flags.menu_dilation_open
        }) {}

        if phase!(15, 2.7, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, NextOverlayModeAction>(TriggerState::Fired, 1.0);
            true
        }) {}

        if phase!(16, 2.85, {
            let index = world.resource::<OverlayMenuState>().selected_index();
            GateFlags::check(
                &mut flags.menu_cycle_next,
                "menu_cycle_next",
                index == 7,
                format!("selected_index={index} expect 7"),
                &mut failed,
            );
            flags.menu_cycle_next
        }) {}

        if phase!(17, 3.0, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, PreviousOverlayModeAction>(TriggerState::Fired, 1.0);
            true
        }) {}

        if phase!(18, 3.15, {
            let index = world.resource::<OverlayMenuState>().selected_index();
            GateFlags::check(
                &mut flags.menu_cycle_prev_back,
                "menu_cycle_prev_back",
                index == 8,
                format!("selected_index={index} expect 8"),
                &mut failed,
            );
            flags.menu_cycle_prev_back
        }) {}

        if phase!(19, 3.3, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, PreviousOverlayModeAction>(TriggerState::Fired, 1.0);
            true
        }) {}

        if phase!(20, 3.45, {
            let index = world.resource::<OverlayMenuState>().selected_index();
            GateFlags::check(
                &mut flags.menu_cycle_prev_to_box,
                "menu_cycle_prev_to_box",
                index == 9,
                format!("selected_index={index} expect 9"),
                &mut failed,
            );
            if flags.menu_cycle_prev_to_box && !shot_menu_done {
                capture(world, "menu");
                shot_menu_done = true;
            }
            flags.menu_cycle_prev_to_box
        }) {}

        if phase!(21, 3.6, {
            if let Some(action) = menu_mock_entity {
                if let Some(mut mock) = world.get_mut::<ActionMock>(action) {
                    mock.enabled = false;
                }
            }
            true
        }) {}

        if phase!(22, 3.9, {
            let overlay = world
                .query_filtered::<&OverlayState, With<AlsCharacter>>()
                .single(world)
                .copied()
                .ok();
            let request = world
                .query_filtered::<&AlsStateRequest, With<AlsCharacter>>()
                .single(world)
                .map(|request| request.overlay_state)
                .ok()
                .flatten();
            GateFlags::check(
                &mut flags.approve_request,
                "approve_request",
                request == Some(OverlayState::Box) || overlay == Some(OverlayState::Box),
                format!("request={request:?} overlay={overlay:?}"),
                &mut failed,
            );
            GateFlags::check(
                &mut flags.approve_transition,
                "approve_transition",
                overlay == Some(OverlayState::Box),
                format!("overlay={overlay:?}"),
                &mut failed,
            );
            let menu_still_open = world.resource::<OverlayMenuState>().open;
            let option_count = world.query::<&OverlayMenuOption>().iter(world).count();
            let menu_gone = !menu_still_open && option_count == 0;
            GateFlags::check(
                &mut flags.menu_despawned,
                "menu_despawned",
                menu_gone,
                "menu should close on release",
                &mut failed,
            );
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            GateFlags::check(
                &mut flags.dilation_restored,
                "dilation_restored",
                speed == 1.0,
                format!("relative_speed={speed}"),
                &mut failed,
            );
            flags.approve_request
                && flags.approve_transition
                && flags.menu_despawned
                && flags.dilation_restored
        }) {}

        if phase!(23, 4.1, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, SlomoAction>(TriggerState::Fired, true);
            true
        }) {}

        if phase!(24, 4.25, {
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            GateFlags::check(
                &mut flags.interplay_slomo_on,
                "interplay_slomo_on",
                speed == bevy_als::als_hud::SLMO_TIME_SCALE,
                format!("relative_speed={speed}"),
                &mut failed,
            );
            flags.interplay_slomo_on
        }) {}

        if phase!(25, 4.3, {
            world
                .commands()
                .entity(player)
                .mock::<OnFoot, OverlayMenuAction>(TriggerState::Fired, true, MockSpan::Manual);
            true
        }) {}

        if phase!(26, 4.6, {
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            GateFlags::check(
                &mut flags.interplay_open_keeps,
                "interplay_open_keeps_slomo",
                speed == bevy_als::als_hud::SLMO_TIME_SCALE,
                format!("relative_speed={speed}"),
                &mut failed,
            );
            flags.interplay_open_keeps
        }) {}

        if phase!(27, 4.7, {
            if let Some(action) = menu_mock_entity {
                if let Some(mut mock) = world.get_mut::<ActionMock>(action) {
                    mock.enabled = false;
                }
            }
            true
        }) {}

        if phase!(28, 4.85, {
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            GateFlags::check(
                &mut flags.interplay_close_keeps,
                "interplay_close_keeps_slomo",
                speed == bevy_als::als_hud::SLMO_TIME_SCALE,
                format!("relative_speed={speed}"),
                &mut failed,
            );
            flags.interplay_close_keeps
        }) {}

        if phase!(29, 4.9, {
            world
                .commands()
                .entity(player)
                .mock_once::<OnFoot, SlomoAction>(TriggerState::Fired, true);
            true
        }) {}

        if phase!(30, 5.05, {
            let speed = world.resource::<Time<Virtual>>().relative_speed();
            GateFlags::check(
                &mut flags.interplay_off_restores,
                "interplay_off_restores",
                speed == 1.0,
                format!("relative_speed={speed}"),
                &mut failed,
            );
            if flags.interplay_off_restores && !shot_hud_done {
                capture(world, "hud");
                shot_hud_done = true;
            }
            flags.interplay_off_restores
        }) {}

        if flags.all_ok() {
            if pass_at.is_none() {
                pass_at = Some(elapsed);
            }
            if elapsed >= pass_at.unwrap() + 1.0 {
                all_done = ControlFlow::Break(());
            }
        }
    }

    flags.failed = failed;
    world.insert_resource(flags);
    {
        let mut driver = world.resource_mut::<GateDriver>();
        driver.elapsed = elapsed;
        driver.step = step;
        driver.menu_mock_entity = menu_mock_entity;
        driver.shot_menu_done = shot_menu_done;
        driver.shot_hud_done = shot_hud_done;
        driver.pass_at = pass_at;
    }
    if all_done.is_break() {
        info!("[BF6B][VERDICT] all checks ok -> PASS");
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
    error!("[BF6B][FAIL] unfinished checks: {:?}", flags.failed);
    error!("[BF6B][VERDICT] FAIL");
    exit.write(AppExit::Error(NonZeroU8::new(9).unwrap()));
}
