use avian3d::prelude::{
    Collider, LinearVelocity, LockedAxes, PhysicsSystems, Position, RigidBody, SweptCcd,
};
use bevy::asset::Handle;
use bevy::prelude::*;
use bevy_tnua::TnuaController;
use bevy_tnua::controller::TnuaConfig;
use bevy_tnua_avian3d::TnuaAvian3dSensorShape;

use crate::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use crate::input::AlsInputCommand;
use crate::params::AlsMovementParams;
use crate::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use crate::transition::{AlsDesiredStance, AlsStateRequest};

#[derive(Component, Debug, Default, Clone)]
pub struct AlsMovementParamsHandle(pub Handle<AlsMovementParams>);

impl std::ops::Deref for AlsMovementParamsHandle {
    type Target = Handle<AlsMovementParams>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
#[require(crate::motion::AlsViewYaw)]
#[require(AlsCrushRecovery)]
#[require(crate::gait::AlsDesiredGait)]
#[require(crate::gait::AlsMaxAllowedGait)]
#[require(crate::gait::AlsGaitAmount)]
#[require(crate::dynamics::AlsGroundedDynamics)]
#[require(AlsCmcAdapterRuntime)]
#[require(crate::motion::AlsAimingYawConstraint)]
#[require(crate::foot_ik::AlsFootLockSettings)]
#[require(crate::foot_ik::AlsDynamicTransitionState)]
#[require(AlsDesiredStance)]
pub struct AlsCharacter;

pub const ALS_CHARACTER_CAPSULE_HALF_HEIGHT: f32 = 0.9;
pub const ALS_CHARACTER_CAPSULE_RADIUS: f32 = 0.3;
pub const ALS_CHARACTER_FLOOR_CLEARANCE: f32 = 0.019;
pub const ALS_CHARACTER_TNUA_FLOAT_EPSILON: f32 = 0.005;
const CRUSH_RECOVERY_SKIN: f32 = 0.001;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsCmcAdapterRuntime {
    pub support_radius: f32,
    pub fixed_delta_seconds: f32,
    pub simulation_substeps: u32,
    pub min_analog_speed_applied: bool,
}

impl Default for AlsCmcAdapterRuntime {
    fn default() -> Self {
        Self {
            support_radius: ALS_CHARACTER_CAPSULE_RADIUS - 0.15,
            fixed_delta_seconds: 0.0,
            simulation_substeps: 0,
            min_analog_speed_applied: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AlsCrushRecoveryFloor {
    pub half_extents: Vec3,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlsCrushRecovery {
    pub corrections: u32,
    pub max_penetration: f32,
}

pub fn als_character_capsule(half_height: f32, radius: f32) -> Collider {
    Collider::capsule(radius, (2.0 * (half_height - radius)).max(0.0))
}

pub fn als_character_ground_sensor(
    capsule_radius: f32,
    perch_radius_threshold: f32,
    perch_additional_height: f32,
) -> TnuaAvian3dSensorShape {
    TnuaAvian3dSensorShape(Collider::cylinder(
        (capsule_radius - perch_radius_threshold).max(0.001),
        (perch_additional_height * 2.0).max(0.0),
    ))
}

pub struct AlsCharacterPlugin;

impl Plugin for AlsCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<AlsMovementParams>()
            .init_asset::<AlsControlSchemeConfig>()
            .add_systems(
                FixedPostUpdate,
                character_crush_recovery_system.in_set(PhysicsSystems::Last),
            )
            .add_systems(Update, character_sensor_shape_sync_system);
    }
}

fn character_half_height(params: &AlsMovementParams, stance: Stance) -> f32 {
    match stance {
        Stance::Standing => params.crouch.standing_capsule_half_height,
        Stance::Crouching => params.crouch.crouched_capsule_half_height,
    }
}

fn vertical_intervals_overlap(
    first_bottom: f32,
    first_top: f32,
    second_bottom: f32,
    second_top: f32,
) -> bool {
    first_bottom < second_top && first_top > second_bottom
}

fn crush_recovery_floor_vertical_bounds(transform: &Transform, half_extents: Vec3) -> (f32, f32) {
    let matrix = transform.to_matrix();
    let center = matrix.transform_point3(Vec3::ZERO).y;
    let vertical_half_extent = matrix.transform_vector3(Vec3::X * half_extents.x).y.abs()
        + matrix.transform_vector3(Vec3::Y * half_extents.y).y.abs()
        + matrix.transform_vector3(Vec3::Z * half_extents.z).y.abs();
    (center - vertical_half_extent, center + vertical_half_extent)
}

fn character_crush_recovery_system(
    floors: Query<(&GlobalTransform, &AlsCrushRecoveryFloor)>,
    params_assets: Res<Assets<AlsMovementParams>>,
    mut characters: Query<
        (
            &mut Position,
            &mut Transform,
            &mut LinearVelocity,
            &Stance,
            &AlsMovementParamsHandle,
            &mut AlsCrushRecovery,
        ),
        With<AlsCharacter>,
    >,
) {
    for (mut position, mut transform, mut velocity, stance, params_handle, mut recovery) in
        &mut characters
    {
        let half_height = params_assets
            .get(params_handle.0.id())
            .map(|params| character_half_height(params, *stance))
            .unwrap_or(ALS_CHARACTER_CAPSULE_HALF_HEIGHT);
        let capsule_bottom = position.0.y - half_height;
        let capsule_top = position.0.y + half_height;
        let mut recovery_top = None;
        for (floor_transform, floor) in &floors {
            let floor_transform = floor_transform.compute_transform();
            let floor_matrix = floor_transform.to_matrix();
            let local_position = floor_matrix.inverse().transform_point3(position.0);
            if local_position.x.abs() > floor.half_extents.x
                || local_position.z.abs() > floor.half_extents.z
            {
                continue;
            }
            let (floor_bottom, top) =
                crush_recovery_floor_vertical_bounds(&floor_transform, floor.half_extents);
            if vertical_intervals_overlap(capsule_bottom, capsule_top, floor_bottom, top) {
                recovery_top = Some(recovery_top.map_or(top, |current: f32| current.max(top)));
            }
        }
        let Some(top) = recovery_top else {
            continue;
        };
        let penetration = top - capsule_bottom;
        if penetration <= 0.0 {
            continue;
        }
        let corrected_y = top + half_height + CRUSH_RECOVERY_SKIN;
        position.0.y = corrected_y;
        transform.translation.y = corrected_y;
        velocity.0.y = velocity.0.y.max(0.0);
        recovery.corrections += 1;
        recovery.max_penetration = recovery.max_penetration.max(penetration);
    }
}

fn character_sensor_shape_sync_system(
    params_assets: Res<Assets<AlsMovementParams>>,
    mut characters: Query<
        (
            &AlsMovementParamsHandle,
            &mut TnuaAvian3dSensorShape,
            &mut AlsCmcAdapterRuntime,
        ),
        With<AlsCharacter>,
    >,
) {
    for (params_handle, mut sensor, mut runtime) in &mut characters {
        let Some(params) = params_assets.get(params_handle.id()) else {
            continue;
        };
        let radius = params.crouch.standing_capsule_radius;
        let perch = params.cmc.perch_radius_threshold;
        sensor.0 = als_character_ground_sensor(radius, perch, params.cmc.perch_additional_height).0;
        runtime.support_radius = (radius - perch).max(0.001);
    }
}

#[derive(Bundle)]
pub struct AlsCharacterBundle {
    pub character: AlsCharacter,
    pub transform: Transform,
    pub rigid_body: RigidBody,
    pub collider: Collider,
    pub sensor_shape: TnuaAvian3dSensorShape,
    pub locked_axes: LockedAxes,
    pub swept_ccd: SweptCcd,
    pub controller: TnuaController<AlsControlScheme>,
    pub tnua_config: TnuaConfig<AlsControlScheme>,
    pub params: AlsMovementParamsHandle,
    pub input_command: AlsInputCommand,
    pub locomotion_state: LocomotionState,
    pub rotation_mode: RotationMode,
    pub stance: Stance,
    pub desired_stance: AlsDesiredStance,
    pub gait: Gait,
    pub desired_gait: crate::gait::AlsDesiredGait,
    pub max_allowed_gait: crate::gait::AlsMaxAllowedGait,
    pub gait_amount: crate::gait::AlsGaitAmount,
    pub grounded_dynamics: crate::dynamics::AlsGroundedDynamics,
    pub cmc_adapter_runtime: AlsCmcAdapterRuntime,
    pub aiming_yaw_constraint: crate::motion::AlsAimingYawConstraint,
    pub overlay_state: OverlayState,
    pub request: AlsStateRequest,
}

impl AlsCharacterBundle {
    pub fn new(
        params: Handle<AlsMovementParams>,
        tnua_config: Handle<AlsControlSchemeConfig>,
    ) -> Self {
        Self {
            character: AlsCharacter,
            transform: Transform::default(),
            rigid_body: RigidBody::Dynamic,
            collider: als_character_capsule(
                ALS_CHARACTER_CAPSULE_HALF_HEIGHT,
                ALS_CHARACTER_CAPSULE_RADIUS,
            ),
            sensor_shape: als_character_ground_sensor(ALS_CHARACTER_CAPSULE_RADIUS, 0.15, 0.0),
            locked_axes: LockedAxes::ROTATION_LOCKED,
            swept_ccd: SweptCcd::LINEAR,
            controller: TnuaController::default(),
            tnua_config: TnuaConfig(tnua_config),
            params: AlsMovementParamsHandle(params),
            input_command: AlsInputCommand::default(),
            locomotion_state: LocomotionState::Grounded,
            rotation_mode: RotationMode::ViewDirection,
            stance: Stance::Standing,
            desired_stance: AlsDesiredStance(Stance::Standing),
            gait: Gait::Running,
            desired_gait: crate::gait::AlsDesiredGait(Gait::Running),
            max_allowed_gait: crate::gait::AlsMaxAllowedGait(Gait::Running),
            gait_amount: crate::gait::AlsGaitAmount::default(),
            grounded_dynamics: crate::dynamics::AlsGroundedDynamics::default(),
            cmc_adapter_runtime: AlsCmcAdapterRuntime::default(),
            aiming_yaw_constraint: crate::motion::AlsAimingYawConstraint::default(),
            overlay_state: OverlayState::default(),
            request: AlsStateRequest::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crush_recovery_test_app() -> (App, Handle<AlsMovementParams>) {
        let mut app = App::new();
        app.insert_resource(Assets::<AlsMovementParams>::default())
            .add_systems(Update, character_crush_recovery_system);
        let params = app
            .world_mut()
            .resource_mut::<Assets<AlsMovementParams>>()
            .add(AlsMovementParams::default());
        (app, params)
    }

    fn spawn_crush_recovery_character(
        app: &mut App,
        params: Handle<AlsMovementParams>,
        center: Vec3,
        vertical_velocity: f32,
    ) -> Entity {
        app.world_mut()
            .spawn((
                AlsCharacter,
                Position(center),
                Transform::from_translation(center),
                LinearVelocity(Vec3::Y * vertical_velocity),
                Stance::Standing,
                AlsMovementParamsHandle(params),
            ))
            .id()
    }

    fn spawn_crush_recovery_floor(app: &mut App, center: Vec3, half_extents: Vec3) {
        app.world_mut().spawn((
            GlobalTransform::from(Transform::from_translation(center)),
            AlsCrushRecoveryFloor { half_extents },
        ));
    }

    #[test]
    fn bundle_initial_state_matches_b_als_character() {
        let bundle = AlsCharacterBundle::new(
            Handle::<AlsMovementParams>::default(),
            Handle::<AlsControlSchemeConfig>::default(),
        );
        assert_eq!(bundle.rotation_mode, RotationMode::ViewDirection);
        assert_eq!(bundle.gait, Gait::Running);
        assert_eq!(bundle.desired_gait.0, Gait::Running);
        assert_eq!(bundle.max_allowed_gait.0, Gait::Running);
        assert!((bundle.grounded_dynamics.0.acceleration - 20.0).abs() < 1e-6);
        assert!((bundle.cmc_adapter_runtime.support_radius - 0.15).abs() < 1e-6);
        assert_eq!(bundle.aiming_yaw_constraint.active_limit_degrees, 180.0);
        assert_eq!(bundle.stance, Stance::Standing);
        assert_eq!(bundle.desired_stance.0, Stance::Standing);
    }

    #[test]
    fn crush_recovery_ignores_an_overhead_floor_without_vertical_overlap() {
        let (mut app, params) = crush_recovery_test_app();
        let character =
            spawn_crush_recovery_character(&mut app, params, Vec3::new(0.0, 1.0, 0.0), -1.0);
        spawn_crush_recovery_floor(&mut app, Vec3::new(0.0, 3.0, 0.0), Vec3::new(2.0, 0.2, 2.0));

        app.update();

        let entity = app.world().entity(character);
        let position = entity.get::<Position>().unwrap();
        let transform = entity.get::<Transform>().unwrap();
        let velocity = entity.get::<LinearVelocity>().unwrap();
        let recovery = entity.get::<AlsCrushRecovery>().unwrap();
        assert_eq!(position.0, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(transform.translation, position.0);
        assert_eq!(velocity.0.y, -1.0);
        assert_eq!(recovery.corrections, 0);
    }

    #[test]
    fn crush_recovery_resolves_a_vertically_overlapping_floor() {
        let (mut app, params) = crush_recovery_test_app();
        let character =
            spawn_crush_recovery_character(&mut app, params, Vec3::new(0.0, 3.0, 0.0), -1.0);
        spawn_crush_recovery_floor(&mut app, Vec3::new(0.0, 3.0, 0.0), Vec3::new(2.0, 0.2, 2.0));

        app.update();

        let entity = app.world().entity(character);
        let position = entity.get::<Position>().unwrap();
        let transform = entity.get::<Transform>().unwrap();
        let velocity = entity.get::<LinearVelocity>().unwrap();
        let recovery = entity.get::<AlsCrushRecovery>().unwrap();
        let expected_y = 3.2 + ALS_CHARACTER_CAPSULE_HALF_HEIGHT + CRUSH_RECOVERY_SKIN;
        assert!((position.0.y - expected_y).abs() < 1e-6);
        assert_eq!(transform.translation, position.0);
        assert_eq!(velocity.0.y, 0.0);
        assert_eq!(recovery.corrections, 1);
        assert!((recovery.max_penetration - 1.1).abs() < 1e-6);
    }
}
