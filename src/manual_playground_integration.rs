use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use als_camera::{AlsCameraDebugInfo, AlsCameraRig, AlsCameraSet, AlsCameraShakeRequest};
use als_core::character::AlsCharacter;
use als_core::dynamics::AlsMovementBaseDynamicsState;
use als_core::foot_ik::{AlsFootLockState, AlsFootProbe};
use als_core::input_request::{AlsCameraLeftShoulder, AlsFirstPersonView};
use als_core::mantle::AlsMantleState;
use als_core::motion::{AlsInputOverride, AlsViewYaw};
use als_core::roll::AlsRollState;
use als_core::start_stop::{StartStopPhase, StartStopState};
use als_core::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use als_core::transition::AlsStateRequest;
use als_core::view::AlsViewPitch;
use avian3d::prelude::{Collider, CollisionLayers, LinearVelocity, Position, RigidBody, Rotation};
use bevy::ecs::message::MessageWriter;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::{Real, Virtual};
use bevy_animation_graph::core::animated_scene::AnimatedSceneInstance;
use bevy_animation_graph::core::animation_graph_player::{AnimationGraphPlayer, AnimationSource};
use bevy_animation_graph::core::edge_data::DataValue;
use bevy_animation_graph::core::ragdoll::definition::BodyLabel;
use bevy_tnua::TnuaToggle;

use crate::character_assembly::{AlsCharacterAssembly, AlsCharacterAvatar};
use crate::control_rig_apply::AlsControlRigState;
use crate::debug_display::AlsDebugDisplayStats;
use crate::game_mode::AlsPlayerAdmissionComplete;
use crate::head_layer::AlsHeadState;
use crate::layering_layer::AlsLayeringState;
use crate::overlay_mode_menu::OverlayMenuState;
use crate::overlay_object::{AlsOverlayObject, AlsOverlayObjectState};
use crate::ragdoll_link::{AlsRagdollGroundFollow, AlsRagdollJointDrive};
use crate::roll_drive::AlsRollSlotRuntime;
use crate::show_debug_hud::{
    AlsShowDebugHudRoot, AlsShowDebugHudState, DebugCategory, apply_show_debug_key,
};
use crate::stance_layer::AlsStanceBlend;
use crate::turn_layer::AlsTurnDrive;
use crate::{
    air_jump_layer::AlsJumpVariantState,
    als_hud::{AlsHudDebugSection, AlsHudInputSection, AlsHudRoot, AlsHudState},
    gait_layer::AlsGaitBlend,
};

const GATE_TIMEOUT: Duration = Duration::from_secs(100);

#[derive(Resource, Default)]
pub struct ManualPlaygroundStartupReady {
    pub passed: bool,
}

pub struct ManualPlaygroundIntegrationPlugin;

impl Plugin for ManualPlaygroundIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ManualPlaygroundStartupReady>()
            .init_resource::<ManualIntegrationState>()
            .add_systems(Update, initialize_manual_integration)
            .add_systems(
                PreUpdate,
                drive_manual_integration
                    .after(bevy_enhanced_input::prelude::EnhancedInputSystems::Apply),
            )
            .add_systems(PostUpdate, observe_manual_integration.after(AlsCameraSet));
    }
}

#[derive(Component)]
struct ManualIntegrationPlatform;

#[derive(Resource)]
struct ManualIntegrationFixture {
    platform: Entity,
    platform_home: Vec3,
    locomotion_start: Vec3,
    mantle_start: Vec3,
    roll_start: Vec3,
    facing_yaw: f32,
    obstacle_mesh: Handle<Mesh>,
    obstacle_material: Handle<StandardMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum IntegrationPhase {
    #[default]
    Await,
    Locomotion,
    Turn,
    Mantle,
    Platform,
    Roll,
    Ragdoll,
    Camera,
    Interface,
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegrationScenario {
    Base,
    Ragdoll,
    Camera,
    Interface,
}

impl IntegrationScenario {
    fn from_environment() -> Self {
        match std::env::var("ALS_MANUAL_INTEGRATION_CASE").ok().as_deref() {
            Some("ragdoll") => Self::Ragdoll,
            Some("camera") => Self::Camera,
            Some("interface") => Self::Interface,
            _ => Self::Base,
        }
    }
}

#[derive(Default)]
struct IntegrationCoverage {
    assembly: bool,
    graph: bool,
    locomotion: bool,
    backward: bool,
    sideways: bool,
    crouching: bool,
    sprinting: bool,
    airborne: bool,
    landed: bool,
    overlay: bool,
    gait_layer: bool,
    stance_layer: bool,
    jump_variant: bool,
    turn_in_place: bool,
    layering: bool,
    overlay_object: bool,
    start_stop: bool,
    view_head: bool,
    control_rig: bool,
    mantle_started: bool,
    mantle_completed: bool,
    movement_base: bool,
    foot_probe: bool,
    foot_lock: bool,
    roll_started: bool,
    roll_slot: bool,
    ragdoll_entered: bool,
    ragdoll_follow: bool,
    ragdoll_joint_drive: bool,
    ragdoll_recovered: bool,
    camera_rig: bool,
    camera_collision: bool,
    camera_recovery: bool,
    camera_first_person: bool,
    camera_shoulder: bool,
    camera_shake: bool,
    camera_unscaled_time: bool,
    hud_layout: bool,
    hud_toggle: bool,
    hud_slomo: bool,
    overlay_menu: bool,
    show_debug: bool,
    debug_draw: bool,
}

#[derive(Resource)]
struct ManualIntegrationState {
    character: Option<Entity>,
    avatar: Option<Entity>,
    camera: Option<Entity>,
    spawn_translation: Option<Vec3>,
    spawn_yaw: Option<f32>,
    scenario: IntegrationScenario,
    phase: IntegrationPhase,
    phase_started: Instant,
    deadline: Instant,
    entered: bool,
    initialized: bool,
    camera_obstacles: [Option<Entity>; 2],
    camera_obstacle_removed: bool,
    camera_blocked_ratio: Option<f32>,
    shake_requested: bool,
    dilation_active: bool,
    initial_view_yaw: Option<f32>,
    initial_view_pitch: Option<f32>,
    mantle_target_y: Option<f32>,
    mantle_was_active: bool,
    ragdoll_seen: bool,
    ragdoll_exit_requested: bool,
    ragdoll_frames: u32,
    ragdoll_pelvis_boosted: bool,
    ragdoll_grounded_at: Option<Instant>,
    ragdoll_recovered_at: Option<Instant>,
    camera_scenario_armed: bool,
    camera_last_probe_secs: f32,
    camera_view_mode_requested: bool,
    camera_shoulder_requested: bool,
    interface_debug_armed: bool,
    interface_hud_hidden_seen: bool,
    interface_hud_restored_seen: bool,
    interface_slomo_seen: bool,
    interface_slomo_restored_seen: bool,
    interface_menu_open_seen: bool,
    interface_menu_closed_seen: bool,
    turn_body_yaw: Option<f32>,
    timed_out: bool,
    final_logged: bool,
    coverage: IntegrationCoverage,
}

impl Default for ManualIntegrationState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            character: None,
            avatar: None,
            camera: None,
            spawn_translation: None,
            spawn_yaw: None,
            scenario: IntegrationScenario::from_environment(),
            phase: IntegrationPhase::Await,
            phase_started: now,
            deadline: now + GATE_TIMEOUT,
            entered: false,
            initialized: false,
            camera_obstacles: [None, None],
            camera_obstacle_removed: false,
            camera_blocked_ratio: None,
            shake_requested: false,
            dilation_active: false,
            initial_view_yaw: None,
            initial_view_pitch: None,
            mantle_target_y: None,
            mantle_was_active: false,
            ragdoll_seen: false,
            ragdoll_exit_requested: false,
            ragdoll_frames: 0,
            ragdoll_pelvis_boosted: false,
            ragdoll_grounded_at: None,
            ragdoll_recovered_at: None,
            camera_scenario_armed: false,
            camera_last_probe_secs: -1.0,
            camera_view_mode_requested: false,
            camera_shoulder_requested: false,
            interface_debug_armed: false,
            interface_hud_hidden_seen: false,
            interface_hud_restored_seen: false,
            interface_slomo_seen: false,
            interface_slomo_restored_seen: false,
            interface_menu_open_seen: false,
            interface_menu_closed_seen: false,
            turn_body_yaw: None,
            timed_out: false,
            final_logged: false,
            coverage: IntegrationCoverage::default(),
        }
    }
}

