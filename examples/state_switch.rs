use als_core::character::{AlsCharacterBundle, AlsCharacterPlugin};
use als_core::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use als_core::debug_hud::AlsStateHudPlugin;
use als_core::input::{AlsInputMap, AlsInputPlugin, spawn_als_actions};
use als_core::params::AlsMovementParams;
use als_core::state::{Gait, LocomotionState, RotationMode, Stance};
use als_core::transition::{
    AlsStateRequest, AlsStateTransitionPlugin, StateTransitionEvent, StateTransitionRejectedEvent,
};
use avian3d::prelude::*;
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::EnhancedInputPlugin;
use bevy_tnua::TnuaControllerPlugin;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            file_path: "assets".into(),
            ..default()
        }),
        PhysicsPlugins::new(FixedPostUpdate),
        TnuaAvian3dPlugin::new(FixedUpdate),
        TnuaControllerPlugin::<AlsControlScheme>::new(Update),
        EnhancedInputPlugin,
        AlsCharacterPlugin,
        AlsInputPlugin,
        AlsStateTransitionPlugin,
        AlsStateHudPlugin,
    ))
    .add_systems(Startup, setup)
    .add_systems(Update, (keyboard_driver, transition_logger));

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        app.add_systems(Update, auto_demo);
    }

    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params_assets: ResMut<Assets<AlsMovementParams>>,
    mut config_assets: ResMut<Assets<AlsControlSchemeConfig>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 0.1, 20.0),
        Transform::from_xyz(0.0, -0.05, 0.0),
        Mesh3d(meshes.add(Mesh::from(Rectangle::new(40.0, 40.0)))),
        MeshMaterial3d(materials.add(Color::srgb(0.25, 0.28, 0.25))),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.2, 1.8, 4.6).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(3.0, 5.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let params = params_assets.add(AlsMovementParams::default());
    let config = config_assets.add(AlsControlSchemeConfig::default());
    let mut bundle = AlsCharacterBundle::new(params, config);
    bundle.transform = Transform::from_xyz(0.0, 1.0, 0.0);
    let character = commands.spawn(bundle).id();
    spawn_als_actions(&mut commands, character, &AlsInputMap::default());

    if std::env::var("ALS_BOOT_AUTO_QUIT_SECS").is_ok() {
        commands.entity(character).insert(ScriptedDemo { step: 0 });
        info!("[DEMO] scripted state-switch sequence armed");
    }
}

const STEP_SECONDS: f64 = 1.2;

#[derive(Component)]
struct ScriptedDemo {
    step: usize,
}

fn auto_demo(
    time: Res<Time>,
    mut demo: Query<(&mut ScriptedDemo, &mut AlsStateRequest)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((mut demo, mut request)) = demo.single_mut() else {
        return;
    };
    let elapsed = time.elapsed_secs_f64();
    let target_step = (elapsed / STEP_SECONDS) as usize + 1;
    while demo.step < target_step {
        let step = demo.step;
        demo.step += 1;
        match step {
            0 => {}
            1 => info!("[DEMO] step 1: initial states printed by HUD"),
            2 => {
                info!("[DEMO] step 2: request Crouch (grounded -> expect accept)");
                request.stance = Some(Stance::Crouching);
            }
            3 => {
                info!("[DEMO] step 3: request Standing (expect accept)");
                request.stance = Some(Stance::Standing);
            }
            4 => {
                info!("[DEMO] step 4: request Air (simulate jump, expect accept)");
                request.locomotion_state = Some(LocomotionState::Air);
            }
            5 => {
                info!("[DEMO] step 5: request Crouch WHILE AIRBORNE (illegal -> expect queue)");
                request.stance = Some(Stance::Crouching);
            }
            6 => {
                info!("[DEMO] step 6: request Grounded (expect accept + queued crouch applies)");
                request.locomotion_state = Some(LocomotionState::Grounded);
            }
            7 => {
                info!("[DEMO] step 7: request Gait=Sprinting (expect accept)");
                request.gait = Some(Gait::Sprinting);
            }
            8 => {
                info!("[DEMO] step 8: request Ragdolling (expect accept)");
                request.ragdoll_start = true;
            }
            9 => {
                info!("[DEMO] step 9: request Air WHILE RAGDOLLED (never legal -> expect reject)");
                request.locomotion_state = Some(LocomotionState::Air);
            }
            10 => {
                info!("[DEMO] step 10: request ragdoll recovery (requires physics route)");
                request.ragdoll_exit = true;
            }
            _ => {
                info!("[DEMO] final states dumped by HUD above; done");
                exit.write(AppExit::Success);
            }
        }
    }
}

fn keyboard_driver(keys: Res<ButtonInput<KeyCode>>, mut q_request: Query<(&mut AlsStateRequest, Option<&als_core::state::AlsNotifyLocomotionAction>)>) {
    let Ok((mut request, action)) = q_request.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyC) {
        let toggle = match current_stance_hint(request.stance) {
            Stance::Standing => Stance::Crouching,
            Stance::Crouching => Stance::Standing,
        };
        info!("[INPUT] C pressed -> stance toggle to {toggle:?}");
        request.stance = Some(toggle);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        let next = cycle_gait(request.gait);
        info!("[INPUT] G pressed -> gait cycle to {next:?}");
        request.gait = Some(next);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let next = cycle_rotation(request.rotation_mode);
        info!("[INPUT] R pressed -> rotation mode cycle to {next:?}");
        request.rotation_mode = Some(next);
    }
    if keys.just_pressed(KeyCode::KeyJ) {
        let next = match current_locomotion_hint(request.locomotion_state) {
            LocomotionState::Grounded => LocomotionState::Air,
            _ => LocomotionState::Grounded,
        };
        info!("[INPUT] J pressed -> locomotion toggle to {next:?}");
        request.locomotion_state = Some(next);
    }
    if keys.just_pressed(KeyCode::KeyK) {
        request.ragdoll_exit = action == Some(&als_core::state::AlsNotifyLocomotionAction::Ragdolling);
        request.ragdoll_start = !request.ragdoll_exit;
        info!("[INPUT] K pressed -> ragdoll recovery={}", request.ragdoll_exit);
    }
}

fn current_stance_hint(requested: Option<Stance>) -> Stance {
    requested.unwrap_or(Stance::Standing)
}

fn current_locomotion_hint(requested: Option<LocomotionState>) -> LocomotionState {
    requested.unwrap_or(LocomotionState::Grounded)
}

fn cycle_gait(requested: Option<Gait>) -> Gait {
    match requested.unwrap_or(Gait::Walking) {
        Gait::Walking => Gait::Running,
        Gait::Running => Gait::Sprinting,
        Gait::Sprinting => Gait::Walking,
    }
}

fn cycle_rotation(requested: Option<RotationMode>) -> RotationMode {
    match requested.unwrap_or(RotationMode::VelocityDirection) {
        RotationMode::VelocityDirection => RotationMode::ViewDirection,
        RotationMode::ViewDirection => RotationMode::Aiming,
        RotationMode::Aiming => RotationMode::VelocityDirection,
    }
}

fn transition_logger(
    mut accepted: MessageReader<StateTransitionEvent>,
    mut rejected: MessageReader<StateTransitionRejectedEvent>,
) {
    for e in accepted.read() {
        info!(
            "[GATE][ACCEPTED] entity={:?} {:?} {:?} -> {:?}",
            e.entity, e.dimension, e.from, e.to
        );
    }
    for e in rejected.read() {
        info!(
            "[GATE][REJECTED] entity={:?} {:?} requested={:?} reason={:?}",
            e.entity, e.dimension, e.requested, e.reason
        );
    }
}
