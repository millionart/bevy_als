use avian3d::dynamics::integrator::VelocityIntegrationData;
use avian3d::dynamics::solver::islands::{BodyIslandNode, IslandId, PhysicsIslands, SleepIslands};
use avian3d::dynamics::solver::solver_body::{
    SolverBodies, SolverBodyFlags, SolverBodyIndex, SolverBodyInertia,
};
use avian3d::prelude::{
    AngularInertia, AngularVelocity, Collider, ComputedCenterOfMass, CustomPositionIntegration,
    JointDisabled, LinearVelocity, Physics, Position, RigidBody, Rotation,
};
use avian3d::schedule::LastPhysicsTick;
use bevy::ecs::{change_detection::Tick, system::SystemChangeTick};
use bevy::math::Isometry3d;
use bevy::prelude::*;
use serde::Deserialize;

use crate::extensions::source_physics_joint::{SourceJointSolverSettings, SourcePhysicsJoint};

#[derive(Component, Clone, Copy, Debug, Deserialize)]
pub struct SourceBodySleep {
    pub linear_threshold_m_per_second: f32,
    pub angular_threshold_radians_per_second: f32,
    pub counter_threshold: i32,
    #[serde(skip)]
    pub(super) counter: i32,
    #[serde(skip)]
    pub(super) smooth_linear: Vec3,
    #[serde(skip)]
    pub(super) smooth_angular: Vec3,
    #[serde(skip)]
    explicit_wake: bool,
    #[serde(skip)]
    pending_linear_impulse: Vec3,
    #[serde(skip)]
    last_impulse_tick: Option<Tick>,
    #[serde(skip)]
    linear_write_before_impulse: bool,
}

impl SourceBodySleep {
    pub(crate) fn has_external_linear_write(
        &self,
        changed: Tick,
        physics: Tick,
        current: Tick,
    ) -> bool {
        changed.is_newer_than(physics, current) && Some(changed) != self.last_impulse_tick
    }

    pub(crate) fn record_linear_impulse(
        &mut self,
        delta: Vec3,
        tick: Tick,
        had_velocity_write: bool,
    ) {
        if delta != Vec3::ZERO {
            self.pending_linear_impulse += delta;
            self.last_impulse_tick = Some(tick);
            self.linear_write_before_impulse |= had_velocity_write;
        }
    }
}

#[derive(Component)]
pub(super) struct SourceGyroscopicTorqueDisabled;

#[derive(Component)]
pub(super) struct SourceBodyDof {
    pub(super) axes: avian3d::prelude::LockedAxes,
    pub(super) anchor: Option<Isometry3d>,
    pub(super) settings: SourceJointSolverSettings,
}

#[derive(Component, Clone, Copy, Debug, Deserialize)]
pub struct SourceInertiaConditioningSettings {
    pub(super) enabled: bool,
    distance_m: f32,
    rotation_ratio: f32,
    max_inverse_inertia_component_ratio: f32,
    inverse_mass_tolerance: f32,
    inverse_inertia_tolerance: f32,
    extent_tolerance_m: f32,
    exclude_free_joints: bool,
}

impl SourceInertiaConditioningSettings {
    fn inverse_scale(self, inverse_mass: f32, inverse: Vec3, extents: Vec3) -> Vec3 {
        if !(self.enabled
            && inverse_mass > self.inverse_mass_tolerance
            && inverse.min_element() > self.inverse_inertia_tolerance
            && extents.min_element() > self.extent_tolerance_m)
        {
            return Vec3::ONE;
        }
        let squared = extents * extents;
        let ratio = inverse / inverse_mass
            * Vec3::new(
                squared.y.max(squared.z),
                squared.x.max(squared.z),
                squared.x.max(squared.y),
            );
        let maximum = self.rotation_ratio
            * if self.distance_m > 0.0 {
                (extents.max_element() / self.distance_m).min(1.0)
            } else {
                1.0
            };
        let mut scale = (Vec3::splat(maximum) / ratio).min(Vec3::ONE);
        if self.max_inverse_inertia_component_ratio > 1.0 {
            let scaled = scale * inverse;
            let limit = scaled.min_element() * self.max_inverse_inertia_component_ratio;
            for index in 0..3 {
                if scaled[index] > limit {
                    scale[index] = limit / scaled[index];
                }
            }
            scale = scale.min(Vec3::ONE);
        }
        scale
    }
}