#[derive(SystemParam)]
struct InterfaceProbe<'w, 's> {
    virtual_time: Res<'w, Time<Virtual>>,
    hud_state: Res<'w, AlsHudState>,
    menu_state: Res<'w, OverlayMenuState>,
    show_debug: Res<'w, AlsShowDebugHudState>,
    debug_stats: Res<'w, AlsDebugDisplayStats>,
    overlay_props: Query<'w, 's, (), With<AlsOverlayObject>>,
    hud_roots: Query<'w, 's, (), With<AlsHudRoot>>,
    hud_debug_nodes: Query<'w, 's, &'static Node, With<AlsHudDebugSection>>,
    hud_input_nodes: Query<'w, 's, &'static Node, With<AlsHudInputSection>>,
    show_debug_roots: Query<'w, 's, (), With<AlsShowDebugHudRoot>>,
}

fn spawn_manual_integration_fixture(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    facing_yaw: f32,
) {
    let facing = Quat::from_rotation_y(facing_yaw);
    let forward = facing * Vec3::NEG_Z;
    let right = facing * Vec3::X;
    let ground_y = origin.y - 0.92;
    let ledge_center = origin + forward * 6.0 + Vec3::Y * (ground_y + 0.4 - origin.y);
    let mantle_start = origin - forward * 1.5;
    let platform_home =
        origin + right * 9.0 + forward * 2.0 + Vec3::Y * (ground_y + 0.25 - origin.y);
    let locomotion_start = origin - right * 8.0 - forward * 2.0;
    let roll_start = origin - right * 8.0 + forward * 7.0;
    let ledge_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.48, 0.76),
        perceptual_roughness: 0.7,
        ..default()
    });
    let platform_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.48, 0.08),
        perceptual_roughness: 0.6,
        ..default()
    });
    let obstacle_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.08, 0.18),
        perceptual_roughness: 0.5,
        ..default()
    });
    commands.spawn((
        Name::new("manual_integration_mantle_ledge"),
        RigidBody::Static,
        Collider::cuboid(2.4, 0.8, 1.2),
        Transform::from_translation(ledge_center),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(4.8, 1.6, 2.4)))),
        MeshMaterial3d(ledge_material),
    ));
    let platform = commands
        .spawn((
            Name::new("manual_integration_platform"),
            ManualIntegrationPlatform,
            RigidBody::Kinematic,
            Collider::cuboid(3.0, 0.25, 3.0),
            LinearVelocity(Vec3::X * 1.5),
            Transform::from_translation(platform_home),
            Mesh3d(meshes.add(Mesh::from(Cuboid::new(6.0, 0.5, 6.0)))),
            MeshMaterial3d(platform_material),
        ))
        .id();
    commands.insert_resource(ManualIntegrationFixture {
        platform,
        platform_home,
        locomotion_start,
        mantle_start,
        roll_start,
        facing_yaw,
        obstacle_mesh: meshes.add(Mesh::from(Cuboid::new(1.6, 1.6, 1.6))),
        obstacle_material,
    });
    info!(
        "[MANUAL-INTEGRATION] visible fixture attached to playground origin=({:.2},{:.2},{:.2})",
        origin.x, origin.y, origin.z
    );
}

