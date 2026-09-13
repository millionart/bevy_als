use super::*;
use als_core::aim::AlsLastFreeRotationMode;
use als_core::input::AlsInputCommand;
use als_core::turn_in_place::wrap_angle_degrees;
use crate::playground_collision::PlaygroundMeshColliderBuilt;

const MESHES: [&str; 3] = ["SM_Als_Stairs_01", "SM_Als_Stairs_02", "SM_Als_Stairs_03"];
const NO_PROGRESS_SECONDS: f64 = 5.0;

#[derive(Clone, Copy, Debug)]
struct Route {
    bottom: Vec3,
    top: Vec3,
}

impl Route {
    fn from_collider(collider: &Collider, transform: &GlobalTransform, half_height: f32) -> Option<Self> {
        let mesh = collider.shape().as_trimesh()?;
        let vertices = mesh.vertices();
        let min = vertices.iter().map(|point| Vec3::new(point.x, point.y, point.z)).reduce(Vec3::min)?;
        let max = vertices.iter().map(|point| Vec3::new(point.x, point.y, point.z)).reduce(Vec3::max)?;
        let top_vertices = vertices.iter().filter(|point| (point.y - max.y).abs() < 1e-4);
        let top_z_min = top_vertices.clone().map(|point| point.z).reduce(f32::min)?;
        let top_z_max = top_vertices.map(|point| point.z).reduce(f32::max)?;
        let center_x = (min.x + max.x) * 0.5;
        let center_offset = Vec3::Y * half_height;
        Some(Self {
            bottom: transform.transform_point(Vec3::new(center_x, min.y, max.z + 0.75)) + center_offset,
            top: transform.transform_point(Vec3::new(center_x, max.y, (top_z_min + top_z_max) * 0.5)) + center_offset,
        })
    }