#[derive(Component, Default)]
pub(crate) struct SourceBodyFrameRotation {
    integrated: bool,
}

pub(crate) fn capture_source_sleep_wakes(
    last_physics_tick: Res<LastPhysicsTick>,
    tick: SystemChangeTick,
    mut bodies: Query<(
        &RigidBody,
        &mut LinearVelocity,
        Ref<AngularVelocity>,
        &mut SourceBodySleep,
    )>,
) {
    let [linear_tolerance, angular_tolerance]: [f32; 2] = include!("sleep_velocity_tolerance.rs");
    for (body, mut linear, angular, mut sleep) in &mut bodies {
        sleep.explicit_wake = false;
        let external_linear = linear.is_changed()
            && sleep.has_external_linear_write(
                linear.last_changed(),
                last_physics_tick.0,
                tick.this_run(),
            );
        let changed = sleep.linear_write_before_impulse
            || external_linear
            || (angular.is_changed()
                && angular
                    .last_changed()
                    .is_newer_than(last_physics_tick.0, tick.this_run()));
        if body.is_dynamic() && changed {
            let source_linear = if sleep.last_impulse_tick == Some(linear.last_changed()) {
                linear.0 - sleep.pending_linear_impulse
            } else {
                linear.0
            };
            if external_linear && sleep.last_impulse_tick.is_some() {
                linear.0 += sleep.pending_linear_impulse;
            }
            sleep.smooth_linear = source_linear;
            sleep.smooth_angular = angular.0;
            sleep.explicit_wake = source_linear.abs().max_element() > linear_tolerance
                || angular.0.abs().max_element() > angular_tolerance;
            if sleep.explicit_wake {
                sleep.counter = 0;
            }
        }
    }
}

pub(super) fn seed_source_sleep_from_forces(
    mut bodies: Query<(
        &RigidBody,
        &LinearVelocity,
        &AngularVelocity,
        &Rotation,
        &VelocityIntegrationData,
        Option<&avian3d::dynamics::rigid_body::forces::AccumulatedLocalAcceleration>,
        &mut SourceBodySleep,
    )>,
) {
    let [linear_tolerance, angular_tolerance]: [f32; 2] = include!("sleep_velocity_tolerance.rs");
    let [prediction_time, rate]: [f32; 2] = include!("sleep_force_parameters.rs");
    for (body, linear, angular, rotation, integration, local, mut sleep) in &mut bodies {
        if body.is_dynamic() {
            let linear_acceleration = integration.linear_increment
                + local.map_or(Vec3::ZERO, |local| rotation.0 * local.linear);
            let angular_acceleration = integration.angular_increment
                + local.map_or(Vec3::ZERO, |local| rotation.0 * local.angular);
            if linear_acceleration.abs().max_element() > linear_tolerance
                || sleep.pending_linear_impulse.abs().max_element() > linear_tolerance
            {
                sleep.smooth_linear = sleep
                    .smooth_linear
                    .lerp(linear.0 + linear_acceleration * prediction_time, rate);
            }
            if angular_acceleration.abs().max_element() > angular_tolerance {
                sleep.smooth_angular = sleep
                    .smooth_angular
                    .lerp(angular.0 + angular_acceleration * prediction_time, rate);
            }
        }
        sleep.pending_linear_impulse = Vec3::ZERO;
        sleep.last_impulse_tick = None;
        sleep.linear_write_before_impulse = false;
    }
}

pub(crate) fn integrate_source_body_rotation(
    frame: Res<Time<Physics>>,
    step: Res<Time>,
    mut solver_bodies: ResMut<SolverBodies>,
    mut bodies: Query<(&SolverBodyIndex, &mut SourceBodyFrameRotation)>,
) {
    for (index, mut state) in &mut bodies {
        let Some(body) = solver_bodies.get_mut(*index) else {
            continue;
        };
        let velocity = body.linear_velocity;
        body.delta_position += velocity * step.delta_secs();
        if !state.integrated {
            let angular = body.angular_velocity * (0.5 * frame.delta_secs());
            body.delta_rotation = (Quat::from_xyzw(angular.x, angular.y, angular.z, 1.0)
                * body.delta_rotation)
                .normalize();
            state.integrated = true;
        }
    }
}