fn initialize_manual_integration(
    mut commands: Commands,
    mut state: ResMut<ManualIntegrationState>,
    startup: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    assemblies: Query<&AlsCharacterAssembly>,
    avatars: Query<&AnimatedSceneInstance, With<AlsCharacterAvatar>>,
    players: Query<&AnimationGraphPlayer>,
    cameras: Query<(&AlsCameraRig, &AlsCameraDebugInfo)>,
    toggles: Query<&TnuaToggle, With<AlsCharacter>>,
    view: Query<(&AlsViewYaw, &AlsViewPitch), With<AlsCharacter>>,
    transforms: Query<&Transform, With<AlsCharacter>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if state.initialized || !startup.passed {
        return;
    }
    let Ok(admission) = admissions.single() else {
        return;
    };
    let character = admission.character;
    let Ok(assembly) = assemblies.get(character) else {
        return;
    };
    let Ok(instance) = avatars.get(assembly.avatar) else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    let Some(camera) = assembly.camera else {
        return;
    };
    let Ok((_rig, debug)) = cameras.get(camera) else {
        return;
    };
    let Ok(toggle) = toggles.get(character) else {
        return;
    };
    let Ok((view_yaw, view_pitch)) = view.get(character) else {
        return;
    };
    let Ok(transform) = transforms.get(character) else {
        return;
    };
    let graph_ready = matches!(player.get_animation_source(), AnimationSource::Graph(_))
        && player.get_error().is_none();
    if !graph_ready || !debug.valid || *toggle != TnuaToggle::Enabled {
        return;
    }
    commands
        .entity(character)
        .insert(AlsInputOverride::default());
    let spawn_yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
    spawn_manual_integration_fixture(
        &mut commands,
        &mut meshes,
        &mut materials,
        transform.translation,
        spawn_yaw,
    );
    state.character = Some(character);
    state.avatar = Some(assembly.avatar);
    state.camera = Some(camera);
    state.spawn_translation = Some(transform.translation);
    state.spawn_yaw = Some(spawn_yaw);
    state.initial_view_yaw = Some(view_yaw.0);
    state.initial_view_pitch = Some(view_pitch.0);
    state.initialized = true;
    state.coverage.assembly = true;
    state.coverage.graph = true;
    let first_phase = match state.scenario {
        IntegrationScenario::Base => IntegrationPhase::Locomotion,
        IntegrationScenario::Ragdoll => IntegrationPhase::Ragdoll,
        IntegrationScenario::Camera => IntegrationPhase::Camera,
        IntegrationScenario::Interface => IntegrationPhase::Interface,
    };
    begin_phase(&mut state, first_phase);
    info!(
        "[MANUAL-INTEGRATION][EXECUTED] runtime_assembly scenario={:?} character={character:?} avatar={:?} camera={camera:?} playable_plugin=AlsPlayableCharacterPlugin runtime_plugin=AlsCharacterRuntimePlugin graph=Graph tnua=Enabled camera_valid=true",
        state.scenario, assembly.avatar,
    );
}