    fn endpoints(self, index: usize) -> (Vec3, Vec3) {
        if index % 2 == 0 { (self.bottom, self.top) } else { (self.top, self.bottom) }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
enum Stage {
    #[default]
    Ready,
    Traverse,
    Settle,
    VelocityMode,
    ViewMode,
}

#[derive(Default)]
pub(super) struct Probe {
    route: Option<Route>,
    index: usize,
    stage: Stage,
    stage_started: f64,
    last_progress_at: f64,
    furthest: f32,
    pub(super) completed: [bool; 6],
    pub(super) failure: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
struct Frame {
    position: Vec3,
    velocity: Vec3,
    locomotion: LocomotionState,
    stance: Stance,
    desired_gait: Gait,
    desired_mode: RotationMode,
    current_mode: RotationMode,
    view_yaw: f32,
}

impl Probe {
    fn begin(&mut self, stage: Stage, now: f64) {
        self.stage = stage;
        self.stage_started = now;
        self.last_progress_at = now;
        info!("[MANUAL-INTEGRATION][STAIRS] route={} stage={stage:?}", self.index);
    }

    fn command(&mut self, now: f64, frame: Frame) -> AlsInputCommand {
        let mut command = AlsInputCommand::default();
        let Some(route) = self.route else { return command; };
        let (start, end) = route.endpoints(self.index);
        let direction = (end - start).with_y(0.0).normalize();
        let length = (end - start).with_y(0.0).length();
        let distance = (frame.position - start).dot(direction);
        let remaining = length - distance;
        let yaw_error = wrap_angle_degrees((world_forward_to_yaw(direction) - frame.view_yaw).to_degrees()).to_radians();
        let speed = frame.velocity.with_y(0.0).length();
        command.look_delta.x = -yaw_error / als_core::view::LOOK_MOUSE_RADIANS_PER_COUNT;
        command.crouch_pressed = frame.stance != Stance::Standing;
        let gait = if self.stage != Stage::Traverse || distance < 3.5 || remaining < 4.5 {
            Gait::Walking
        } else if distance < 8.0 { Gait::Running } else { Gait::Sprinting };
        command.sprint_held = gait == Gait::Sprinting;
        command.walk_pressed = matches!((gait, frame.desired_gait),
            (Gait::Walking, Gait::Running | Gait::Sprinting) | (Gait::Running, Gait::Walking));
        match self.stage {
            Stage::Ready => {
                command.rotation_mode_pressed = frame.desired_mode != RotationMode::ViewDirection;
                if frame.locomotion == LocomotionState::Grounded && frame.stance == Stance::Standing
                    && speed < 0.05 && yaw_error.abs() < 1.0_f32.to_radians()
                    && frame.current_mode == RotationMode::ViewDirection
                    && frame.desired_mode == RotationMode::ViewDirection
                    && now - self.stage_started >= 0.3 {
                    self.begin(Stage::Traverse, now);
                }
            }
            Stage::Traverse => {
                if frame.locomotion != LocomotionState::Grounded {
                    self.failure = Some(format!("route={} became airborne at distance={distance:.3}", self.index));
                } else if remaining <= 0.03 {
                    self.begin(Stage::Settle, now);
                } else {
                    command.move_dir = Vec2::Y * remaining.clamp(0.0, 1.0);
                    if distance > self.furthest + 0.1 {
                        if distance.floor() > self.furthest.floor() {
                            info!("[MANUAL-INTEGRATION][STAIRS] route={} distance={distance:.3}", self.index);
                        }
                        self.furthest = distance;
                        self.last_progress_at = now;
                    }
                }
            }
            Stage::Settle => {
                if speed < 0.05 && frame.locomotion == LocomotionState::Grounded
                    && (frame.position - end).with_y(0.0).length() < 0.15
                    && (frame.position.y - end.y).abs() < 0.1 {
                    self.begin(Stage::VelocityMode, now);
                }
            }
            Stage::VelocityMode | Stage::ViewMode => {
                let target = if self.stage == Stage::VelocityMode { RotationMode::VelocityDirection } else { RotationMode::ViewDirection };
                command.rotation_mode_pressed = frame.desired_mode != target;
                if frame.current_mode == target && frame.desired_mode == target
                    && frame.locomotion == LocomotionState::Grounded && now - self.stage_started >= 0.3 {
                    if self.stage == Stage::VelocityMode { self.begin(Stage::ViewMode, now); }
                    else {
                        self.completed[self.index] = true;
                        self.index += 1;
                        self.furthest = 0.0;
                        if self.index % 2 == 0 { self.route = None; }
                        self.begin(Stage::Ready, now);
                    }
                }
            }
        }
        if now - self.last_progress_at >= NO_PROGRESS_SECONDS {
            self.failure = Some(format!("route={} stage={:?} no_progress_seconds={:.3} distance={distance:.3} frame={frame:?}", self.index, self.stage, now - self.last_progress_at));
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(position: Vec3) -> Frame {
        Frame { position, velocity: Vec3::ZERO, locomotion: LocomotionState::Grounded,
            stance: Stance::Standing, desired_gait: Gait::Walking,
            desired_mode: RotationMode::ViewDirection, current_mode: RotationMode::ViewDirection,
            view_yaw: 0.0 }
    }

    #[test]
    fn stairs_route_uses_loaded_mesh_bounds_and_actor_transform() {
        let collider = Collider::trimesh(vec![
            Vec3::new(0.0, -0.18, 0.0), Vec3::new(6.67, -0.18, 0.0), Vec3::new(0.0, -0.18, -0.37),
            Vec3::new(0.0, 7.92, -16.65), Vec3::new(6.67, 7.92, -16.65), Vec3::new(0.0, 7.92, -16.28),
        ], vec![[0, 1, 2], [3, 4, 5]]);
        let transform = GlobalTransform::from(Transform::from_xyz(-3.333333, 0.18, -30.0)
            .with_rotation(Quat::from_rotation_y(0.7)).with_scale(Vec3::splat(1.5)));
        let route = Route::from_collider(&collider, &transform, 0.9).unwrap();
        assert!(route.bottom.abs_diff_eq(transform.transform_point(Vec3::new(3.335, -0.18, 0.75)) + Vec3::Y * 0.9, 1e-5));
        assert!(route.top.abs_diff_eq(transform.transform_point(Vec3::new(3.335, 7.92, -16.465)) + Vec3::Y * 0.9, 1e-5));
        assert_eq!(route.endpoints(1), (route.top, route.bottom));
        assert!(Route::from_collider(&Collider::sphere(1.0), &transform, 0.9).is_none());
    }

    #[test]
    fn stairs_driver_requires_grounding_then_walk_run_sprint_and_hotkey_recovery() {
        let route = Route { bottom: Vec3::new(0.0, 0.9, 0.75), top: Vec3::new(0.0, 8.82, -16.465) };
        let mut probe = Probe { route: Some(route), ..default() };
        let mut sample = frame(route.bottom);
        sample.locomotion = LocomotionState::Air;
        assert_eq!(probe.command(0.4, sample).move_dir, Vec2::ZERO);
        assert_eq!(probe.stage, Stage::Ready);
        sample.locomotion = LocomotionState::Grounded;
        probe.command(0.5, sample);
        assert_eq!(probe.stage, Stage::Traverse);
        sample.position.z -= 1.0;
        sample.desired_gait = Gait::Running;
        assert!(probe.command(0.6, sample).walk_pressed);
        sample.position.z = route.bottom.z - 5.0;
        sample.desired_gait = Gait::Walking;
        assert!(probe.command(0.7, sample).walk_pressed);
        sample.position.z = route.bottom.z - 9.0;
        sample.desired_gait = Gait::Running;
        assert!(probe.command(0.8, sample).sprint_held);
        sample.position = route.top;
        probe.command(0.9, sample);
        assert_eq!(probe.stage, Stage::Settle);
        probe.command(1.0, sample);
        assert_eq!(probe.stage, Stage::VelocityMode);
        assert!(probe.command(1.1, sample).rotation_mode_pressed);
        sample.desired_mode = RotationMode::VelocityDirection;
        assert!(!probe.command(1.4, sample).rotation_mode_pressed);
        assert_eq!(probe.stage, Stage::VelocityMode);
        assert!(!probe.completed[0]);
        sample.current_mode = RotationMode::VelocityDirection;
        probe.command(1.5, sample);
        assert_eq!(probe.stage, Stage::ViewMode);
        assert!(probe.command(1.6, sample).rotation_mode_pressed);
        sample.current_mode = RotationMode::ViewDirection;
        sample.desired_mode = RotationMode::ViewDirection;
        probe.command(1.9, sample);
        assert!(probe.completed[0]);
        assert_eq!(probe.index, 1);
        assert!(probe.route.is_some());
        let command = probe.command(2.0, sample);
        assert!((command.look_delta.x.abs() * als_core::view::LOOK_MOUSE_RADIANS_PER_COUNT - std::f32::consts::PI).abs() < 1e-5);
        assert_eq!(command.move_dir, Vec2::ZERO);
    }

    #[test]
    fn stairs_driver_watchdog_rejects_stalls_and_locked_rotation_mode() {
        let route = Route { bottom: Vec3::new(0.0, 0.9, 0.75), top: Vec3::new(0.0, 8.82, -16.465) };
        for stage in [Stage::Traverse, Stage::VelocityMode, Stage::ViewMode] {
            let mut probe = Probe { route: Some(route), stage, ..default() };
            let mut sample = frame(route.bottom);
            sample.desired_mode = RotationMode::VelocityDirection;
            sample.current_mode = RotationMode::Aiming;
            probe.command(NO_PROGRESS_SECONDS + 0.1, sample);
            assert!(probe.failure.as_deref().unwrap().contains("no_progress_seconds="));
            assert!(!probe.completed.into_iter().any(|value| value));
        }
    }

    #[test]
    fn stairs_observer_records_actual_frame_before_qualifying_animation_windows() {
        use bevy::ecs::system::RunSystemOnce;
        use als_core::character::AlsCmcAdapterRuntime;
        let mut world = World::new();
        let mut assets = Assets::<AlsMovementParams>::default();
        let handle = assets.add(AlsMovementParams::default());
        world.insert_resource(assets);
        let character = world.spawn((AlsCharacter, Position(Vec3::Y * 0.9), LinearVelocity(Vec3::NEG_Z * 1.75),
            LocomotionState::Grounded, Stance::Standing, AlsDesiredGait(Gait::Walking), Gait::Walking,
            AlsLastFreeRotationMode(RotationMode::ViewDirection), RotationMode::ViewDirection, AlsViewYaw(0.0),
            AlsInputOverride(AlsInputCommand { move_dir: Vec2::Y, ..default() }),
            AlsActualMotion { horizontal_speed: 1.75, ..default() },
            AlsCmcAdapterRuntime { simulation_delta_seconds: 0.1, ..default() }, AlsMovementParamsHandle(handle),
            AlsStandingGraphTelemetry { actual_movement_state: Some(0), actual_movement_pose_ready: Some(true),
                actual_standing_pose_ready: Some(true), ..default() })).id();
        world.insert_resource(ManualIntegrationState { character: Some(character), phase: IntegrationPhase::Stairs,
            stairs_probe: Probe { route: Some(Route { bottom: Vec3::Y * 0.9, top: Vec3::new(0.0, 9.0, -17.215) }),
                stage: Stage::Traverse, ..default() }, ..default() });
        world.init_resource::<ManualIntegrationGateReport>();
        world.init_resource::<ManualAnimationWindowEvidence>();
        for index in 0..8 {
            world.resource_mut::<ManualIntegrationState>().game_time_seconds = index as f64 * 0.1;
            world.get_mut::<Position>(character).unwrap().0.z = -index as f32 * 0.175;
            world.run_system_once(observe).unwrap();
            if index < 5 {
                assert!(world.resource::<ManualAnimationWindowEvidence>().label.is_none());
            }
        }
        let window = world.resource::<ManualAnimationWindowEvidence>();
        assert_eq!(window.label, Some("stairs_01_up_walk"));
        assert!(window.requires_source_pose_chain && !window.source_pose_chain.is_empty());
        let report = world.resource::<ManualIntegrationGateReport>();
        let sample = report.stairs_trace.last().unwrap();
        assert!((sample.displacement_speed.unwrap() - 1.75).abs() < 1e-5);
        let json = serde_json::to_value(sample).unwrap();
        assert_eq!(json["position"][2], sample.frame.position.z);
        assert_eq!(json["desired_mode"], "ViewDirection");
        assert_eq!(json["movement_pose_ready"], true);
        world.get_mut::<LinearVelocity>(character).unwrap().0.z = -2.0;
        world.resource_mut::<ManualIntegrationState>().game_time_seconds = 0.8;
        world.run_system_once(observe).unwrap();
        let sample = world.resource::<ManualIntegrationGateReport>().stairs_trace.last().unwrap();
        assert_eq!(sample.frame.velocity.z, -2.0);
        assert_eq!(sample.animation_speed, 1.75);
    }
}

pub(super) fn drive(
    mut state: ResMut<ManualIntegrationState>,
    mut phase_evidence: ResMut<ManualRuntimePhaseEvidence>,
    stairs: Query<(&Collider, &GlobalTransform, &PlaygroundMeshColliderBuilt)>,
    mut characters: Query<(&mut AlsInputOverride, &mut Transform, &mut Position, Option<&mut Rotation>, &mut LinearVelocity,
        &mut AlsViewYaw, &mut AlsViewPitch, &Stance, &LocomotionState, &AlsDesiredGait, &AlsLastFreeRotationMode, &RotationMode), With<AlsCharacter>>,
    mut rotation_state: Query<(&AlsMovementParamsHandle, Option<&mut AlsRotationResponseState>, &mut AlsAimingYawConstraint, Option<&mut AlsRotationAnimationState>)>,
    params: Res<Assets<AlsMovementParams>>,
) {
    if state.phase != IntegrationPhase::Stairs { return; }
    let Some(character) = state.character else { return; };
    let Ok((mut input, mut transform, mut position, rotation, mut velocity, mut yaw, mut pitch, stance, locomotion, gait, desired_mode, mode)) = characters.get_mut(character) else { return; };
    let now = state.game_time_seconds;
    if state.stairs_probe.index == 6 {
        begin_phase(&mut state, IntegrationPhase::Finish);
        return;
    }
    if state.stairs_probe.route.is_none() {
        let name = MESHES[state.stairs_probe.index / 2];
        let Some((collider, global, _)) = stairs.iter().find(|(_, _, source)| source.mesh_base == name) else { return; };
        let Ok((handle, response, mut constraint, animation)) = rotation_state.get_mut(character) else { return; };
        let Some(params) = params.get(handle.id()) else { return; };
        let Some(route) = Route::from_collider(collider, global, params.crouch.standing_capsule_half_height) else { return; };
        reset_character(&mut transform, &mut position, rotation, &mut velocity, &mut yaw, &mut pitch,
            route.bottom + Vec3::Y * 0.05, world_forward_to_yaw((route.top - route.bottom).with_y(0.0)));
        reinitialize_fixture_rotation_response(response, &mut constraint, animation);
        rebase_temporal_fixture(&mut state, &mut phase_evidence, "stairs");
        state.stairs_probe.route = Some(route);
        state.stairs_probe.begin(Stage::Ready, now);
        return;
    }
    input.0 = state.stairs_probe.command(now, Frame {
        position: position.0, velocity: velocity.0, locomotion: *locomotion, stance: *stance,
        desired_gait: gait.0, desired_mode: desired_mode.0, current_mode: *mode, view_yaw: yaw.0,
    });
    if let Some(failure) = &state.stairs_probe.failure {
        error!("[MANUAL-INTEGRATION][STAIRS] {failure}");
        state.timed_out = true;
        begin_phase(&mut state, IntegrationPhase::Finish);
    }
}

const WINDOWS: [[&str; 3]; 6] = [
    ["stairs_01_up_walk", "stairs_01_up_run", "stairs_01_up_sprint"],
    ["stairs_01_down_walk", "stairs_01_down_run", "stairs_01_down_sprint"],
    ["stairs_02_up_walk", "stairs_02_up_run", "stairs_02_up_sprint"],
    ["stairs_02_down_walk", "stairs_02_down_run", "stairs_02_down_sprint"],
    ["stairs_03_up_walk", "stairs_03_up_run", "stairs_03_up_sprint"],
    ["stairs_03_down_walk", "stairs_03_down_run", "stairs_03_down_sprint"],
];

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Sample {
    route_index: usize,
    mesh: &'static str,
    stage: Stage,
    elapsed_seconds: f64,
    simulation_delta_seconds: f32,
    distance_m: f32,
    route_length_m: f32,
    input: Vec2,
    rotation_hotkey: bool,
    actual_gait: Gait,
    #[serde(flatten)]
    frame: Frame,
    expected_speed: f32,
    animation_speed: f32,
    safe_move: bool,
    displacement_speed: Option<f32>,
    movement_state: Option<u8>,
    movement_pose_ready: Option<bool>,
    standing_pose_ready: Option<bool>,
    gait_started_seconds: f64,
}

impl Sample {
    fn stable(&self) -> bool {
        self.stage == Stage::Traverse && self.input.length() >= 0.999
            && self.elapsed_seconds - self.gait_started_seconds >= 0.5
    }

    fn gait_index(&self) -> usize {
        match self.frame.desired_gait { Gait::Walking => 0, Gait::Running => 1, Gait::Sprinting => 2 }
    }

}

pub(super) fn observe(
    state: Res<ManualIntegrationState>,
    mut report: ResMut<ManualIntegrationGateReport>,
    mut window: ResMut<ManualAnimationWindowEvidence>,
    characters: Query<(&Position, &LinearVelocity, &LocomotionState, &Stance, &AlsDesiredGait, &Gait,
        &AlsLastFreeRotationMode, &RotationMode, &AlsViewYaw, &AlsInputOverride, &AlsActualMotion,
        &als_core::character::AlsCmcAdapterRuntime, &AlsMovementParamsHandle, &AlsStandingGraphTelemetry), With<AlsCharacter>>,
    params: Res<Assets<AlsMovementParams>>,
) {
    if state.stairs_probe.index >= 6 { return; }
    let Some(character) = state.character else { return; };
    let Ok((position, velocity, locomotion, stance, desired_gait, gait, desired_mode, mode, yaw, input, motion, cmc, handle, telemetry)) = characters.get(character) else { return; };
    let Some(route) = state.stairs_probe.route else { return; };
    let Some(params) = params.get(handle.id()) else { return; };
    let index = state.stairs_probe.index;
    let stage = state.stairs_probe.stage;
    let now = state.game_time_seconds;
    let (start, end) = route.endpoints(index);
    let direction = (end - start).with_y(0.0).normalize();
    let previous = report.stairs_trace.last().filter(|sample| sample.route_index == index);
    let sample = Sample {
        route_index: index, mesh: MESHES[index / 2], stage, elapsed_seconds: now,
        simulation_delta_seconds: cmc.simulation_delta_seconds,
        distance_m: (position.0 - start).dot(direction), route_length_m: (end - start).with_y(0.0).length(),
        input: input.0.move_dir, rotation_hotkey: input.0.rotation_mode_pressed, actual_gait: *gait,
        frame: Frame { position: position.0, velocity: velocity.0, locomotion: *locomotion, stance: *stance,
            desired_gait: desired_gait.0, desired_mode: desired_mode.0, current_mode: *mode, view_yaw: yaw.0 },
        expected_speed: als_core::motion::gait_max_speed(params, stance, &desired_gait.0),
        animation_speed: motion.horizontal_speed, safe_move: cmc.grounded_move_start.is_some(),
        displacement_speed: previous.filter(|_| cmc.simulation_delta_seconds > 0.0)
            .map(|previous| (position.0 - previous.frame.position).with_y(0.0).length() / cmc.simulation_delta_seconds),
        movement_state: telemetry.actual_movement_state, movement_pose_ready: telemetry.actual_movement_pose_ready,
        standing_pose_ready: telemetry.actual_standing_pose_ready,
        gait_started_seconds: previous.filter(|sample| sample.frame.desired_gait == desired_gait.0
            && sample.stage == Stage::Traverse && stage == Stage::Traverse)
            .map_or(now, |sample| sample.gait_started_seconds),
    };
    if sample.stable() {
        *window = ManualAnimationWindowEvidence {
            label: Some(WINDOWS[index][sample.gait_index()]), settled: true,
            capture_source_window: true, capture_every_frame: true,
            requires_pre_rig_motion: true, requires_final_pose_motion: true,
            requires_non_reference_root_pose: true, requires_source_pose_chain: true,
            source_node: Some("AB_Als_Standing/AnimGraph"), source_state: Some("Grounded stair traversal"),
            source_pose_chain: generated_standing_source_pose_chain(),
            minimum_pre_rig_rotation_span_radians: 0.05, minimum_final_rotation_span_radians: 0.05,
            minimum_observation_seconds: 0.12,
            expected_runtime_state: Some(0), observed_runtime_state: telemetry.actual_movement_state,
            expected_direction: Some("Forward"), observed_direction: telemetry.actual_movement_state
                .and_then(movement_direction_from_fsm_index).map(movement_direction_name),
            observed_parent_runtime_state: telemetry.actual_standing_state,
            observed_parent_transition_active: telemetry.actual_standing_transition_active,
            ..default()
        };
    }
    report.stairs_trace.push(sample);
}