pub(super) fn disable_source_body_rotation(
    mut commands: Commands,
    bodies: Query<Entity, With<SourceBodyFrameRotation>>,
) {
    for entity in &bodies {
        commands
            .entity(entity)
            .remove::<(CustomPositionIntegration, SourceBodyFrameRotation)>();
    }
}

pub(super) fn prepare_source_sleep_islands(
    mut islands: ResMut<PhysicsIslands>,
    bodies: Query<&BodyIslandNode, With<SourceBodySleep>>,
) {
    if let Some(node) = bodies.iter().find(|node| {
        islands
            .get(node.island_id())
            .is_some_and(|island| island.constraints_removed() > 0)
    }) {
        islands.split_candidate = Some(node.island_id());
    }
}

pub(super) fn update_source_sleep(
    mut commands: Commands,
    islands: Res<PhysicsIslands>,
    time: Res<Time>,
    time_to_sleep: Res<avian3d::prelude::TimeToSleep>,
    solver_bodies: Res<SolverBodies>,
    mut bodies: Query<(
        &BodyIslandNode,
        &RigidBody,
        &SolverBodyIndex,
        &Rotation,
        &ComputedCenterOfMass,
        &mut SourceBodySleep,
        Has<avian3d::prelude::SleepingDisabled>,
        Has<avian3d::prelude::Sleeping>,
    )>,
    other_bodies: Query<
        (
            &BodyIslandNode,
            &avian3d::prelude::SleepTimer,
            Has<avian3d::prelude::SleepingDisabled>,
        ),
        Without<SourceBodySleep>,
    >,
    topology: Query<(Entity, &BodyIslandNode)>,
    mut previous_connected: Local<bevy::platform::collections::HashMap<Entity, bool>>,
) {
    let [_, smooth_rate]: [f32; 2] = include!("sleep_force_parameters.rs");
    let retained_islands = topology
        .iter()
        .filter_map(|(entity, node)| {
            previous_connected
                .get(&entity)
                .copied()
                .unwrap_or(false)
                .then_some(node.island_id().0)
        })
        .collect::<bevy::platform::collections::HashSet<_>>();
    let mut groups = bevy::platform::collections::HashMap::<u32, (i32, i32, bool, bool)>::default();
    for (node, mode, index, rotation, center, mut sleep, disabled, sleeping) in &mut bodies {
        let Some(solver) = solver_bodies.get(*index) else {
            continue;
        };
        if !mode.is_dynamic() {
            sleep.counter = 0;
            sleep.smooth_linear = solver.linear_velocity;
            sleep.smooth_angular = solver.angular_velocity;
            continue;
        }
        if sleeping {
            continue;
        }
        let Some(island) = islands.get(node.island_id()) else {
            continue;
        };
        let isolated = island.contact_count() == 0 && island.joint_count() == 0;
        if time.delta_secs() > 1e-8 {
            if isolated {
                sleep.smooth_linear = solver.linear_velocity;
                sleep.smooth_angular = solver.angular_velocity;
            } else {
                let world_center = rotation.0 * center.0;
                let origin_delta = solver.delta_position
                    + (solver.delta_rotation.conjugate() * world_center - world_center);
                sleep.smooth_linear = sleep
                    .smooth_linear
                    .lerp(origin_delta / time.delta_secs(), smooth_rate);
                let delta = solver.delta_rotation;
                let delta = if delta.w < 0.0 { -delta } else { delta };
                let angular = Vec3::new(delta.x, delta.y, delta.z) * (2.0 / time.delta_secs());
                sleep.smooth_angular = sleep.smooth_angular.lerp(angular, smooth_rate);
            }
        }
        let within = !disabled
            && (sleep.linear_threshold_m_per_second > 0.0
                || sleep.angular_threshold_radians_per_second > 0.0)
            && sleep.smooth_linear.length_squared() <= sleep.linear_threshold_m_per_second.powi(2)
            && sleep.smooth_angular.length_squared()
                <= sleep.angular_threshold_radians_per_second.powi(2);
        let group = groups
            .entry(node.island_id().0)
            .or_insert((i32::MAX, 0, true, isolated));
        group.0 = group.0.min(sleep.counter);
        if !isolated && !retained_islands.contains(&node.island_id().0) {
            group.0 = 0;
        }
        group.1 = group.1.max(if isolated {
            sleep.counter_threshold.min(127)
        } else {
            sleep.counter_threshold
        });
        group.2 &= within && (!sleep.explicit_wake || isolated);
    }
    for (node, timer, disabled) in &other_bodies {
        if let Some(group) = groups.get_mut(&node.island_id().0) {
            group.2 &= !disabled && timer.0 >= time_to_sleep.0;
        }
    }
    let mut to_sleep = Vec::new();
    for (id, group) in &mut groups {
        if group.2 {
            group.0 = group.0.saturating_add(1);
            if group.3 {
                group.0 = group.0.min(127);
            }
            if group.0 > group.1 {
                to_sleep.push(IslandId(*id));
                group.0 = 0;
            }
        } else if !group.3 {
            group.0 = 0;
        }
    }
    for (node, _, _, _, _, mut sleep, _, _) in &mut bodies {
        if let Some(group) = groups.get(&node.island_id().0) {
            sleep.counter = group.0;
        }
    }
    previous_connected.clear();
    for (entity, node) in &topology {
        if let Some(island) = islands.get(node.island_id()) {
            previous_connected.insert(
                entity,
                island.contact_count() > 0 || island.joint_count() > 0,
            );
        }
    }
    if !to_sleep.is_empty() {
        commands.queue(SleepIslands(to_sleep));
    }
}