#[allow(clippy::type_complexity)]
fn drive_manual_integration(
    mut commands: Commands,
    fixture: Option<Res<ManualIntegrationFixture>>,
    mut state: ResMut<ManualIntegrationState>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut show_debug: ResMut<AlsShowDebugHudState>,
    mut shake_requests: MessageWriter<AlsCameraShakeRequest>,
    mut characters: Query<
        (
            &mut AlsInputOverride,
            &mut Transform,
            &mut Position,
            Option<&mut Rotation>,
            &mut LinearVelocity,
            &mut AlsStateRequest,
            &mut Gait,
            &mut OverlayState,
            &mut RotationMode,
            &mut AlsViewYaw,
            &mut AlsViewPitch,
            Option<&AlsRagdollGroundFollow>,
        ),
        (With<AlsCharacter>, Without<ManualIntegrationPlatform>),
    >,
    mut platforms: Query<
        (&mut Transform, Option<&mut Position>, &mut LinearVelocity),
        (With<ManualIntegrationPlatform>, Without<AlsCharacter>),
    >,
    cameras: Query<&AlsCameraDebugInfo, With<AlsCameraRig>>,
) {
    if !state.initialized || state.final_logged {
        return;
    }
    let Some(fixture) = fixture else {
        return;
    };
    if Instant::now() >= state.deadline {
        state.timed_out = true;
        virtual_time.set_relative_speed(1.0);
        begin_phase(&mut state, IntegrationPhase::Finish);
        return;
    }
    let Some(character) = state.character else {
        return;
    };
    let Ok((
        mut input,
        mut transform,
        mut position,
        rotation,
        mut velocity,
        mut request,
        mut gait,
        mut overlay,
        mut rotation_mode,
        mut view_yaw,
        mut view_pitch,
        ground_follow,
    )) = characters.get_mut(character)
    else {
        return;
    };
    let elapsed = state.phase_started.elapsed().as_secs_f32();
    let entering = std::mem::take(&mut state.entered);
    input.0.reset();

    match state.phase {
        IntegrationPhase::Await => {}
        IntegrationPhase::Locomotion => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.locomotion_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            }
            if (1.0..2.5).contains(&elapsed) {
                input.0.move_dir = Vec2::NEG_Y;
            } else if (2.5..4.0).contains(&elapsed) {
                input.0.move_dir = Vec2::X;
            } else if (4.0..5.5).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
            } else if (5.5..7.0).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
                input.0.sprint_held = true;
            } else if (7.0..8.5).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
                input.0.crouch_pressed = elapsed < 7.2;
            } else if (8.5..10.0).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
                input.0.crouch_pressed = elapsed < 8.7;
            } else if (10.0..12.0).contains(&elapsed) {
                input.0.move_dir = Vec2::Y;
                input.0.jump_pressed = elapsed < 10.2;
                input.0.jump_held = elapsed < 10.6;
            } else if (12.0..14.0).contains(&elapsed) {
                request.overlay_state = Some(OverlayState::Rifle);
            } else if (14.0..15.0).contains(&elapsed) {
                request.overlay_state = Some(OverlayState::Default);
            }
            if (0.5..1.5).contains(&elapsed) {
                input.0.look_delta = Vec2::new(6.0, -3.0);
                input.0.aim_held = true;
            }
            if elapsed >= 15.0 {
                begin_phase(&mut state, IntegrationPhase::Turn);
            }
        }
        IntegrationPhase::Turn => {
            if entering {
                let turn_start = state.spawn_translation.unwrap_or(transform.translation);
                let turn_yaw = state.spawn_yaw.unwrap_or(fixture.facing_yaw);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    turn_start,
                    turn_yaw,
                );
                state.turn_body_yaw = Some(turn_yaw);
                *gait = Gait::Walking;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            }
            if elapsed >= 0.5 {
                let body_yaw = state.turn_body_yaw.unwrap_or(fixture.facing_yaw);
                transform.rotation = Quat::from_rotation_y(body_yaw);
                view_yaw.0 = body_yaw + 90.0f32.to_radians();
            }
            if elapsed >= 2.5 {
                begin_phase(&mut state, IntegrationPhase::Mantle);
            }
        }
        IntegrationPhase::Mantle => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.mantle_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            }
            let forward = Quat::from_rotation_y(fixture.facing_yaw) * Vec3::NEG_Z;
            let progress = (transform.translation - fixture.mantle_start).dot(forward);
            input.0.move_dir = Vec2::Y;
            input.0.jump_pressed = (5.85..7.1).contains(&progress) && elapsed < 7.0;
            if elapsed >= 10.0 {
                begin_phase(&mut state, IntegrationPhase::Platform);
            }
        }
        IntegrationPhase::Platform => {
            if entering {
                if let Ok((mut platform_transform, platform_position, mut platform_velocity)) =
                    platforms.get_mut(fixture.platform)
                {
                    let platform_location = fixture.platform_home;
                    platform_transform.translation = platform_location;
                    if let Some(mut platform_position) = platform_position {
                        platform_position.0 = platform_location;
                    }
                    platform_velocity.0 = Vec3::X * 1.5;
                }
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.platform_home + Vec3::Y * 1.17,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                request.locomotion_state = Some(LocomotionState::Grounded);
            }
            if elapsed >= 6.0 {
                begin_phase(&mut state, IntegrationPhase::Roll);
            }
        }
        IntegrationPhase::Roll => {
            if entering {
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    fixture.roll_start,
                    fixture.facing_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            }
            input.0.move_dir = Vec2::Y;
            input.0.roll_pressed = (1.0..1.2).contains(&elapsed);
            if elapsed >= 5.5 {
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Ragdoll => {
            if entering {
                state.ragdoll_seen = false;
                state.ragdoll_exit_requested = false;
                state.ragdoll_frames = 0;
                state.ragdoll_pelvis_boosted = false;
                state.ragdoll_grounded_at = None;
                state.ragdoll_recovered_at = None;
                state.coverage.ragdoll_entered = false;
                state.coverage.ragdoll_follow = false;
                state.coverage.ragdoll_joint_drive = false;
                state.coverage.ragdoll_recovered = false;
                let spawn_translation = state.spawn_translation.unwrap_or(transform.translation);
                let spawn_yaw = state.spawn_yaw.unwrap_or(fixture.facing_yaw);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    spawn_translation,
                    spawn_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                request.locomotion_state = Some(LocomotionState::Grounded);
                request.stance = Some(Stance::Standing);
            }
            let ready_to_exit = elapsed >= 2.0
                && state.coverage.ragdoll_follow
                && state.coverage.ragdoll_joint_drive
                && ground_follow.is_some_and(|follow| follow.grounded);
            if state.ragdoll_exit_requested {
            } else if elapsed < 1.0 && state.scenario != IntegrationScenario::Ragdoll {
                input.0.move_dir = Vec2::Y;
            } else if !ready_to_exit {
                request.locomotion_state = Some(LocomotionState::Ragdolling);
            } else {
                request.ragdoll_exit = true;
                state.ragdoll_exit_requested = true;
            }
            if state
                .ragdoll_recovered_at
                .is_some_and(|at| at.elapsed() >= Duration::from_secs_f32(1.5))
                && elapsed >= 3.0
            {
                begin_phase(&mut state, IntegrationPhase::Finish);
            } else if elapsed >= 20.0 {
                state.timed_out = true;
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Camera => {
            if entering {
                state.coverage.camera_collision = false;
                state.coverage.camera_recovery = false;
                state.camera_obstacles = [None, None];
                state.camera_obstacle_removed = false;
                state.camera_blocked_ratio = None;
                state.camera_scenario_armed = false;
                state.camera_last_probe_secs = -1.0;
                state.camera_view_mode_requested = false;
                state.camera_shoulder_requested = false;
                let camera_spawn = state.spawn_translation.unwrap_or(transform.translation);
                let camera_yaw = state.spawn_yaw.unwrap_or(fixture.facing_yaw);
                reset_character(
                    &mut transform,
                    &mut position,
                    rotation,
                    &mut velocity,
                    &mut view_yaw,
                    &mut view_pitch,
                    camera_spawn,
                    camera_yaw,
                );
                *gait = Gait::Running;
                *overlay = OverlayState::Default;
                *rotation_mode = RotationMode::ViewDirection;
            }
            if elapsed < 1.0 {
                input.0.look_delta = Vec2::new(8.0, -4.0);
            }
            if !state.camera_shoulder_requested {
                input.0.switch_shoulder_pressed = true;
                state.camera_shoulder_requested = true;
            }
            if elapsed >= 2.0 && !state.camera_view_mode_requested {
                input.0.view_mode_pressed = true;
                state.camera_view_mode_requested = true;
            }
            if (3.0..4.0).contains(&elapsed) {
                input.0.look_delta = Vec2::new(2.0, 5.0);
                if !state.dilation_active {
                    virtual_time.set_relative_speed(0.2);
                    state.dilation_active = true;
                }
            } else if state.dilation_active {
                virtual_time.set_relative_speed(1.0);
                state.dilation_active = false;
            }
            if elapsed >= 4.0 && !state.shake_requested {
                shake_requests.write(AlsCameraShakeRequest::sprint(character, 1.0));
                state.shake_requested = true;
            }
            if elapsed >= 0.5
                && state.camera_obstacles.iter().all(Option::is_none)
                && !state.camera_obstacle_removed
            {
                if let Some(camera) = state.camera
                    && let Ok(debug) = cameras.get(camera)
                    && debug.valid
                    && debug.trace_start.distance(debug.trace_end) > 0.5
                {
                    let owner_direction =
                        (transform.translation - debug.trace_start).normalize_or_zero();
                    let owner_direction = if owner_direction.length_squared() > 1e-6 {
                        owner_direction
                    } else {
                        Vec3::Y
                    };
                    let trace_direction = (debug.trace_end - debug.trace_start).normalize_or_zero();
                    let lateral = trace_direction.cross(owner_direction).normalize_or_zero();
                    let lateral = if lateral.length_squared() > 1e-6 {
                        lateral
                    } else {
                        Vec3::X
                    };
                    let directions = [
                        owner_direction,
                        (owner_direction + lateral * 0.35).normalize_or_zero(),
                    ];
                    for (index, direction) in directions.into_iter().enumerate() {
                        let direction = if direction.length_squared() > 1e-6 {
                            direction
                        } else {
                            owner_direction
                        };
                        let center = debug.trace_start - direction * 0.1;
                        state.camera_obstacles[index] = Some(
                            commands
                                .spawn((
                                    Name::new(format!(
                                        "manual_integration_camera_obstacle_{index}"
                                    )),
                                    RigidBody::Static,
                                    CollisionLayers::from_bits(0b0010, 0),
                                    Collider::cuboid(0.05, 0.6, 0.6),
                                    Transform::from_translation(center)
                                        .with_rotation(Quat::from_rotation_arc(Vec3::X, direction)),
                                    Mesh3d(fixture.obstacle_mesh.clone()),
                                    MeshMaterial3d(fixture.obstacle_material.clone()),
                                ))
                                .id(),
                        );
                    }
                    state.camera_scenario_armed = true;
                    info!("[MANUAL-INTEGRATION] camera MTD pair armed");
                }
            }
            if state.coverage.camera_collision && !state.camera_obstacle_removed {
                for obstacle in state.camera_obstacles.iter_mut().flatten() {
                    commands.entity(*obstacle).despawn();
                }
                state.camera_obstacles = [None, None];
                state.camera_obstacle_removed = true;
                info!("[MANUAL-INTEGRATION] camera MTD pair removed after collision probe");
            }
            if elapsed >= 9.0 {
                virtual_time.set_relative_speed(1.0);
                state.dilation_active = false;
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Interface => {
            if entering {
                state.interface_debug_armed = false;
                state.interface_hud_hidden_seen = false;
                state.interface_hud_restored_seen = false;
                state.interface_slomo_seen = false;
                state.interface_slomo_restored_seen = false;
                state.interface_menu_open_seen = false;
                state.interface_menu_closed_seen = false;
                state.coverage.hud_layout = false;
                state.coverage.hud_toggle = false;
                state.coverage.hud_slomo = false;
                state.coverage.overlay_menu = false;
                state.coverage.show_debug = false;
                state.coverage.debug_draw = false;
                for category in DebugCategory::ALL {
                    apply_show_debug_key(
                        &mut show_debug,
                        true,
                        match category {
                            DebugCategory::Curves => KeyCode::Digit1,
                            DebugCategory::State => KeyCode::Digit2,
                            DebugCategory::Shapes => KeyCode::Digit3,
                            DebugCategory::Traces => KeyCode::Digit4,
                            DebugCategory::Mantling => KeyCode::Digit5,
                            DebugCategory::CameraCurves => KeyCode::Digit6,
                            DebugCategory::CameraShapes => KeyCode::Digit7,
                            DebugCategory::CameraTraces => KeyCode::Digit8,
                        },
                    );
                }
                state.interface_debug_armed = true;
                info!(
                    "[MANUAL-INTEGRATION][INTERFACE] HUD, overlay menu, and show-debug probes armed"
                );
            }
            drive_key_interval(&mut keys, KeyCode::Tab, elapsed, 0.25, 0.55);
            drive_key_interval(&mut keys, KeyCode::Tab, elapsed, 0.95, 1.25);
            drive_key_interval(&mut keys, KeyCode::KeyZ, elapsed, 1.65, 1.95);
            drive_key_interval(&mut keys, KeyCode::KeyZ, elapsed, 2.25, 2.55);
            drive_key_interval(&mut keys, KeyCode::KeyQ, elapsed, 2.9, 3.9);
            if elapsed >= 5.0 {
                keys.release(KeyCode::Tab);
                keys.release(KeyCode::KeyZ);
                keys.release(KeyCode::KeyQ);
                virtual_time.set_relative_speed(1.0);
                begin_phase(&mut state, IntegrationPhase::Finish);
            }
        }
        IntegrationPhase::Finish => {}
    }
}

fn reset_character(
    transform: &mut Transform,
    position: &mut Position,
    rotation: Option<Mut<Rotation>>,
    velocity: &mut LinearVelocity,
    view_yaw: &mut AlsViewYaw,
    view_pitch: &mut AlsViewPitch,
    translation: Vec3,
    yaw: f32,
) {
    let rotation_value = Quat::from_rotation_y(yaw);
    transform.translation = translation;
    transform.rotation = rotation_value;
    position.0 = translation;
    if let Some(mut rotation) = rotation {
        rotation.0 = rotation_value;
    }
    velocity.0 = Vec3::ZERO;
    view_yaw.0 = yaw;
    view_pitch.0 = 0.0;
}

fn drive_key_interval(
    keys: &mut ButtonInput<KeyCode>,
    key: KeyCode,
    elapsed: f32,
    start: f32,
    end: f32,
) {
    if (start..end).contains(&elapsed) {
        keys.press(key);
    } else if elapsed >= end {
        keys.release(key);
    }
}

fn begin_phase(state: &mut ManualIntegrationState, phase: IntegrationPhase) {
    state.phase = phase;
    state.phase_started = Instant::now();
    state.entered = true;
    info!("[MANUAL-INTEGRATION][PHASE] {phase:?}");
}

fn graph_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    match player.get_input_data(key.to_owned()) {
        Some(DataValue::F32(value)) if value.is_finite() => Some(value),
        _ => None,
    }
}

fn graph_global_f32(player: &AnimationGraphPlayer, key: &str) -> Option<f32> {
    match player.get_global_input_data(key) {
        Some(DataValue::F32(value)) if value.is_finite() => Some(*value),
        _ => None,
    }
}

#[allow(clippy::type_complexity)]
fn observe_manual_integration(
    mut commands: Commands,
    fixture: Option<Res<ManualIntegrationFixture>>,
    mut state: ResMut<ManualIntegrationState>,
    real_time: Res<Time<Real>>,
    interface: InterfaceProbe,
    core: Query<
        (
            &Transform,
            &LinearVelocity,
            &Stance,
            &Gait,
            &LocomotionState,
            &OverlayState,
            &AlsViewYaw,
            &AlsViewPitch,
            Has<AlsFirstPersonView>,
            Has<AlsCameraLeftShoulder>,
            Option<&StartStopState>,
            Option<&AlsControlRigState>,
            Option<&AlsHeadState>,
            Option<&AlsLayeringState>,
            Option<&AlsOverlayObjectState>,
        ),
        With<AlsCharacter>,
    >,
    action: Query<
        (
            Option<&AlsMantleState>,
            Option<&AlsRollState>,
            Option<&AlsRollSlotRuntime>,
            Option<&AlsFootProbe>,
            Option<&AlsFootLockState>,
            Option<&AlsMovementBaseDynamicsState>,
            Option<&AlsRagdollGroundFollow>,
            Option<&AlsRagdollJointDrive>,
        ),
        With<AlsCharacter>,
    >,
    avatars: Query<(
        &AlsCharacterAvatar,
        &AnimatedSceneInstance,
        Option<&AlsGaitBlend>,
        Option<&AlsStanceBlend>,
        Option<&AlsJumpVariantState>,
        Option<&AlsTurnDrive>,
    )>,
    players: Query<&AnimationGraphPlayer>,
    labels: Query<&BodyLabel>,
    cameras: Query<(&Transform, &AlsCameraDebugInfo), With<AlsCameraRig>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !state.initialized {
        return;
    }
    let Some(fixture) = fixture else {
        return;
    };
    let Some(character) = state.character else {
        return;
    };
    let Ok((
        transform,
        velocity,
        stance,
        gait,
        locomotion,
        overlay,
        view_yaw,
        view_pitch,
        first_person,
        left_shoulder,
        start_stop,
        control_rig,
        head,
        layering,
        overlay_object,
    )) = core.get(character)
    else {
        return;
    };
    let Ok((
        mantle,
        roll,
        roll_slot,
        foot_probe,
        foot_lock,
        movement_base,
        ragdoll_follow,
        joint_drive,
    )) = action.get(character)
    else {
        return;
    };
    let Some((_, instance, gait_blend, stance_blend, jump_variant, turn_drive)) = avatars
        .iter()
        .find(|(avatar, _, _, _, _, _)| avatar.character == character)
    else {
        return;
    };
    let Ok(player) = players.get(instance.player_entity()) else {
        return;
    };
    let speed = Vec2::new(velocity.0.x, velocity.0.z).length();
    let direction = graph_f32(player, "direction_angle");
    let jump_phase = graph_global_f32(player, "als_locomotion_jump_phase");
    let land_phase = graph_global_f32(player, "als_locomotion_land_phase");
    let overlay_index = graph_global_f32(player, "als_root_overlay_index");
    let source_ready = matches!(player.get_animation_source(), AnimationSource::Graph(_))
        && player.get_error().is_none();
    if !source_ready {
        state.coverage.graph = false;
    }
    state.coverage.locomotion |= speed > 0.25 && direction.is_some();
    if speed > 0.5 && direction.is_some_and(|value| value.abs() > 150.0) {
        state.coverage.backward = true;
    }
    if speed > 0.5 && direction.is_some_and(|value| (60.0..120.0).contains(&value.abs())) {
        state.coverage.sideways = true;
    }
    state.coverage.crouching |= *stance == Stance::Crouching && speed > 0.1;
    state.coverage.sprinting |= *gait == Gait::Sprinting && speed > 0.1;
    if *locomotion == LocomotionState::Air && jump_phase.is_some() {
        state.coverage.airborne = true;
    }
    if state.coverage.airborne
        && *locomotion == LocomotionState::Grounded
        && land_phase.is_some_and(|value| value > 1e-4)
    {
        state.coverage.landed = true;
    }
    state.coverage.overlay |= *overlay == OverlayState::Rifle && overlay_index.is_some();
    state.coverage.gait_layer |=
        gait_blend.is_some_and(|blend| blend.weights.sprint_w > 1e-4) && *gait == Gait::Sprinting;
    state.coverage.stance_layer |=
        stance_blend.is_some_and(|blend| blend.weight > 1e-4) && *stance == Stance::Crouching;
    state.coverage.jump_variant |=
        jump_variant.is_some_and(|variant| variant.airborne) && *locomotion == LocomotionState::Air;
    state.coverage.turn_in_place |= turn_drive.is_some_and(|drive| {
        (drive.playing && drive.weight > 1e-4)
            || (drive.rotate_active && drive.rotate_weight > 1e-4)
    });
    state.coverage.layering |= *stance == Stance::Crouching
        && layering.is_some()
        && graph_global_f32(player, "als_layering_crouching_amount")
            .is_some_and(|value| value > 1e-4);
    state.coverage.overlay_object |= overlay_object.is_some_and(|object| {
        object.visible
            && object.applied_overlay == Some(OverlayState::Rifle)
            && object
                .prop
                .is_some_and(|prop| interface.overlay_props.get(prop).is_ok())
    });
    state.coverage.start_stop |= start_stop.is_some_and(|value| {
        matches!(
            value.phase,
            StartStopPhase::Started(_) | StartStopPhase::Stopping(_)
        )
    });
    state.coverage.view_head |= state
        .initial_view_yaw
        .is_some_and(|initial| (view_yaw.0 - initial).abs() > 0.01)
        && state
            .initial_view_pitch
            .is_some_and(|initial| (view_pitch.0 - initial).abs() > 0.01)
        && head.is_some_and(|value| value.pitch_angle.abs() > 0.01 || value.yaw_angle.abs() > 0.01);
    state.coverage.control_rig |= control_rig
        .is_some_and(|value| value.spine_rotation_allowed && value.spine_amount.abs() > 1e-4);

    if let Some(mantle) = mantle {
        if mantle.active {
            state.coverage.mantle_started = true;
            state.mantle_target_y = Some(mantle.target.y);
        }
        if state.mantle_was_active
            && !mantle.active
            && state
                .mantle_target_y
                .is_some_and(|target_y| transform.translation.y >= target_y - 0.3)
        {
            state.coverage.mantle_completed = true;
        }
        state.mantle_was_active = mantle.active;
    }
    if let Some(probe) = foot_probe {
        state.coverage.foot_probe |= probe.left.is_some() && probe.right.is_some();
    }
    if let Some(lock) = foot_lock {
        state.coverage.foot_lock |= lock.valid;
    }
    if let Some(base) = movement_base {
        state.coverage.movement_base |= base.entity == Some(fixture.platform)
            && base.support_velocity.length() > 0.25
            && base.relative_velocity.length() < 0.75;
    }
    if let Some(roll) = roll {
        state.coverage.roll_started |= roll.active;
    }
    if let Some(slot) = roll_slot {
        state.coverage.roll_slot |= slot.active
            && slot.pose_ready
            && slot.weight > 0.2
            && slot.root_motion_delta.length() > 1e-5;
    }
    if *locomotion == LocomotionState::Ragdolling {
        state.coverage.ragdoll_entered = true;
        state.ragdoll_seen = true;
        state.ragdoll_frames = state.ragdoll_frames.saturating_add(1);
        if state
            .ragdoll_grounded_at
            .is_some_and(|at| at.elapsed() >= Duration::from_secs_f32(0.75))
            && !state.ragdoll_pelvis_boosted
        {
            if let Some(spawned) = player.get_spawned_ragdoll()
                && let Some(pelvis) = spawned
                    .bodies
                    .values()
                    .find(|body| labels.get(**body).is_ok_and(|label| label.0 == "pelvis"))
            {
                commands
                    .entity(*pelvis)
                    .insert(LinearVelocity(Vec3::X * 0.1));
                state.ragdoll_pelvis_boosted = true;
                info!("[MANUAL-INTEGRATION] ragdoll pelvis drive injected speed=0.100");
            }
        }
    }
    if let Some(follow) = ragdoll_follow {
        if state.ragdoll_seen && follow.grounded {
            state.coverage.ragdoll_follow = true;
            if state.ragdoll_grounded_at.is_none() {
                state.ragdoll_grounded_at = Some(Instant::now());
                info!("[MANUAL-INTEGRATION] ragdoll ground follow stabilized");
            }
        } else if state.ragdoll_seen {
            state.ragdoll_grounded_at = None;
        }
    }
    if let Some(drive) = joint_drive {
        state.coverage.ragdoll_joint_drive |= state.ragdoll_seen
            && drive.target_joint_count > 0
            && drive.driven_joint_count > 0
            && drive.angular_stiffness > 0.0;
    }
    let ragdoll_recovered = state.ragdoll_seen
        && state.ragdoll_exit_requested
        && *locomotion == LocomotionState::Grounded
        && velocity.0.length() < 0.5;
    if ragdoll_recovered {
        state.coverage.ragdoll_recovered = true;
        if state.ragdoll_recovered_at.is_none() {
            state.ragdoll_recovered_at = Some(Instant::now());
            info!(
                "[MANUAL-INTEGRATION] ragdoll recovered position=({:.3},{:.3},{:.3}) velocity={:.3}",
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                velocity.0.length()
            );
        }
    }

    if state.scenario == IntegrationScenario::Interface {
        let hud_nodes_hidden = !interface.hud_debug_nodes.is_empty()
            && !interface.hud_input_nodes.is_empty()
            && interface
                .hud_debug_nodes
                .iter()
                .all(|node| node.display == Display::None)
            && interface
                .hud_input_nodes
                .iter()
                .all(|node| node.display == Display::None);
        let hud_nodes_visible = !interface.hud_debug_nodes.is_empty()
            && !interface.hud_input_nodes.is_empty()
            && interface
                .hud_debug_nodes
                .iter()
                .all(|node| node.display != Display::None)
            && interface
                .hud_input_nodes
                .iter()
                .all(|node| node.display != Display::None);
        state.coverage.hud_layout |= !interface.hud_roots.is_empty()
            && !interface.hud_debug_nodes.is_empty()
            && !interface.hud_input_nodes.is_empty();
        state.interface_hud_hidden_seen |= !interface.hud_state.ui_visible && hud_nodes_hidden;
        state.interface_hud_restored_seen |=
            state.interface_hud_hidden_seen && interface.hud_state.ui_visible && hud_nodes_visible;
        state.coverage.hud_toggle =
            state.interface_hud_hidden_seen && state.interface_hud_restored_seen;

        state.interface_slomo_seen |= interface.hud_state.slomo_active
            && (interface.virtual_time.relative_speed() - 0.15).abs() < 1e-4;
        state.interface_slomo_restored_seen |= state.interface_slomo_seen
            && !interface.hud_state.slomo_active
            && (interface.virtual_time.relative_speed() - 1.0).abs() < 1e-4;
        state.coverage.hud_slomo =
            state.interface_slomo_seen && state.interface_slomo_restored_seen;

        state.interface_menu_open_seen |= interface.menu_state.open
            && interface.menu_state.root.is_some()
            && (interface.virtual_time.relative_speed() - 0.35).abs() < 1e-4;
        state.interface_menu_closed_seen |= state.interface_menu_open_seen
            && !interface.menu_state.open
            && interface.menu_state.root.is_none()
            && (interface.virtual_time.relative_speed() - 1.0).abs() < 1e-4;
        state.coverage.overlay_menu =
            state.interface_menu_open_seen && state.interface_menu_closed_seen;

        let all_debug_categories_active = DebugCategory::ALL
            .into_iter()
            .all(|category| interface.show_debug.is_active(category));
        state.coverage.show_debug |= state.interface_debug_armed
            && all_debug_categories_active
            && !interface.show_debug_roots.is_empty();
        state.coverage.debug_draw |= state.coverage.show_debug && interface.debug_stats.total() > 0;
    }

    if let Some(camera) = state.camera
        && let Ok((_camera_transform, debug)) = cameras.get(camera)
    {
        let camera_debug = debug;
        state.coverage.camera_rig |= camera_debug.valid;
        if state.phase == IntegrationPhase::Camera
            && state.camera_scenario_armed
            && state.camera_obstacles.iter().any(Option::is_some)
            && camera_debug.trace_blocking_hit
            && camera_debug.trace_distance_ratio < 0.98
            && camera_debug.trace_adjustment_count >= 2
            && camera_debug.trace_adjustment_resolved
        {
            if !state.coverage.camera_collision {
                info!(
                    "[MANUAL-INTEGRATION][CAMERA][MTD] blocking={} adjustment_count={} resolved={} ratio={:.3}",
                    camera_debug.trace_blocking_hit,
                    camera_debug.trace_adjustment_count,
                    camera_debug.trace_adjustment_resolved,
                    camera_debug.trace_distance_ratio
                );
            }
            state.coverage.camera_collision = true;
            state.camera_blocked_ratio = Some(camera_debug.trace_distance_ratio);
        }
        if state.phase == IntegrationPhase::Camera
            && state.camera_scenario_armed
            && state.camera_obstacles.iter().any(Option::is_some)
        {
            let elapsed = state.phase_started.elapsed().as_secs_f32();
            if elapsed - state.camera_last_probe_secs >= 0.5 {
                state.camera_last_probe_secs = elapsed;
                info!(
                    "[MANUAL-INTEGRATION][CAMERA] blocking={} adjusted={} adjustment_count={} resolved={} ratio={:.3} first_person={:.3}",
                    camera_debug.trace_blocking_hit,
                    camera_debug.trace_adjusted_start,
                    camera_debug.trace_adjustment_count,
                    camera_debug.trace_adjustment_resolved,
                    camera_debug.trace_distance_ratio,
                    camera_debug.first_person_override
                );
            }
        }
        if state.camera_obstacle_removed
            && state
                .camera_blocked_ratio
                .is_some_and(|blocked| camera_debug.trace_distance_ratio > blocked + 0.02)
            && !camera_debug.trace_blocking_hit
        {
            state.coverage.camera_recovery = true;
        }
        state.coverage.camera_first_person |=
            first_person && camera_debug.first_person_override > 0.5;
        state.coverage.camera_shoulder |= left_shoulder && !camera_debug.right_shoulder;
        state.coverage.camera_shake |= camera_debug.shake_location_offset.length() > 1e-5
            || camera_debug.shake_rotation_degrees.length() > 1e-5;
        if state.dilation_active && real_time.delta_secs() > 1e-5 {
            let ratio = camera_debug.unscaled_delta_seconds / real_time.delta_secs();
            state.coverage.camera_unscaled_time |= (ratio - 1.0).abs() < 0.25;
        }
    }

    if state.phase == IntegrationPhase::Finish && !state.final_logged {
        state.final_logged = true;
        let coverage = &state.coverage;
        let results = match state.scenario {
            IntegrationScenario::Base => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("locomotion", coverage.locomotion),
                ("direction_backward", coverage.backward),
                ("direction_sideways", coverage.sideways),
                ("stance_crouching", coverage.crouching),
                ("gait_sprinting", coverage.sprinting),
                ("jump_air", coverage.airborne),
                ("jump_land", coverage.landed),
                ("jump_variant", coverage.jump_variant),
                ("overlay_layer", coverage.overlay),
                ("gait_layer", coverage.gait_layer),
                ("stance_layer", coverage.stance_layer),
                ("turn_in_place", coverage.turn_in_place),
                ("layering", coverage.layering),
                ("overlay_object", coverage.overlay_object),
                ("start_stop", coverage.start_stop),
                ("view_head", coverage.view_head),
                ("control_rig", coverage.control_rig),
                ("mantle_start", coverage.mantle_started),
                ("mantle_complete", coverage.mantle_completed),
                ("movement_base", coverage.movement_base),
                ("foot_probe", coverage.foot_probe),
                ("foot_lock", coverage.foot_lock),
                ("roll_start", coverage.roll_started),
                ("roll_slot", coverage.roll_slot),
            ],
            IntegrationScenario::Ragdoll => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("ragdoll_enter", coverage.ragdoll_entered),
                ("ragdoll_follow", coverage.ragdoll_follow),
                ("ragdoll_joint_drive", coverage.ragdoll_joint_drive),
                ("ragdoll_recover", coverage.ragdoll_recovered),
            ],
            IntegrationScenario::Camera => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("camera_rig", coverage.camera_rig),
                ("camera_collision", coverage.camera_collision),
                ("camera_collision_recovery", coverage.camera_recovery),
                ("camera_first_person", coverage.camera_first_person),
                ("camera_shoulder", coverage.camera_shoulder),
                ("camera_shake", coverage.camera_shake),
                ("camera_unscaled_time", coverage.camera_unscaled_time),
            ],
            IntegrationScenario::Interface => vec![
                ("runtime_assembly", coverage.assembly),
                ("animation_graph", coverage.graph),
                ("hud_layout", coverage.hud_layout),
                ("hud_toggle", coverage.hud_toggle),
                ("hud_slomo", coverage.hud_slomo),
                ("overlay_menu", coverage.overlay_menu),
                ("show_debug", coverage.show_debug),
                ("debug_draw", coverage.debug_draw),
            ],
        };
        for (name, covered) in &results {
            if *covered {
                info!("[MANUAL-INTEGRATION][EXECUTED] {name}");
            } else {
                error!("[MANUAL-INTEGRATION][NOT_COVERED] {name}");
            }
        }
        let passed = !state.timed_out && results.iter().all(|(_, covered)| *covered);
        if passed {
            info!(
                "[MANUAL-INTEGRATION][VERDICT] scenario={:?} real_entry_paths -> PASS",
                state.scenario
            );
            exit.write(AppExit::Success);
        } else {
            error!(
                "[MANUAL-INTEGRATION][VERDICT] scenario={:?} real_entry_paths -> FAIL timed_out={}",
                state.scenario, state.timed_out
            );
            exit.write(AppExit::Error(NonZeroU8::new(91).unwrap()));
        }
    }
}
