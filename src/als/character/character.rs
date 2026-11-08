use avian3d::prelude::{
    SimpleCollider, Collider, ColliderDensity, ColliderMassProperties, CollisionLayers, ComputeMassProperties3d, ContactGraph, Sensor, LinearVelocity, LockedAxes, PhysicsLayer, PhysicsSchedule,
    PhysicsStepSystems, PhysicsSystems, Position, RigidBody,
    SpeculativeMargin,
};
use bevy::asset::Handle;
use bevy::prelude::*;
use bevy_tnua::TnuaController;
use bevy_tnua::controller::TnuaConfig;
use bevy_tnua_avian3d::TnuaAvian3dSensorShape;

use crate::als::control_scheme::{AlsControlScheme, AlsControlSchemeConfig};
use crate::als::input::AlsInputCommand;
use crate::als::params::AlsMovementParams;
use crate::als::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};
use crate::als::transition::{AlsDesiredStance, AlsStateRequest};

#[derive(Component, Debug, Default, Clone)]
pub struct AlsMovementParamsHandle(pub Handle<AlsMovementParams>);

impl std::ops::Deref for AlsMovementParamsHandle {
    type Target = Handle<AlsMovementParams>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
#[require(AlsInitialMovementModePending)]
#[require(avian3d::prelude::CollisionEventsEnabled)]
#[require(crate::als::motion::AlsViewYaw)]
#[require(AlsCrushRecovery)]
#[require(crate::als::gait::AlsDesiredGait)]
#[require(crate::als::gait::AlsMaxAllowedGait)]
#[require(crate::als::gait::AlsGaitAmount)]
#[require(crate::als::dynamics::AlsGroundedDynamics)]
#[require(crate::als::dynamics::AlsMovementBase)]
#[require(crate::als::dynamics::AlsLandingBrakingFriction)]
#[require(AlsCmcAdapterRuntime)]
#[require(crate::als::motion::AlsAimingYawConstraint)]
#[require(crate::als::motion::AlsActualVelocity)]
#[require(crate::als::motion::AlsCharacterDebug)]
#[require(crate::als::foot_ik::AlsFootLockSettings)]
#[require(crate::als::foot_ik::AlsDynamicTransitionState)]
#[require(AlsDesiredStance)]
#[require(crate::als::motion::AlsCrouchTransaction)]
#[require(AlsCollisionLayerOwnership)]
#[require(AlsEffectiveGravity)]
#[require(crate::extensions::SourceCollisionPawn)]
#[require(AlsPhysicalCapsuleScale)]
#[require(crate::als::mantle::AlsMantleTargetPermission = crate::als::mantle::AlsMantleTargetPermission { allows_step_up: false })]
pub struct AlsCharacter;

#[derive(Component, Debug, Default)]
struct AlsPhysicalCapsuleScale {
    dimensions: Option<(Vec3, f32, f32)>,
    minimum_axis_half: f32,
}

#[derive(Component, Debug, Default)]
pub(crate) struct AlsInitialMovementModePending;

#[derive(Component, Debug, Default)]
pub struct AlsBirthAnimationPending;

pub const ALS_CHARACTER_CAPSULE_HALF_HEIGHT: f32 = 0.9;
pub const ALS_CHARACTER_CAPSULE_RADIUS: f32 = 0.3;
pub const ALS_CHARACTER_FLOOR_CLEARANCE: f32 = 0.019;
pub const ALS_CHARACTER_TNUA_FLOAT_EPSILON: f32 = 0.005;
const CRUSH_RECOVERY_SKIN: f32 = 0.001;

#[derive(PhysicsLayer, Default, Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum AlsCollisionLayer {
    #[default]
    Default,
    Visibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsCollisionLayerOwner {
    Mantle,
    Ragdoll,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsCollisionLayerOwnership {
    pub mantle: bool,
    pub ragdoll: bool,
    original: Option<CollisionLayers>,
    had_original: bool,
}

impl AlsCollisionLayerOwnership {
    pub fn active(&self) -> bool {
        self.mantle || self.ragdoll
    }

    pub fn original_layers(&self) -> CollisionLayers {
        self.original.unwrap_or_else(|| CollisionLayers::new(
            AlsCollisionLayer::Default,
            AlsCollisionLayer::Default,
        ))
    }

    pub fn set_owner(
        &mut self,
        owner: AlsCollisionLayerOwner,
        current: Option<CollisionLayers>,
        active: bool,
    ) -> Option<Option<CollisionLayers>> {
        let was_active = self.active();
        if active && !was_active {
            self.original = current;
            self.had_original = current.is_some();
        }
        match owner {
            AlsCollisionLayerOwner::Mantle => self.mantle = active,
            AlsCollisionLayerOwner::Ragdoll => self.ragdoll = active,
        }
        if self.active() {
            (current != Some(CollisionLayers::NONE)).then_some(Some(CollisionLayers::NONE))
        } else if was_active {
            let restored = self.had_original.then_some(self.original).flatten();
            self.original = None;
            self.had_original = false;
            Some(restored)
        } else {
            None
        }
    }
}

pub fn set_collision_layer_owner(
    commands: &mut Commands,
    entity: Entity,
    ownership: &mut AlsCollisionLayerOwnership,
    current: Option<CollisionLayers>,
    owner: AlsCollisionLayerOwner,
    active: bool,
) {
    let Some(layers) = ownership.set_owner(owner, current, active) else {
        return;
    };
    match layers {
        Some(layers) => {
            commands.entity(entity).insert(layers);
        }
        None => {
            commands.entity(entity).remove::<CollisionLayers>();
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsEffectiveGravity {
    pub value: Vec3,
    pub global: Vec3,
    pub gravity_scale: f32,
    pub tnua_gravity: Vec3,
}

impl Default for AlsEffectiveGravity {
    fn default() -> Self {
        Self {
            value: Vec3::ZERO,
            global: Vec3::ZERO,
            gravity_scale: 1.0,
            tnua_gravity: Vec3::ZERO,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsCmcAdapterRuntime {
    pub support_radius: f32,
    pub simulation_delta_seconds: f32,
    pub simulation_substeps: u32,
    pub grounded_max_speed: f32,
    pub controlled_input_max_acceleration: Option<f32>,
    pub min_analog_speed_applied: bool,
    pub grounded_move_start: Option<Vec3>,
    pub grounded_movement_delta: Option<Vec3>,
    pub grounded_collision_velocity: Option<Vec3>,
    pub has_animation_root_velocity: bool,
    pub airborne_sweep_contact: bool,
    pub post_collision_mode: Option<LocomotionState>,
    pub mantle_exit_grounded: bool,
    pub deferred_base_follow: bool,
    pub random_seed: u32,
}

impl Default for AlsCmcAdapterRuntime {
    fn default() -> Self {
        Self {
            support_radius: ALS_CHARACTER_CAPSULE_RADIUS - 0.15,
            simulation_delta_seconds: 0.0,
            simulation_substeps: 0,
            grounded_max_speed: 3.75,
            controlled_input_max_acceleration: None,
            min_analog_speed_applied: false,
            grounded_move_start: None,
            grounded_movement_delta: None,
            grounded_collision_velocity: None,
            has_animation_root_velocity: false,
            airborne_sweep_contact: false,
            post_collision_mode: None,
            mantle_exit_grounded: false,
            deferred_base_follow: false,
            random_seed: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |time| time.as_nanos() as u32),
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

pub(crate) fn physical_capsule_dimensions(half: f32, radius: f32, scale: Vec3, minimum_axis_half: f32) -> (f32, f32) {
    let scale = scale.abs();
    let scaled_half = (half * scale.y).max(0.001);
    let scaled_radius = (radius * scale.x.max(scale.z)).clamp(0.001, scaled_half);
    (scaled_radius, (scaled_half - scaled_radius).max(minimum_axis_half))
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
            .init_resource::<crate::als::dynamics::AlsDefaultPhysicsVolume>()
            .add_systems(avian3d::prelude::SubstepSchedule,
                crate::als::dynamics::limit_falling_velocity
                    .after(avian3d::dynamics::integrator::IntegrationSystems::Velocity)
                    .before(avian3d::dynamics::solver::schedule::SubstepSolverSystems::WarmStart))
            .add_systems(
                PostUpdate,
                character_crush_recovery_system.in_set(PhysicsSystems::Last),
            )
            .add_systems(Update, sync_physical_capsules
                .after(crate::als::motion::capsule_reshape_system)
                .before(crate::als::motion::cmc_movement_transaction_system)
                .after(avian3d::dynamics::rigid_body::mass_properties::MassPropertySystems::UpdateColliderMassProperties)
                .before(avian3d::dynamics::rigid_body::mass_properties::MassPropertySystems::QueueRecomputation))
            .add_systems(PostUpdate, sync_physical_capsules
                .in_set(PhysicsSystems::Prepare)
                .after(avian3d::dynamics::rigid_body::mass_properties::MassPropertySystems::UpdateColliderMassProperties)
                .before(avian3d::dynamics::rigid_body::mass_properties::MassPropertySystems::QueueRecomputation))
            .add_systems(FixedPostUpdate, sync_physical_capsules
                .in_set(PhysicsSystems::Prepare)
                .after(avian3d::dynamics::rigid_body::mass_properties::MassPropertySystems::UpdateColliderMassProperties)
                .before(avian3d::dynamics::rigid_body::mass_properties::MassPropertySystems::QueueRecomputation))
            .add_systems(Update, character_sensor_shape_sync_system)
            .add_systems(PhysicsSchedule, character_contact_friction_system
                .after(PhysicsStepSystems::NarrowPhase).before(PhysicsStepSystems::Solver))
            .add_systems(PhysicsSchedule, sync_child_collider_poses
                .in_set(PhysicsStepSystems::Last));
    }
}

fn sync_physical_capsules(
    params_assets: Res<Assets<AlsMovementParams>>,
    transforms: bevy::transform::helper::TransformHelper,
    mut characters: Query<(Entity, &Stance, &AlsMovementParamsHandle, &mut Collider,
        &mut AlsPhysicalCapsuleScale, Option<&ColliderDensity>, Option<&mut ColliderMassProperties>, Has<Sensor>), With<AlsCharacter>>,
) {
    for (entity, stance, handle, mut collider, mut previous, density, mass, sensor) in &mut characters {
        let Some(params) = params_assets.get(handle.id()) else { continue };
        let Ok(world) = transforms.compute_global_transform(entity) else { continue };
        let scale = world.to_scale_rotation_translation().0.abs();
        let half = if *stance == Stance::Crouching { params.crouch.actual_crouched_half_height() }
            else { params.crouch.standing_capsule_half_height };
        let radius = params.crouch.standing_capsule_radius;
        let dimensions = (scale, half, radius);
        if previous.dimensions != Some(dimensions) {
            previous.minimum_axis_half = if previous.dimensions.is_some() { 0.000001 } else { 0.0005 };
            previous.dimensions = Some(dimensions);
        }
        let (scaled_radius, axis_half) = physical_capsule_dimensions(half, radius, scale, previous.minimum_axis_half);
        let rebuild = collider.shape().as_capsule().is_none_or(|capsule|
            capsule.radius != scaled_radius || capsule.segment.a.y != axis_half || capsule.segment.b.y != -axis_half);
        let rescale = collider.scale() != Vec3::ONE;
        if rebuild { *collider = Collider::capsule(scaled_radius, axis_half * 2.0); }
        else if rescale { collider.set_scale(Vec3::ONE, 10); }
        if (rebuild || rescale) && !sensor && let (Some(density), Some(mut mass)) = (density, mass) {
            *mass = ColliderMassProperties::from(collider.mass_properties(density.0));
        }
    }
}

fn sync_child_collider_poses(
    mut colliders: Query<(&avian3d::prelude::ColliderTransform, &avian3d::prelude::ColliderOf,
        &mut Position, &mut avian3d::prelude::Rotation), Without<RigidBody>>,
    bodies: Query<(&Position, &avian3d::prelude::Rotation), With<RigidBody>>,
) {
    for (local, owner, mut position, mut rotation) in &mut colliders {
        let Ok((body_position, body_rotation)) = bodies.get(owner.body) else { continue; };
        position.0 = body_position.0 + body_rotation.0 * local.translation;
        rotation.0 = (body_rotation.0 * local.rotation.0).normalize();
    }
}

fn character_contact_friction_system(
    characters: Query<(), With<AlsCharacter>>,
    mut contacts: ResMut<ContactGraph>,
) {
    for pair in contacts.iter_active_mut() {
        if characters.contains(pair.body1.unwrap_or(pair.collider1))
            || characters.contains(pair.body2.unwrap_or(pair.collider2)) {
            for manifold in &mut pair.manifolds {
                manifold.friction = 0.0;
            }
        }
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
    floors: Query<(Entity, &GlobalTransform, &AlsCrushRecoveryFloor, &Collider)>,
    candidates: Query<crate::als::motion::CharacterCollisionCandidates>,
    mut characters: Query<
        (
            Entity,
            &mut Position,
            &mut Transform,
            &mut LinearVelocity,
            &Collider,
            &mut AlsCrushRecovery,
        ),
        (With<AlsCharacter>, Without<AlsBirthAnimationPending>),
    >,
) {
    for (entity, mut position, mut transform, mut velocity, capsule, mut recovery) in
        &mut characters
    {
        let half_height = capsule.shape_scaled().compute_local_aabb().half_extents().y;
        let capsule_bottom = position.0.y - half_height;
        let capsule_top = position.0.y + half_height;
        let mut recovery_top = None;
        for (floor_entity, floor_transform, floor, floor_collider) in &floors {
            if !crate::als::motion::character_blocks_collider(entity, floor_entity, &candidates) { continue; }
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
                let mut manifolds = Vec::new();
                capsule.contact_manifolds(floor_collider, position.0, transform.rotation,
                    floor_transform.translation, floor_transform.rotation, 0.0, &mut manifolds);
                if manifolds.iter().any(|manifold| manifold.points.iter().any(|point| point.penetration > 0.0)) {
                    recovery_top = Some(recovery_top.map_or(top, |current: f32| current.max(top)));
                }
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
    pub speculative_margin: SpeculativeMargin,
    pub controller: TnuaController<AlsControlScheme>,
    pub tnua_config: TnuaConfig<AlsControlScheme>,
    pub params: AlsMovementParamsHandle,
    pub input_command: AlsInputCommand,
    pub locomotion_state: LocomotionState,
    pub rotation_mode: RotationMode,
    pub stance: Stance,
    pub desired_stance: AlsDesiredStance,
    pub gait: Gait,
    pub desired_gait: crate::als::gait::AlsDesiredGait,
    pub max_allowed_gait: crate::als::gait::AlsMaxAllowedGait,
    pub gait_amount: crate::als::gait::AlsGaitAmount,
    pub grounded_dynamics: crate::als::dynamics::AlsGroundedDynamics,
    pub cmc_adapter_runtime: AlsCmcAdapterRuntime,
    pub aiming_yaw_constraint: crate::als::motion::AlsAimingYawConstraint,
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
            speculative_margin: SpeculativeMargin::MAX,
            controller: TnuaController::default(),
            tnua_config: TnuaConfig(tnua_config),
            params: AlsMovementParamsHandle(params),
            input_command: AlsInputCommand::default(),
            locomotion_state: LocomotionState::Grounded,
            rotation_mode: RotationMode::ViewDirection,
            stance: Stance::Standing,
            desired_stance: AlsDesiredStance(Stance::Standing),
            gait: Gait::Running,
            desired_gait: crate::als::gait::AlsDesiredGait(Gait::Running),
            max_allowed_gait: crate::als::gait::AlsMaxAllowedGait(Gait::Running),
            gait_amount: crate::als::gait::AlsGaitAmount::default(),
            grounded_dynamics: crate::als::dynamics::AlsGroundedDynamics::default(),
            cmc_adapter_runtime: AlsCmcAdapterRuntime::default(),
            aiming_yaw_constraint: crate::als::motion::AlsAimingYawConstraint::default(),
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
                als_character_capsule(ALS_CHARACTER_CAPSULE_HALF_HEIGHT, ALS_CHARACTER_CAPSULE_RADIUS),
                AlsMovementParamsHandle(params),
            ))
            .id()
    }

    fn spawn_crush_recovery_floor(app: &mut App, center: Vec3, half_extents: Vec3) {
        app.world_mut().spawn((
            GlobalTransform::from(Transform::from_translation(center)),
            AlsCrushRecoveryFloor { half_extents },
            Collider::cuboid(half_extents.x * 2.0, half_extents.y * 2.0, half_extents.z * 2.0),
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
    fn character_capsules_default_to_no_step_and_keep_explicit_overrides() {
        let mut world = World::new();
        let character = world.spawn(AlsCharacter).id();
        let override_character = world.spawn((AlsCharacter,
            crate::als::mantle::AlsMantleTargetPermission { allows_step_up: true })).id();
        let prop = world.spawn_empty().id();
        assert!(!world.get::<crate::als::mantle::AlsMantleTargetPermission>(character).unwrap().allows_step_up);
        assert!(world.get::<crate::als::mantle::AlsMantleTargetPermission>(override_character).unwrap().allows_step_up);
        assert!(world.get::<crate::als::mantle::AlsMantleTargetPermission>(prop).is_none());
    }

    #[test]
    fn physical_capsule_keeps_the_source_circle_for_parent_and_direct_scale() {
        use avian3d::prelude::{PhysicsPlugins, ShapeCastConfig, SpatialQueryFilter};
        use bevy::{asset::AssetPlugin, ecs::system::RunSystemOnce};
        for parented in [false, true] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
                PhysicsPlugins::new(PostUpdate), AlsCharacterPlugin));
            app.finish();
            let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
            let parent = parented.then(|| app.world_mut().spawn(Transform::from_xyz(0.0, 3.0, 0.0)).id());
            let actor = app.world_mut().spawn((AlsCharacter, Stance::Standing, AlsMovementParamsHandle(params),
                RigidBody::Kinematic, als_character_capsule(0.9, 0.3),
                if parented { Transform::IDENTITY } else { Transform::from_xyz(0.0, 3.0, 0.0) })).id();
            if let Some(parent) = parent { app.world_mut().entity_mut(actor).insert(ChildOf(parent)); }
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(4.0, 4.0, 0.1),
                Transform::from_xyz(0.0, 3.0, -1.05)));
            for (scale, stance, expected) in [
                (Vec3::new(2.0, 1.0, 1.0), Stance::Standing, Vec3::new(0.6, 0.9, 0.6)),
                (Vec3::new(1.0, 1.0, 2.0), Stance::Standing, Vec3::new(0.6, 0.9, 0.6)),
                (Vec3::splat(2.0), Stance::Standing, Vec3::new(0.6, 1.8, 0.6)),
                (Vec3::new(2.0, 0.25, 1.0), Stance::Crouching, Vec3::splat(0.14)),
            ] {
                app.world_mut().get_mut::<Transform>(parent.unwrap_or(actor)).unwrap().scale = scale;
                *app.world_mut().get_mut::<Stance>(actor).unwrap() = stance;
                app.update();
                app.update();
                let collider = app.world().get::<Collider>(actor).unwrap();
                let extent = collider.shape_scaled().compute_local_aabb().half_extents();
                assert!(extent.abs_diff_eq(expected, 2e-5), "parented={parented} scale={scale:?} extent={extent:?}");
                let density = app.world().get::<ColliderDensity>(actor).unwrap().0;
                assert_eq!(*app.world().get::<ColliderMassProperties>(actor).unwrap(),
                    ColliderMassProperties::from_shape(collider, density));
                if scale == Vec3::new(2.0, 1.0, 1.0) {
                    let capsule = collider.clone();
                    let origin = app.world().get::<Position>(actor).unwrap().0;
                    let hit = app.world_mut().run_system_once(move |spatial: crate::extensions::RagdollSpatialQuery| {
                        spatial.cast_shape(&capsule, origin, Quat::IDENTITY, Dir3::NEG_Z,
                            &ShapeCastConfig::from_max_distance(2.0), &SpatialQueryFilter::from_excluded_entities([actor]))
                    }).unwrap().unwrap();
                    assert!((hit.distance - 0.4).abs() < 1e-4, "parented={parented} hit={hit:?}");
                }
            }
        }
    }

    #[test]
    fn clamped_physical_capsule_uses_source_creation_and_scale_update_lengths() {
        use avian3d::prelude::PhysicsPlugins;
        use bevy::asset::AssetPlugin;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin,
            PhysicsPlugins::new(PostUpdate), AlsCharacterPlugin));
        app.finish();
        let params = app.world_mut().resource_mut::<Assets<AlsMovementParams>>().add(AlsMovementParams::default());
        let actor = app.world_mut().spawn((AlsCharacter, Stance::Standing, AlsMovementParamsHandle(params),
            RigidBody::Kinematic, als_character_capsule(0.9, 0.3),
            Transform::from_scale(Vec3::new(2.0, 0.25, 1.0)))).id();
        app.update();
        let initial = app.world().get::<Collider>(actor).unwrap().shape_scaled().as_capsule().unwrap();
        assert!((initial.segment.a.y - 0.0005).abs() < 1e-7);
        app.world_mut().get_mut::<Transform>(actor).unwrap().scale.y = 0.3;
        app.update();
        let updated = app.world().get::<Collider>(actor).unwrap().shape_scaled().as_capsule().unwrap();
        assert!((updated.segment.a.y - 0.000001).abs() < 1e-8);
    }

    #[test]
    fn collision_layer_owners_restore_only_after_the_last_release() {
        let custom = CollisionLayers::new(AlsCollisionLayer::Visibility, [AlsCollisionLayer::Default, AlsCollisionLayer::Visibility]);
        let mut ownership = AlsCollisionLayerOwnership::default();
        assert_eq!(
            ownership.set_owner(AlsCollisionLayerOwner::Mantle, Some(custom), true),
            Some(Some(CollisionLayers::NONE))
        );
        assert_eq!(ownership.original_layers(), custom);
        assert_eq!(
            ownership.set_owner(
                AlsCollisionLayerOwner::Ragdoll,
                Some(CollisionLayers::NONE),
                true
            ),
            None
        );
        assert_eq!(
            ownership.set_owner(
                AlsCollisionLayerOwner::Mantle,
                Some(CollisionLayers::NONE),
                false
            ),
            None
        );
        assert_eq!(
            ownership.set_owner(
                AlsCollisionLayerOwner::Ragdoll,
                Some(CollisionLayers::NONE),
                false
            ),
            Some(Some(custom))
        );
    }

    #[test]
    fn collision_layer_owner_restores_an_absent_component() {
        let mut ownership = AlsCollisionLayerOwnership::default();
        assert_eq!(ownership.original_layers(), CollisionLayers::new(AlsCollisionLayer::Default, AlsCollisionLayer::Default));
        assert_eq!(
            ownership.set_owner(AlsCollisionLayerOwner::Mantle, None, true),
            Some(Some(CollisionLayers::NONE))
        );
        assert_eq!(
            ownership.set_owner(
                AlsCollisionLayerOwner::Mantle,
                Some(CollisionLayers::NONE),
                false
            ),
            Some(None)
        );
    }

    #[test]
    fn crush_recovery_keeps_scaled_air_and_recovers_real_penetration() {
        for scale in [0.5, 1.0, 2.0] {
            for stance in [Stance::Standing, Stance::Crouching] {
                for penetrating in [false, true] {
                    let (mut app, params) = crush_recovery_test_app();
                    let half = if stance == Stance::Standing { 0.9 } else { 0.56 };
                    let center = Vec3::Y * (half * scale + if penetrating { -0.05 } else { 0.15 });
                    let character = spawn_crush_recovery_character(&mut app, params, center, -1.0);
                    let mut capsule = als_character_capsule(half, 0.3);
                    capsule.set_scale(Vec3::splat(scale), 16);
                    app.world_mut().entity_mut(character).insert((capsule, stance, LocomotionState::Air));
                    app.world_mut().get_mut::<Transform>(character).unwrap().scale = Vec3::splat(scale);
                    spawn_crush_recovery_floor(&mut app, Vec3::Y * -0.1, Vec3::new(2.0, 0.1, 2.0));
                    app.update();
                    let expected = if penetrating { Vec3::Y * (half * scale + CRUSH_RECOVERY_SKIN) } else { center };
                    assert!(app.world().get::<Position>(character).unwrap().0.distance(expected) < 1e-6,
                        "scale={scale} stance={stance:?} penetrating={penetrating}");
                    assert_eq!(app.world().get::<Transform>(character).unwrap().translation,
                        app.world().get::<Position>(character).unwrap().0);
                    assert_eq!(app.world().get::<LinearVelocity>(character).unwrap().y, if penetrating { 0.0 } else { -1.0 });
                    assert_eq!(app.world().get::<AlsCrushRecovery>(character).unwrap().corrections, u32::from(penetrating));
                    app.update();
                    assert_eq!(app.world().get::<AlsCrushRecovery>(character).unwrap().corrections, u32::from(penetrating));
                }
            }
        }
    }

    #[test]
    fn crush_recovery_rejects_nonblocking_floor_candidates() {
        for sensor in [true, false] {
            let (mut app, params) = crush_recovery_test_app();
            let character = spawn_crush_recovery_character(&mut app, params, Vec3::Y * 0.85, -1.0);
            app.world_mut().entity_mut(character).insert(CollisionLayers::new(
                AlsCollisionLayer::Default, AlsCollisionLayer::Default));
            spawn_crush_recovery_floor(&mut app, Vec3::Y * -0.1, Vec3::new(2.0, 0.1, 2.0));
            let floor = app.world_mut().query_filtered::<Entity, With<AlsCrushRecoveryFloor>>()
                .single(app.world()).unwrap();
            if sensor { app.world_mut().entity_mut(floor).insert(avian3d::prelude::Sensor); }
            else { app.world_mut().entity_mut(floor).insert(CollisionLayers::new(
                AlsCollisionLayer::Visibility, AlsCollisionLayer::Visibility)); }
            app.update();
            assert_eq!(app.world().get::<Position>(character).unwrap().0, Vec3::Y * 0.85,
                "A nonblocking floor must not trigger recovery: sensor={sensor}");
            assert_eq!(app.world().get::<LinearVelocity>(character).unwrap().y, -1.0);
            assert_eq!(app.world().get::<AlsCrushRecovery>(character).unwrap().corrections, 0);
        }
    }

    #[test]
    fn crush_recovery_uses_real_floor_geometry_after_transforming() {
        for (rotation, scale, outside) in [
            (Quat::IDENTITY, Vec3::splat(2.0), false),
            (Quat::from_rotation_y(0.7), Vec3::splat(2.0), true),
            (Quat::from_rotation_z(0.35), Vec3::ONE, false),
        ] {
            let (mut app, params) = crush_recovery_test_app();
            let center = if outside { Vec3::new(8.0, 0.6, 0.0) } else { Vec3::Y * 0.6 };
            let character = spawn_crush_recovery_character(&mut app, params, center, -1.0);
            let mut capsule = als_character_capsule(0.9, 0.3);
            capsule.set_scale(Vec3::splat(0.5), 16);
            app.world_mut().entity_mut(character).insert((capsule, LocomotionState::Air));
            app.world_mut().get_mut::<Transform>(character).unwrap().scale = Vec3::splat(0.5);
            let transform = Transform::from_xyz(0.0, -0.1 * scale.y, 0.0)
                .with_rotation(rotation).with_scale(scale);
            let mut floor = Collider::cuboid(4.0, 0.2, 4.0);
            floor.set_scale(scale, 16);
            app.world_mut().spawn((GlobalTransform::from(transform),
                AlsCrushRecoveryFloor { half_extents: Vec3::new(2.0, 0.1, 2.0) }, floor));
            app.update();
            assert_eq!(app.world().get::<Position>(character).unwrap().0, center,
                "A transformed floor bounding box is not proof of capsule penetration");
            assert_eq!(app.world().get::<LinearVelocity>(character).unwrap().y, -1.0);
            assert_eq!(app.world().get::<AlsCrushRecovery>(character).unwrap().corrections, 0);
        }
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