pub(super) fn initialize_source_body_dof(
    mut bodies: Query<(&RigidBody, &Position, &Rotation, &mut SourceBodyDof)>,
) {
    for (mode, position, rotation, mut dof) in &mut bodies {
        if !mode.is_dynamic() {
            dof.anchor = None;
        } else if dof.anchor.is_none() {
            dof.anchor = Some(Isometry3d::new(position.0, rotation.0));
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn condition_source_solver_inertia(
    mut solver_bodies: ResMut<SolverBodies>,
    bodies: Query<(
        Entity,
        &RigidBody,
        &Rotation,
        &avian3d::prelude::ComputedMass,
        &avian3d::prelude::ComputedAngularInertia,
        &avian3d::prelude::ComputedCenterOfMass,
        &SourceInertiaConditioningSettings,
        Option<&SourceBodyDof>,
        &SolverBodyIndex,
    )>,
    colliders: Query<(
        &Collider,
        &avian3d::prelude::ColliderOf,
        &avian3d::prelude::ColliderTransform,
    )>,
    joints: Query<&SourcePhysicsJoint, Without<JointDisabled>>,
) {
    for (entity, mode, rotation, mass, angular, com, settings, dof, index) in &bodies {
        if !mode.is_dynamic() {
            continue;
        }
        let Some(body) = solver_bodies.get(*index) else {
            continue;
        };
        let delta_rotation = body.delta_rotation;
        let Some(solver) = solver_bodies.get_inertia_mut(*index) else {
            continue;
        };
        let principal = AngularInertia::from_tensor(angular.inverse());
        let local_basis = Mat3::from_quat(principal.local_frame.conjugate());
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for (shape, owner, transform) in &colliders {
            if owner.body != entity {
                continue;
            }
            let pose =
                avian3d::parry::math::Pose3::from_parts(transform.translation, transform.rotation);
            let bounds = shape.shape_scaled().compute_aabb(&pose);
            min = min.min(bounds.mins);
            max = max.max(bounds.maxs);
        }
        let half = ((max - min) * 0.5).max(Vec3::ZERO);
        let mut extents = local_basis.x_axis.abs() * half.x
            + local_basis.y_axis.abs() * half.y
            + local_basis.z_axis.abs() * half.z;
        for joint in &joints {
            if settings.exclude_free_joints
                && !joint.linear_locked
                && !joint
                    .linear_motors
                    .iter()
                    .any(|motor| motor.position_enabled)
            {
                continue;
            }
            let arm = if joint.parent == entity {
                Some(Vec3::from(joint.parent_frame.translation))
            } else if joint.child == entity {
                Some(Vec3::from(joint.child_frame.translation))
            } else {
                None
            };
            if let Some(arm) = arm {
                extents = extents.max((local_basis * (arm - com.0)).abs());
            }
        }
        if dof.is_some_and(|dof| {
            !settings.exclude_free_joints
                || dof.axes.is_translation_x_locked()
                || dof.axes.is_translation_y_locked()
                || dof.axes.is_translation_z_locked()
        }) {
            extents = extents.max((local_basis * -com.0).abs());
        }
        let scale = settings.inverse_scale(mass.inverse(), principal.principal, extents);
        let basis = Mat3::from_quat(delta_rotation * rotation.0 * principal.local_frame);
        let inverse = avian3d::math::SymmetricTensor::from_mat3_unchecked(
            basis * Mat3::from_diagonal(principal.principal * scale) * basis.transpose(),
        );
        *solver = SolverBodyInertia::new(
            mass.inverse(),
            inverse,
            solver.flags().locked_axes(),
            solver.dominance() as i8,
            true,
        );
    }
}

pub(super) fn solve_source_body_dof(
    mut solver_bodies: ResMut<SolverBodies>,
    bodies: Query<
        (
            &RigidBody,
            &Position,
            &Rotation,
            &avian3d::prelude::ComputedCenterOfMass,
            &SourceBodyDof,
            &SolverBodyIndex,
        ),
        Without<avian3d::prelude::RigidBodyDisabled>,
    >,
) {
    for (mode, position, rotation, com, dof, index) in &bodies {
        let Some(anchor) = dof.anchor.filter(|_| mode.is_dynamic()) else {
            continue;
        };
        let Some(inertia) = solver_bodies.get_inertia(*index) else {
            continue;
        };
        let translation_locked = [
            dof.axes.is_translation_x_locked(),
            dof.axes.is_translation_y_locked(),
            dof.axes.is_translation_z_locked(),
        ];
        let rotation_locked = [
            dof.axes.is_rotation_x_locked(),
            dof.axes.is_rotation_y_locked(),
            dof.axes.is_rotation_z_locked(),
        ];
        let inverse_mass = inertia.effective_inv_mass();
        let inverse_angular = inertia.effective_inv_angular_inertia();
        let Some(body) = solver_bodies.get_mut(*index) else {
            continue;
        };
        for _ in 0..dof.settings.position_iterations {
            for index in 0..3 {
                if !rotation_locked[index] {
                    continue;
                }
                let mut relative =
                    (body.delta_rotation * rotation.0 * anchor.rotation.conjugate()).normalize();
                if relative.w < 0.0 {
                    relative = -relative;
                }
                let vector = Vec3::new(relative.x, relative.y, relative.z);
                let axis = 0.5 * (Vec3::AXES[index] * relative.w + vector.cross(Vec3::AXES[index]));
                let response = inverse_angular * axis;
                let denominator = axis.dot(response);
                if denominator > f32::EPSILON {
                    body.delta_rotation =
                        (Quat::from_scaled_axis(-response * vector[index] / denominator)
                            * body.delta_rotation)
                            .normalize();
                }
            }
            for index in 0..3 {
                if !translation_locked[index] {
                    continue;
                }
                let axis = Vec3::AXES[index];
                let arm = -(body.delta_rotation * rotation.0 * com.0);
                let angular_axis = arm.cross(axis);
                let response = inverse_angular * angular_axis;
                let denominator = inverse_mass[index] + angular_axis.dot(response);
                let origin = position.0 + body.delta_position + rotation.0 * com.0 + arm;
                if denominator > f32::EPSILON {
                    let lambda = (origin[index] - anchor.translation[index]) / denominator;
                    body.delta_position -= inverse_mass * axis * lambda;
                    body.delta_rotation = (Quat::from_scaled_axis(-response * lambda)
                        * body.delta_rotation)
                        .normalize();
                }
            }
        }
        for _ in 0..dof.settings.velocity_iterations {
            for index in 0..3 {
                if rotation_locked[index] {
                    let relative = (body.delta_rotation * rotation.0 * anchor.rotation.conjugate())
                        .normalize();
                    let vector = Vec3::new(relative.x, relative.y, relative.z);
                    let axis =
                        0.5 * (Vec3::AXES[index] * relative.w + vector.cross(Vec3::AXES[index]));
                    let response = inverse_angular * axis;
                    let denominator = axis.dot(response);
                    if denominator > f32::EPSILON {
                        let lambda = body.angular_velocity.dot(axis) / denominator;
                        body.angular_velocity -= response * lambda;
                    }
                }
                if translation_locked[index] {
                    let arm = -(body.delta_rotation * rotation.0 * com.0);
                    let angular_axis = arm.cross(Vec3::AXES[index]);
                    let response = inverse_angular * angular_axis;
                    let denominator = inverse_mass[index] + angular_axis.dot(response);
                    if denominator > f32::EPSILON {
                        let lambda = (body.linear_velocity + body.angular_velocity.cross(arm))
                            [index]
                            / denominator;
                        body.linear_velocity -= inverse_mass * Vec3::AXES[index] * lambda;
                        body.angular_velocity -= response * lambda;
                    }
                }
            }
        }
    }
}

pub(super) fn disable_source_gyroscopic_torque(
    mut solver_bodies: ResMut<SolverBodies>,
    bodies: Query<&SolverBodyIndex, With<SourceGyroscopicTorqueDisabled>>,
) {
    for index in &bodies {
        if let Some(body) = solver_bodies.get_mut(*index) {
            body.flags.remove(SolverBodyFlags::GYROSCOPIC_MOTION);
        }
    }
}

pub(super) fn source_angular_inertia(inertia: Mat3) -> AngularInertia {
    let diagonal = Vec3::new(inertia.x_axis.x, inertia.y_axis.y, inertia.z_axis.z);
    let trace = (f64::from(diagonal.x) + f64::from(diagonal.y) + f64::from(diagonal.z)) / 3.0;
    let off_diagonal = [inertia.x_axis.y, inertia.x_axis.z, inertia.y_axis.z]
        .into_iter()
        .map(|value| f64::from(value).powi(2))
        .sum::<f64>();
    if trace * 10_000.0 <= f64::from(1e-8_f32)
        || off_diagonal / (trace * trace) < f64::from(1e-8_f32)
    {
        return AngularInertia::new(diagonal);
    }
    let mut result = AngularInertia::from_mat3_unchecked(inertia / trace as f32);
    result.principal *= trace as f32;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_body_inertia_algorithm_matches_native_ue_samples() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/physics/body_inertia_conditioning.json"
        ))
        .unwrap();
        let settings: SourceInertiaConditioningSettings =
            serde_json::from_value(fixture["settings"].clone()).unwrap();
        let vector = |value: &serde_json::Value| {
            Vec3::new(
                value[0].as_f64().unwrap() as f32,
                value[1].as_f64().unwrap() as f32,
                value[2].as_f64().unwrap() as f32,
            )
        };
        for (index, sample) in fixture["samples"].as_array().unwrap().iter().enumerate() {
            let settings = SourceInertiaConditioningSettings {
                enabled: true,
                distance_m: sample["maximum_distance_m"].as_f64().unwrap() as f32,
                rotation_ratio: sample["maximum_rotation_ratio"].as_f64().unwrap() as f32,
                max_inverse_inertia_component_ratio: sample["maximum_component_ratio"]
                    .as_f64()
                    .unwrap() as f32,
                ..settings
            };
            let actual = settings.inverse_scale(
                sample["inverse_mass"].as_f64().unwrap() as f32,
                vector(&sample["inverse_inertia"]),
                vector(&sample["extents_m"]),
            );
            assert!(
                actual.abs_diff_eq(vector(&sample["inverse_scale"]), 1e-6),
                "reference inertia sample {index}: {actual:?} != {:?}",
                vector(&sample["inverse_scale"])
            );
        }
    }
}
