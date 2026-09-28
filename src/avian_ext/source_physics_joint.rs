use avian3d::{
    dynamics::{
        joints::EntityConstraint,
        solver::{
            joint_graph::JointGraphPlugin,
            solver_body::{SolverBody, SolverBodyInertia},
            xpbd::{
                XpbdConstraint, XpbdConstraintSolverData, XpbdSolverSystems,
                joints::SphericalJointSolverData, prepare_xpbd_joint,
            },
        },
    },
    math::SymmetricMatrix,
    physics_transform::PreSolveDeltaRotation,
    prelude::{
        AngularInertia, JointDisabled, LockedAxes, PhysicsSchedule, RigidBodyDisabled,
        RigidBodyQueryReadOnlyItem, SolverSystems, SphericalJoint, SubstepSchedule,
    },
};
use bevy::{
    ecs::entity::{EntityMapper, MapEntities},
    math::Isometry3d,
    prelude::*,
};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum SourceAngularLimit {
    Free,
    Locked,
    Limited(f32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub struct SourceAngularSpring {
    pub stiffness: f32,
    pub damping: f32,
    pub acceleration_mode: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct SourceAngularMotor {
    #[serde(flatten)]
    pub spring: SourceAngularSpring,
    pub position_enabled: bool,
    pub velocity_enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct SourceLinearMotor {
    pub stiffness: f32,
    pub damping: f32,
    pub max_force: f32,
    pub position_enabled: bool,
    pub velocity_enabled: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SourceJointSolverSettings {
    pub position_iterations: u32,
    pub projection_iterations: u32,
    pub min_parent_mass_ratio: f32,
    pub max_inertia_ratio: f32,
    pub angle_tolerance_radians: f32,
    pub position_tolerance_m: f32,
    pub projection_velocity_alpha: f32,
}

#[derive(Component, Clone, Debug)]
#[require(SourcePhysicsJointSolverData)]
pub struct SourcePhysicsJoint {
    pub parent: Entity,
    pub child: Entity,
    pub parent_frame: Isometry3d,
    pub child_frame: Isometry3d,
    pub angular_limits: [SourceAngularLimit; 3],
    pub swing_spring: Option<SourceAngularSpring>,
    pub twist_spring: Option<SourceAngularSpring>,
    pub motors: [SourceAngularMotor; 3],
    pub motor_target: Quat,
    pub motor_velocity_target: Vec3,
    pub motor_stiffness_scale: f32,
    pub motor_damping_scale: f32,
    pub linear_locked: bool,
    pub linear_motors: [SourceLinearMotor; 3],
    pub linear_position_target: Vec3,
    pub linear_velocity_target: Vec3,
    pub linear_drive_acceleration_mode: bool,
    pub mass_conditioning: bool,
    pub projection_linear_alpha: f32,
    pub projection_linear_tolerance_m: f32,
    pub settings: SourceJointSolverSettings,
}

impl EntityConstraint<2> for SourcePhysicsJoint {
    fn entities(&self) -> [Entity; 2] {
        [self.parent, self.child]
    }
}

impl MapEntities for SourcePhysicsJoint {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.parent = mapper.get_mapped(self.parent);
        self.child = mapper.get_mapped(self.child);
    }
}

#[derive(Component, Clone, Debug)]
pub struct SourcePhysicsJointSolverData {
    point: SphericalJoint,
    point_data: SphericalJointSolverData,
    frames: [Quat; 2],
    arms: [Vec3; 2],
    center_difference: Vec3,
    pre_solve_rotation: [Quat; 2],
    limit_lambdas: [f32; 3],
    motor_lambdas: [f32; 3],
    linear_motor_lambdas: [f32; 3],
    total_angular_impulse: Vec3,
}

impl Default for SourcePhysicsJointSolverData {
    fn default() -> Self {
        Self {
            point: SphericalJoint::new(Entity::PLACEHOLDER, Entity::PLACEHOLDER),
            point_data: default(),
            frames: [Quat::IDENTITY; 2],
            arms: [Vec3::ZERO; 2],
            center_difference: Vec3::ZERO,
            pre_solve_rotation: [Quat::IDENTITY; 2],
            limit_lambdas: [0.0; 3],
            motor_lambdas: [0.0; 3],
            linear_motor_lambdas: [0.0; 3],
            total_angular_impulse: Vec3::ZERO,
        }
    }
}

impl XpbdConstraintSolverData for SourcePhysicsJointSolverData {
    fn clear_lagrange_multipliers(&mut self) {
        self.point_data.clear_lagrange_multipliers();
        self.limit_lambdas = [0.0; 3];
        self.motor_lambdas = [0.0; 3];
        self.linear_motor_lambdas = [0.0; 3];
        self.total_angular_impulse = Vec3::ZERO;
    }

    fn total_position_lagrange(&self) -> Vec3 {
        self.point_data.total_position_lagrange()
    }

    fn total_rotation_lagrange(&self) -> Vec3 {
        self.total_angular_impulse
    }
}

impl XpbdConstraint<2> for SourcePhysicsJoint {
    type SolverData = SourcePhysicsJointSolverData;

    fn prepare(&mut self, bodies: [&RigidBodyQueryReadOnlyItem; 2], data: &mut Self::SolverData) {
        data.point = SphericalJoint::new(self.parent, self.child)
            .with_local_frame1(self.parent_frame)
            .with_local_frame2(self.child_frame);
        data.point.prepare(bodies, &mut data.point_data);
        data.frames = [
            bodies[0].rotation.0 * self.parent_frame.rotation,
            bodies[1].rotation.0 * self.child_frame.rotation,
        ];
        data.arms = [
            bodies[0].rotation.0
                * (Vec3::from(self.parent_frame.translation) - bodies[0].center_of_mass.0),
            bodies[1].rotation.0
                * (Vec3::from(self.child_frame.translation) - bodies[1].center_of_mass.0),
        ];
        data.center_difference = bodies[1].position.0 - bodies[0].position.0
            + bodies[1].rotation.0 * bodies[1].center_of_mass.0
            - bodies[0].rotation.0 * bodies[0].center_of_mass.0;
    }

    fn solve(
        &mut self,
        [parent, child]: [&mut SolverBody; 2],
        inertias: [&SolverBodyInertia; 2],
        data: &mut Self::SolverData,
        dt: f32,
    ) {
        let conditioned = conditioned_inertias(inertias, self.mass_conditioning, self.settings);
        let inertias = [&conditioned[0], &conditioned[1]];
        let mut limit_lambdas = data.limit_lambdas;
        let mut motor_lambdas = data.motor_lambdas;
        for index in 0..3 {
            let frames = current_frames(parent, child, data);
            let correction = angular_limit_correction(frames, self.angular_limits, index);
            let spring = match self.angular_limits[index] {
                SourceAngularLimit::Limited(_) if index == 0 => self.twist_spring,
                SourceAngularLimit::Limited(_) => self.swing_spring,
                _ => None,
            };
            apply_angular_correction(
                parent,
                child,
                inertias,
                data,
                correction,
                spring,
                0.0,
                &mut limit_lambdas[index],
                false,
                self.settings.angle_tolerance_radians,
                dt,
            );
        }
        if self.linear_locked {
            data.point
                .solve([parent, child], inertias, &mut data.point_data, dt);
        } else {
            for index in 0..3 {
                let motor = self.linear_motors[index];
                if !motor.position_enabled && !motor.velocity_enabled {
                    continue;
                }
                apply_linear_motor(parent, child, inertias, data, index,
                    motor, self.linear_position_target[index], self.linear_velocity_target[index],
                    self.linear_drive_acceleration_mode, dt);
            }
        }
        for index in 0..3 {
            if self.angular_limits[index] == SourceAngularLimit::Locked {
                continue;
            }
            let motor = self.motors[index];
            let spring = SourceAngularSpring {
                stiffness: if motor.position_enabled {
                    motor.spring.stiffness
                } else {
                    0.0
                },
                damping: if motor.velocity_enabled {
                    motor.spring.damping
                } else {
                    0.0
                },
                acceleration_mode: motor.spring.acceleration_mode,
            };
            if spring.stiffness == 0.0 && spring.damping == 0.0 {
                continue;
            }
            let (parent_frame, child_frame) = current_frames(parent, child, data);
            let error = shortest_quat((parent_frame * self.motor_target).conjugate() * child_frame);
            let twist_direction = error * Vec3::X;
            let errors = Vec3::new(2.0 * error.x, -twist_direction.z, twist_direction.y);
            let axis = child_frame * Vec3::AXES[index];
            apply_angular_correction(
                parent,
                child,
                inertias,
                data,
                Some((axis, errors[index])),
                Some(spring),
                (parent_frame * self.motor_velocity_target).dot(axis),
                &mut motor_lambdas[index],
                true,
                0.0,
                dt,
            );
        }
        data.limit_lambdas = limit_lambdas;
        data.motor_lambdas = motor_lambdas;
    }
}

fn shortest_quat(value: Quat) -> Quat {
    if value.w < 0.0 {
        -value.normalize()
    } else {
        value.normalize()
    }
}

fn current_frames(
    parent: &SolverBody,
    child: &SolverBody,
    data: &SourcePhysicsJointSolverData,
) -> (Quat, Quat) {
    (
        parent.delta_rotation.0 * data.frames[0],
        child.delta_rotation.0 * data.frames[1],
    )
}

fn angular_limit_correction(
    frames: (Quat, Quat),
    limits: [SourceAngularLimit; 3],
    index: usize,
) -> Option<(Vec3, f32)> {
    let (parent, child) = frames;
    if index == 0 {
        let relative = shortest_quat(parent.conjugate() * child);
        let length = Vec2::new(relative.x, relative.w).length();
        if length <= 1e-6 {
            return None;
        }
        let angle = 2.0 * (relative.x / length).atan2(relative.w / length);
        return limit_error(limits[0], angle).map(|error| (child * Vec3::X, error));
    }
    if let [
        _,
        SourceAngularLimit::Limited(left),
        SourceAngularLimit::Limited(right),
    ] = limits
    {
        if index != 1 {
            return None;
        }
        if (left - right).abs() > 1e-3 {
            return elliptical_correction(parent, child, left, right);
        }
        let swing = shortest_quat(relative_swing(parent, child)?);
        let (axis, angle) = swing.to_axis_angle();
        return limit_error(SourceAngularLimit::Limited(left), angle)
            .map(|error| (parent * axis, error));
    }
    let axis = if index == 1 {
        SwingAxis::One
    } else {
        SwingAxis::Two
    };
    match limits[index] {
        SourceAngularLimit::Free => None,
        SourceAngularLimit::Locked => locked_swing_correction(frames, axis),
        SourceAngularLimit::Limited(limit) => limited_swing_correction(frames, axis, limit),
    }
}

fn limit_error(limit: SourceAngularLimit, angle: f32) -> Option<f32> {
    match limit {
        SourceAngularLimit::Free => None,
        SourceAngularLimit::Locked => Some(angle),
        SourceAngularLimit::Limited(limit) => {
            let error = angle - angle.clamp(-limit, limit);
            (error != 0.0).then_some(error)
        }
    }
}

fn spring_impulse(
    spring: SourceAngularSpring,
    error: f32,
    velocity_dt: f32,
    inverse_mass: f32,
    lambda: f32,
    dt: f32,
) -> f32 {
    let scale = if spring.acceleration_mode {
        inverse_mass.recip()
    } else {
        1.0
    };
    let stiffness = scale * spring.stiffness * dt * dt;
    let damping = scale * spring.damping * dt;
    (stiffness * error + damping * velocity_dt - lambda)
        / (1.0 + (stiffness + damping) * inverse_mass)
}

fn apply_linear_motor(
    parent: &mut SolverBody,
    child: &mut SolverBody,
    inertias: [&SolverBodyInertia; 2],
    data: &mut SourcePhysicsJointSolverData,
    index: usize,
    motor: SourceLinearMotor,
    target_position: f32,
    target_velocity: f32,
    acceleration_mode: bool,
    dt: f32,
) {
    let stiffness = if motor.position_enabled { motor.stiffness } else { 0.0 };
    let damping = if motor.velocity_enabled { motor.damping } else { 0.0 };
    if stiffness <= 0.0 && damping <= 0.0 || dt <= 0.0 {
        return;
    }
    let axis = parent.delta_rotation.0 * data.frames[0] * Vec3::AXES[index];
    let arms = [parent.delta_rotation.0 * data.arms[0], child.delta_rotation.0 * data.arms[1]];
    let inverse_masses = [inertias[0].effective_inv_mass(), inertias[1].effective_inv_mass()];
    let responses = [inverse_masses[0] * axis, inverse_masses[1] * axis];
    let angular_axes = [arms[0].cross(axis), arms[1].cross(axis)];
    let angular_responses = [
        inertias[0].effective_inv_angular_inertia() * angular_axes[0],
        inertias[1].effective_inv_angular_inertia() * angular_axes[1],
    ];
    let linear_inverse_mass = axis.dot(responses[0] + responses[1]);
    let inverse_mass = linear_inverse_mass
        + angular_axes[0].dot(angular_responses[0])
        + angular_axes[1].dot(angular_responses[1]);
    if inverse_mass <= f32::EPSILON || acceleration_mode && linear_inverse_mass <= f32::EPSILON {
        return;
    }
    let separation = child.delta_position - parent.delta_position + data.center_difference
        + arms[1] - arms[0];
    let error = axis.dot(separation) - target_position;
    let velocity = child.linear_velocity + child.angular_velocity.cross(arms[1])
        - parent.linear_velocity - parent.angular_velocity.cross(arms[0]);
    let velocity_dt = (axis.dot(velocity) - target_velocity) * dt;
    let scale = if acceleration_mode { linear_inverse_mass.recip() } else { 1.0 };
    let spring = scale * stiffness * dt * dt;
    let damper = scale * damping * dt;
    let lambda = &mut data.linear_motor_lambdas[index];
    let mut delta = (spring * error + damper * velocity_dt - *lambda)
        / (1.0 + (spring + damper) * inverse_mass);
    let limit = motor.max_force * dt * dt;
    if limit > 0.0 {
        delta = delta.clamp(-limit - *lambda, limit - *lambda);
    }
    *lambda += delta;
    parent.delta_position += responses[0] * delta;
    child.delta_position -= responses[1] * delta;
    parent.delta_rotation.0 =
        (Quat::from_scaled_axis(angular_responses[0] * delta) * parent.delta_rotation.0).normalize();
    child.delta_rotation.0 =
        (Quat::from_scaled_axis(-angular_responses[1] * delta) * child.delta_rotation.0).normalize();
}

fn apply_angular_correction(
    parent: &mut SolverBody,
    child: &mut SolverBody,
    inertias: [&SolverBodyInertia; 2],
    data: &mut SourcePhysicsJointSolverData,
    correction: Option<(Vec3, f32)>,
    spring: Option<SourceAngularSpring>,
    target_velocity: f32,
    lambda: &mut f32,
    axis_only: bool,
    tolerance: f32,
    dt: f32,
) {
    let Some((axis, error)) = correction else {
        return;
    };
    if !axis.is_finite() || !error.is_finite() || error.abs() < tolerance {
        return;
    }
    let inertia_parent = inertias[0].effective_inv_angular_inertia();
    let inertia_child = inertias[1].effective_inv_angular_inertia();
    let response_parent = inertia_parent * axis;
    let response_child = inertia_child * axis;
    let inverse_mass = axis.dot(response_parent + response_child);
    if inverse_mass <= f32::EPSILON {
        return;
    }
    let delta = if let Some(spring) = spring {
        let correction_parent =
            shortest_quat(parent.delta_rotation.0 * data.pre_solve_rotation[0].conjugate())
                .to_scaled_axis();
        let correction_child =
            shortest_quat(child.delta_rotation.0 * data.pre_solve_rotation[1].conjugate())
                .to_scaled_axis();
        let velocity_dt = axis.dot(
            (child.angular_velocity - parent.angular_velocity) * dt + correction_child
                - correction_parent,
        ) - target_velocity * dt;
        spring_impulse(spring, error, velocity_dt, inverse_mass, *lambda, dt)
    } else {
        error / inverse_mass
    };
    *lambda += delta;
    let (parent_delta, child_delta) = if axis_only {
        (
            axis * axis.dot(response_parent) * delta,
            -axis * axis.dot(response_child) * delta,
        )
    } else {
        (response_parent * delta, -response_child * delta)
    };
    parent.delta_rotation.0 =
        (Quat::from_scaled_axis(parent_delta) * parent.delta_rotation.0).normalize();
    child.delta_rotation.0 =
        (Quat::from_scaled_axis(child_delta) * child.delta_rotation.0).normalize();
    data.total_angular_impulse += axis * delta;
}

fn condition_principal_inertia(inertia: Vec3, max_ratio: f32) -> Vec3 {
    let min = inertia.min_element();
    let max = inertia.max_element();
    if min > 0.0 && max_ratio > 0.0 && max / min > max_ratio {
        Vec3::splat(max / max_ratio)
            + (inertia - Vec3::splat(min)) / (max - min) * (max - max / max_ratio)
    } else {
        inertia
    }
}

fn conditioned_inertias(
    inertias: [&SolverBodyInertia; 2],
    enabled: bool,
    settings: SourceJointSolverSettings,
) -> [SolverBodyInertia; 2] {
    if !enabled {
        return [inertias[0].clone(), inertias[1].clone()];
    }
    let mut masses = [0.0; 2];
    let mut principal = [Vec3::ZERO; 2];
    let mut frames = [Quat::IDENTITY; 2];
    for index in 0..2 {
        let inverse_mass = inertias[index].effective_inv_mass().max_element();
        if inverse_mass > 0.0 {
            masses[index] = inverse_mass.recip();
            let tensor = inertias[index].effective_inv_angular_inertia();
            let inertia = AngularInertia::from_tensor(tensor.inverse());
            principal[index] =
                condition_principal_inertia(inertia.principal, settings.max_inertia_ratio);
            frames[index] = inertia.local_frame;
        }
    }
    if masses[0] > 0.0 && masses[1] > 0.0 {
        masses[0] = masses[0].max(settings.min_parent_mass_ratio * masses[1]);
        let parent_max = principal[0].max_element();
        let child_max = principal[1].max_element();
        if parent_max > 0.0 && parent_max < settings.min_parent_mass_ratio * child_max {
            principal[0] *= settings.min_parent_mass_ratio * child_max / parent_max;
        }
    }
    std::array::from_fn(|index| {
        if masses[index] <= 0.0 {
            return SolverBodyInertia::DUMMY;
        }
        let basis = Mat3::from_quat(frames[index]);
        let inverse = SymmetricMatrix::from_mat3_unchecked(
            basis * Mat3::from_diagonal(principal[index].recip()) * basis.transpose(),
        );
        SolverBodyInertia::new(
            masses[index].recip(),
            inverse,
            inertias[index].flags().locked_axes(),
            0,
            true,
        )
    })
}

fn capture_pre_solve_rotation(
    mut joints: Query<(&SourcePhysicsJoint, &mut SourcePhysicsJointSolverData)>,
    bodies: Query<&PreSolveDeltaRotation>,
) {
    for (joint, mut data) in &mut joints {
        data.limit_lambdas = [0.0; 3];
        data.motor_lambdas = [0.0; 3];
        data.linear_motor_lambdas = [0.0; 3];
        for (index, entity) in joint.entities().into_iter().enumerate() {
            data.pre_solve_rotation[index] = bodies
                .get(entity)
                .map(|value| value.0.0)
                .unwrap_or(Quat::IDENTITY);
        }
    }
}

fn solve_source_joint_iterations(
    mut joints: Query<
        (&mut SourcePhysicsJoint, &mut SourcePhysicsJointSolverData),
        Without<JointDisabled>,
    >,
    mut bodies: Query<(&mut SolverBody, &SolverBodyInertia), Without<RigidBodyDisabled>>,
    time: Res<Time>,
) {
    let count = joints
        .iter()
        .map(|(joint, _)| joint.settings.position_iterations)
        .max()
        .unwrap_or(0);
    for iteration in 0..count {
        for (mut joint, mut data) in &mut joints {
            if iteration >= joint.settings.position_iterations {
                continue;
            }
            let mut dummy = SolverBody::default();
            if let Ok([(mut parent, parent_inertia), (mut child, child_inertia)]) =
                bodies.get_many_mut(joint.entities())
            {
                let mut inertias = [parent_inertia, child_inertia];
                if parent_inertia.dominance() > child_inertia.dominance() {
                    inertias[0] = &SolverBodyInertia::DUMMY;
                } else if child_inertia.dominance() > parent_inertia.dominance() {
                    inertias[1] = &SolverBodyInertia::DUMMY;
                }
                joint.solve(
                    [&mut parent, &mut child],
                    inertias,
                    &mut data,
                    time.delta_secs(),
                );
            } else if let Ok((mut parent, inertia)) = bodies.get_mut(joint.parent) {
                joint.solve(
                    [&mut parent, &mut dummy],
                    [inertia, &SolverBodyInertia::DUMMY],
                    &mut data,
                    time.delta_secs(),
                );
            } else if let Ok((mut child, inertia)) = bodies.get_mut(joint.child) {
                joint.solve(
                    [&mut dummy, &mut child],
                    [&SolverBodyInertia::DUMMY, inertia],
                    &mut data,
                    time.delta_secs(),
                );
            }
        }
    }
}

fn project_source_joints(
    mut joints: Query<
        (&SourcePhysicsJoint, &mut SourcePhysicsJointSolverData),
        Without<JointDisabled>,
    >,
    mut bodies: Query<(&mut SolverBody, &SolverBodyInertia), Without<RigidBodyDisabled>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (joint, mut data) in &mut joints {
        if !joint.linear_locked || joint.settings.projection_iterations == 0 {
            continue;
        }
        let Ok([(mut parent, _), (mut child, inertia_child)]) =
            bodies.get_many_mut(joint.entities())
        else {
            continue;
        };
        if !child.flags.is_dynamic() {
            continue;
        }
        project_joint(joint, &mut parent, &mut child, inertia_child, &mut data, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> SourceJointSolverSettings {
        SourceJointSolverSettings {
            position_iterations: 1,
            projection_iterations: 1,
            min_parent_mass_ratio: 0.2,
            max_inertia_ratio: 5.0,
            angle_tolerance_radians: 0.001,
            position_tolerance_m: 0.00025,
            projection_velocity_alpha: 0.1,
        }
    }

    fn joint() -> SourcePhysicsJoint {
        SourcePhysicsJoint {
            parent: Entity::from_raw_u32(1).unwrap(),
            child: Entity::from_raw_u32(2).unwrap(),
            parent_frame: Isometry3d::IDENTITY,
            child_frame: Isometry3d::IDENTITY,
            angular_limits: [SourceAngularLimit::Free; 3],
            swing_spring: None,
            twist_spring: None,
            motors: [SourceAngularMotor::default(); 3],
            motor_target: Quat::IDENTITY,
            motor_velocity_target: Vec3::ZERO,
            motor_stiffness_scale: 1.5,
            motor_damping_scale: 1.5,
            linear_locked: false,
            linear_motors: [SourceLinearMotor::default(); 3],
            linear_position_target: Vec3::ZERO,
            linear_velocity_target: Vec3::ZERO,
            linear_drive_acceleration_mode: true,
            mass_conditioning: false,
            projection_linear_alpha: 1.0,
            projection_linear_tolerance_m: 0.05,
            settings: settings(),
        }
    }

    fn inertia(inverse_mass: f32, inverse_principal: Vec3) -> SolverBodyInertia {
        SolverBodyInertia::new(
            inverse_mass,
            SymmetricMatrix::from_diagonal(inverse_principal),
            LockedAxes::default(),
            0,
            true,
        )
    }

    #[test]
    fn acceleration_spring_is_mass_independent_and_force_spring_is_not() {
        let mut spring = SourceAngularSpring {
            stiffness: 37500.0,
            damping: 0.0,
            acceleration_mode: true,
        };
        let first = spring_impulse(spring, 0.2, 0.0, 2.0, 0.0, 1.0 / 60.0) * 2.0;
        let second = spring_impulse(spring, 0.2, 0.0, 200.0, 0.0, 1.0 / 60.0) * 200.0;
        assert!((first - second).abs() < 1e-6);
        assert!(first > 0.0 && first < 0.2);
        spring.acceleration_mode = false;
        let first = spring_impulse(spring, 0.2, 0.0, 2.0, 0.0, 1.0 / 60.0) * 2.0;
        let second = spring_impulse(spring, 0.2, 0.0, 200.0, 0.0, 1.0 / 60.0) * 200.0;
        assert!((first - second).abs() > 0.001);
    }

    #[test]
    fn free_linear_axes_drive_position_and_velocity_with_force_limits() {
        let mut drive_joint = joint();
        drive_joint.linear_motors[0] = SourceLinearMotor {
            stiffness: 2000.0, position_enabled: true, ..default()
        };
        drive_joint.linear_motors[1] = SourceLinearMotor {
            damping: 50.0, velocity_enabled: true, ..default()
        };
        drive_joint.linear_position_target = Vec3::X * 0.3;
        drive_joint.linear_velocity_target = Vec3::Y;
        for force_limit in [0.0, 1.0] {
            drive_joint.linear_motors[0].max_force = force_limit;
            let mut parent = SolverBody::default();
            let mut child = SolverBody::default();
            let mut data = SourcePhysicsJointSolverData::default();
            drive_joint.solve([&mut parent, &mut child], [&SolverBodyInertia::DUMMY, &inertia(1.0, Vec3::ONE)],
                &mut data, 0.1);
            if force_limit == 0.0 {
                assert!(child.delta_position.x > 0.1);
            } else {
                assert!(child.delta_position.x > 0.0 && child.delta_position.x <= 0.010001);
            }
            assert!(child.delta_position.y > 0.01);
            assert_eq!(child.delta_position.z, 0.0);
        }
        let mut no_drive = joint();
        let mut parent = SolverBody::default();
        let mut child = SolverBody::default();
        no_drive.solve([&mut parent, &mut child], [&SolverBodyInertia::DUMMY, &inertia(1.0, Vec3::ONE)],
            &mut SourcePhysicsJointSolverData::default(), 0.1);
        assert_eq!(child.delta_position, Vec3::ZERO);
    }

    #[test]
    fn linear_drive_acceleration_mode_is_mass_independent() {
        let solve = |inverse_mass, acceleration_mode| {
            let mut drive_joint = joint();
            drive_joint.linear_motors[0] = SourceLinearMotor {
                stiffness: 1000.0, position_enabled: true, ..default()
            };
            drive_joint.linear_position_target = Vec3::X * 0.3;
            drive_joint.linear_drive_acceleration_mode = acceleration_mode;
            let mut parent = SolverBody::default();
            let mut child = SolverBody::default();
            drive_joint.solve([&mut parent, &mut child],
                [&SolverBodyInertia::DUMMY, &inertia(inverse_mass, Vec3::ONE)],
                &mut SourcePhysicsJointSolverData::default(), 0.1);
            child.delta_position.x
        };
        assert!((solve(1.0, true) - solve(100.0, true)).abs() < 1e-6);
        assert!((solve(1.0, false) - solve(100.0, false)).abs() > 0.01);
    }

    #[test]
    fn free_swing_drive_uses_position_and_nonzero_angular_velocity_targets() {
        let solve = |position_enabled, velocity_enabled, speed, locked| {
            let mut drive_joint = joint();
            drive_joint.angular_limits[1] = if locked { SourceAngularLimit::Locked } else { SourceAngularLimit::Free };
            drive_joint.motors[1] = SourceAngularMotor {
                spring: SourceAngularSpring {
                    stiffness: 1000.0, damping: 50.0, acceleration_mode: true,
                },
                position_enabled, velocity_enabled,
            };
            drive_joint.motor_target = Quat::from_rotation_y(if position_enabled { 0.3 } else { 0.0 });
            drive_joint.motor_velocity_target = Vec3::Y * speed;
            let mut parent = SolverBody::default();
            let mut child = SolverBody::default();
            drive_joint.solve([&mut parent, &mut child],
                [&SolverBodyInertia::DUMMY, &inertia(1.0, Vec3::ONE)],
                &mut SourcePhysicsJointSolverData::default(), 0.1);
            child.delta_rotation.0.to_scaled_axis().y
        };
        assert!(solve(true, false, 0.0, false) > 0.01);
        assert!(solve(false, true, 1.0, false) > 0.01);
        assert!(solve(false, true, 0.0, false).abs() < 1e-6);
        assert!(solve(false, true, 1.0, true).abs() < 1e-6);
    }

    #[test]
    fn soft_limit_settings_cannot_soften_a_locked_axis() {
        let mut joint = joint();
        joint.angular_limits[0] = SourceAngularLimit::Locked;
        joint.twist_spring = Some(SourceAngularSpring {
            stiffness: 1.0,
            damping: 0.0,
            acceleration_mode: true,
        });
        let mut parent = SolverBody::default();
        let mut child = SolverBody::default();
        child.delta_rotation.0 = Quat::from_rotation_x(0.5);
        let mut data = SourcePhysicsJointSolverData::default();
        let child_inertia = inertia(1.0, Vec3::ONE);
        joint.solve(
            [&mut parent, &mut child],
            [&SolverBodyInertia::DUMMY, &child_inertia],
            &mut data,
            0.01,
        );
        assert!(child.delta_rotation.0.angle_between(Quat::IDENTITY) < 1e-5);
        joint.angular_limits[0] = SourceAngularLimit::Limited(0.1);
        child.delta_rotation.0 = Quat::from_rotation_x(0.5);
        joint.solve(
            [&mut parent, &mut child],
            [&SolverBodyInertia::DUMMY, &child_inertia],
            &mut data,
            0.01,
        );
        assert!(child.delta_rotation.0.angle_between(Quat::IDENTITY) > 0.49);
    }

    #[test]
    fn conditioning_is_per_joint_and_preserves_principal_axes() {
        let parent = inertia(100.0, Vec3::new(100.0, 10.0, 1.0));
        let child = inertia(0.1, Vec3::splat(0.1));
        let result = conditioned_inertias([&parent, &child], true, settings());
        assert!((result[0].effective_inv_mass().x - 0.5).abs() < 1e-6);
        assert_eq!(parent.effective_inv_mass(), Vec3::splat(100.0));
        let tensor = result[0].effective_inv_angular_inertia().inverse();
        assert!((tensor.m22 - 2.0).abs() < 1e-5);
        assert!((tensor.m00 - 0.4).abs() < 1e-5);
        assert!(tensor.m01.abs() + tensor.m02.abs() + tensor.m12.abs() < 1e-5);
        let unconditioned = conditioned_inertias([&parent, &child], false, settings());
        assert_eq!(
            unconditioned[0].effective_inv_mass(),
            parent.effective_inv_mass()
        );
        assert!(
            unconditioned[0]
                .effective_inv_mass()
                .distance(result[0].effective_inv_mass())
                > 1.0
        );
    }

    #[test]
    fn projection_moves_only_the_child_and_returns_the_source_velocity_fraction() {
        let joint = joint();
        let mut parent = SolverBody::default();
        let mut child = SolverBody::default();
        parent.delta_position = Vec3::new(3.0, 2.0, -4.0);
        child.delta_position = parent.delta_position + Vec3::new(0.02, -0.01, 0.01);
        let old_parent = parent.clone();
        let old_child = child.delta_position;
        let mut data = SourcePhysicsJointSolverData::default();
        project_joint(
            &joint,
            &mut parent,
            &mut child,
            &inertia(1.0, Vec3::ONE),
            &mut data,
            0.02,
        );
        assert!(child.delta_position.distance(parent.delta_position) < 1e-6);
        assert_eq!(parent.delta_position, old_parent.delta_position);
        assert_eq!(parent.delta_rotation, old_parent.delta_rotation);
        assert!(
            child
                .linear_velocity
                .distance((child.delta_position - old_child) * 5.0)
                < 1e-5
        );
    }

    #[test]
    fn swing_limit_axes_follow_the_runtime_basis() {
        let limits = [
            SourceAngularLimit::Free,
            SourceAngularLimit::Limited(65.0_f32.to_radians()),
            SourceAngularLimit::Limited(20.0_f32.to_radians()),
        ];
        assert!(
            angular_limit_correction(
                (Quat::IDENTITY, Quat::from_rotation_y(45.0_f32.to_radians())),
                limits,
                1
            )
            .is_none()
        );
        assert!(
            angular_limit_correction(
                (Quat::IDENTITY, Quat::from_rotation_z(45.0_f32.to_radians())),
                limits,
                1
            )
            .is_some()
        );
    }

    #[test]
    fn two_limited_swings_use_the_source_elliptical_cone() {
        let limits = [
            SourceAngularLimit::Free,
            SourceAngularLimit::Limited(65.0_f32.to_radians()),
            SourceAngularLimit::Limited(20.0_f32.to_radians()),
        ];
        let diagonal = Quat::from_rotation_y(55.0_f32.to_radians())
            * Quat::from_rotation_z(16.0_f32.to_radians());
        let swing = relative_swing(Quat::IDENTITY, diagonal).unwrap();
        let angles = Vec2::new(swing_angle(swing.y, swing.w), swing_angle(swing.z, swing.w));
        let bounds = Vec2::new(65.0_f32.to_radians(), 20.0_f32.to_radians());
        assert!(angles.abs().cmplt(bounds).all());
        assert!((angles / bounds).length_squared() > 1.0);
        assert!(angular_limit_correction((Quat::IDENTITY, diagonal), limits, 1).is_some());
        assert!(angular_limit_correction((Quat::IDENTITY, diagonal), limits, 2).is_none());
        let inside = Quat::from_rotation_y(30.0_f32.to_radians())
            * Quat::from_rotation_z(8.0_f32.to_radians());
        assert!(angular_limit_correction((Quat::IDENTITY, inside), limits, 1).is_none());
    }

    #[test]
    fn real_physics_schedule_preserves_joint_frames_and_acceleration_response() {
        use avian3d::prelude::{
            Gravity, LinearVelocity, Mass, PhysicsPlugins, Position, RigidBody, Rotation,
        };
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            TransformPlugin,
            PhysicsPlugins::new(PostUpdate),
            SourcePhysicsJointPlugin,
        ));
        app.insert_resource(Gravity(Vec3::ZERO));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )));
        app.finish();
        app.cleanup();
        let orientation = Quat::from_rotation_y(0.8);
        let mut cases = Vec::new();
        for (offset, mass) in [(0.0, 1.0), (5.0, 10.0)] {
            let parent_position = Vec3::new(4.0 + offset, 2.0, -3.0);
            let child_rotation = orientation * Quat::from_rotation_x(0.35);
            let parent_frame =
                Isometry3d::new(Vec3::new(0.1, -0.5, 0.0), Quat::from_rotation_y(0.2));
            let child_frame =
                Isometry3d::new(Vec3::new(-0.05, 0.5, 0.0), Quat::from_rotation_y(-0.3));
            let parent = app
                .world_mut()
                .spawn((
                    RigidBody::Kinematic,
                    Transform::from_translation(parent_position).with_rotation(orientation),
                ))
                .id();
            let child = app
                .world_mut()
                .spawn((
                    RigidBody::Dynamic,
                    Mass(mass),
                    AngularInertia {
                        principal: Vec3::splat(mass * 0.1),
                        local_frame: Quat::IDENTITY,
                    },
                    Transform::from_translation(
                        parent_position + orientation * Vec3::from(parent_frame.translation)
                            - child_rotation * Vec3::from(child_frame.translation),
                    )
                    .with_rotation(child_rotation),
                ))
                .id();
            let mut definition = joint();
            definition.parent = parent;
            definition.child = child;
            definition.parent_frame = parent_frame;
            definition.child_frame = child_frame;
            definition.linear_locked = true;
            definition.mass_conditioning = true;
            definition.settings.position_iterations = 6;
            definition.motor_target = parent_frame.rotation.conjugate() * child_frame.rotation;
            definition.motors = [SourceAngularMotor {
                spring: SourceAngularSpring {
                    stiffness: 37500.0,
                    damping: 0.0,
                    acceleration_mode: true,
                },
                position_enabled: true,
                velocity_enabled: false,
            }; 3];
            let joint_entity = app.world_mut().spawn(definition).id();
            cases.push((
                parent,
                child,
                joint_entity,
                parent_frame,
                child_frame,
                parent_position,
            ));
        }
        for _ in 0..12 {
            app.update();
        }
        let mut rotations = Vec::new();
        for (parent, child, _, parent_frame, child_frame, parent_position) in &cases {
            let world = app.world();
            let parent_anchor = world.get::<Position>(*parent).unwrap().0
                + world.get::<Rotation>(*parent).unwrap().0 * Vec3::from(parent_frame.translation);
            let child_anchor = world.get::<Position>(*child).unwrap().0
                + world.get::<Rotation>(*child).unwrap().0 * Vec3::from(child_frame.translation);
            assert!(
                parent_anchor.distance(child_anchor) < 1e-4,
                "anchor error: {}",
                parent_anchor.distance(child_anchor)
            );
            assert!(
                world
                    .get::<Position>(*parent)
                    .unwrap()
                    .0
                    .distance(*parent_position)
                    < 1e-6
            );
            rotations.push(world.get::<Rotation>(*child).unwrap().0);
        }
        assert!(rotations[0].angle_between(rotations[1]) < 1e-3);
        assert!(rotations[0].angle_between(orientation) < 0.35);
        let (parent, child, joint_entity, parent_frame, child_frame, _) = cases[0];
        app.world_mut()
            .entity_mut(joint_entity)
            .insert(JointDisabled);
        app.world_mut().get_mut::<LinearVelocity>(child).unwrap().0 = Vec3::X;
        for _ in 0..3 {
            app.update();
        }
        let parent_anchor = app.world().get::<Position>(parent).unwrap().0
            + app.world().get::<Rotation>(parent).unwrap().0 * Vec3::from(parent_frame.translation);
        let child_anchor = app.world().get::<Position>(child).unwrap().0
            + app.world().get::<Rotation>(child).unwrap().0 * Vec3::from(child_frame.translation);
        assert!(parent_anchor.distance(child_anchor) > 0.02);
    }

    #[test]
    fn real_physics_schedule_applies_a_free_linear_position_drive() {
        use avian3d::prelude::{Gravity, Mass, PhysicsPlugins, Position, RigidBody};
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, PhysicsPlugins::new(PostUpdate), SourcePhysicsJointPlugin));
        app.insert_resource(Gravity(Vec3::ZERO));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let parent = app.world_mut().spawn((RigidBody::Kinematic, Transform::IDENTITY)).id();
        let child = app.world_mut().spawn((RigidBody::Dynamic, Mass(1.0),
            AngularInertia { principal: Vec3::splat(0.1), local_frame: Quat::IDENTITY },
            Transform::IDENTITY)).id();
        let mut drive_joint = joint();
        drive_joint.parent = parent;
        drive_joint.child = child;
        drive_joint.linear_position_target = Vec3::X * 0.5;
        drive_joint.linear_motors[0] = SourceLinearMotor {
            stiffness: 2000.0, position_enabled: true, ..default()
        };
        app.world_mut().spawn(drive_joint);
        for _ in 0..8 { app.update(); }
        let position = app.world().get::<Position>(child).unwrap().0.x;
        assert!(position > 0.05 && position < 0.6, "position={position}");
    }

    #[test]
    fn real_physics_schedule_applies_a_free_swing_velocity_drive() {
        use avian3d::prelude::{Gravity, Mass, PhysicsPlugins, RigidBody, Rotation};
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::mesh::MeshPlugin,
            TransformPlugin, PhysicsPlugins::new(PostUpdate), SourcePhysicsJointPlugin));
        app.insert_resource(Gravity(Vec3::ZERO));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        let parent = app.world_mut().spawn((RigidBody::Kinematic, Transform::IDENTITY)).id();
        let child = app.world_mut().spawn((RigidBody::Dynamic, Mass(1.0),
            AngularInertia { principal: Vec3::splat(0.1), local_frame: Quat::IDENTITY },
            Transform::IDENTITY)).id();
        let mut drive_joint = joint();
        drive_joint.parent = parent;
        drive_joint.child = child;
        drive_joint.motor_velocity_target = Vec3::Y;
        drive_joint.motors[1] = SourceAngularMotor {
            spring: SourceAngularSpring { damping: 50.0, acceleration_mode: true, ..default() },
            velocity_enabled: true, ..default()
        };
        app.world_mut().spawn(drive_joint);
        for _ in 0..8 { app.update(); }
        let angle = app.world().get::<Rotation>(child).unwrap().0.to_scaled_axis().y;
        assert!(angle > 0.01, "angle={angle}");
    }
}

fn project_joint(
    joint: &SourcePhysicsJoint,
    parent: &mut SolverBody,
    child: &mut SolverBody,
    inertia_child: &SolverBodyInertia,
    data: &mut SourcePhysicsJointSolverData,
    dt: f32,
) {
    let previous_position = child.delta_position;
    let previous_rotation = child.delta_rotation.0;
    let separation = child.delta_position - parent.delta_position
        + data.center_difference
        + child.delta_rotation.0 * data.arms[1]
        - parent.delta_rotation.0 * data.arms[0];
    let parent_frame = parent.delta_rotation.0 * data.frames[0];
    if joint.projection_linear_tolerance_m > 0.0 {
        for axis in Vec3::AXES.map(|axis| parent_frame * axis) {
            let error = separation.dot(axis);
            if error.abs() > joint.projection_linear_tolerance_m {
                child.delta_position -= axis * error;
            }
        }
    }
    if joint.projection_linear_alpha > 0.0 {
        let inertia =
            AngularInertia::from_tensor(inertia_child.effective_inv_angular_inertia().inverse());
        let spherical_inverse = inertia.principal.max_element().recip();
        let projection_inertia = SolverBodyInertia::new(
            inertia_child.effective_inv_mass().max_element(),
            SymmetricMatrix::from_diagonal(Vec3::splat(spherical_inverse)),
            LockedAxes::default(),
            0,
            true,
        );
        for _ in 0..joint.settings.projection_iterations {
            let position = child.delta_position;
            let rotation = child.delta_rotation.0;
            data.point.solve(
                [parent, child],
                [&SolverBodyInertia::DUMMY, &projection_inertia],
                &mut data.point_data,
                dt,
            );
            child.delta_position =
                position.lerp(child.delta_position, joint.projection_linear_alpha);
            child.delta_rotation.0 =
                rotation.slerp(child.delta_rotation.0, joint.projection_linear_alpha);
        }
    }
    let velocity_scale = joint.settings.projection_velocity_alpha / dt;
    child.linear_velocity += (child.delta_position - previous_position) * velocity_scale;
    child.angular_velocity += shortest_quat(child.delta_rotation.0 * previous_rotation.conjugate())
        .to_scaled_axis()
        * velocity_scale;
}

pub struct SourcePhysicsJointPlugin;

impl Plugin for SourcePhysicsJointPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(JointGraphPlugin::<SourcePhysicsJoint>::default())
            .add_systems(
                PhysicsSchedule,
                prepare_xpbd_joint::<SourcePhysicsJoint>.in_set(SolverSystems::PrepareJoints),
            )
            .add_systems(
                PhysicsSchedule,
                project_source_joints.in_set(SolverSystems::PostSubstep),
            )
            .add_systems(
                SubstepSchedule,
                (capture_pre_solve_rotation, solve_source_joint_iterations)
                    .chain()
                    .in_set(XpbdSolverSystems::SolveUserConstraints),
            );
    }
}

#[derive(Clone, Copy)]
enum SwingAxis {
    One,
    Two,
}

impl SwingAxis {
    fn vector(self) -> Vec3 {
        match self {
            Self::One => Vec3::Y,
            Self::Two => Vec3::Z,
        }
    }

    fn other_vector(self) -> Vec3 {
        match self {
            Self::One => Vec3::Z,
            Self::Two => Vec3::Y,
        }
    }
}

fn elliptical_correction(
    frame_1: Quat,
    frame_2: Quat,
    swing_1_limit: f32,
    swing_2_limit: f32,
) -> Option<(Vec3, f32)> {
    if (swing_1_limit - swing_2_limit).abs() <= 1e-3 {
        return None;
    }
    if swing_1_limit <= 0.0 || swing_2_limit <= 0.0 {
        return None;
    }
    let swing = relative_swing(frame_1, frame_2)?;
    let angles = Vec2::new(swing_angle(swing.y, swing.w), swing_angle(swing.z, swing.w));
    let limits = Vec2::new(swing_1_limit, swing_2_limit);
    if (angles / limits).length_squared() <= 1.0 {
        return None;
    }
    let clamped = near_point_on_ellipse(angles, limits);
    let ellipse_normal = Vec3::new(
        0.0,
        clamped.x / limits.x.powi(2),
        clamped.y / limits.y.powi(2),
    );
    let swing_rotation_axis = Vec3::new(0.0, (clamped.x * 0.25).tan(), (clamped.y * 0.25).tan());
    let squared_length = swing_rotation_axis.length_squared();
    let a = 1.0 - squared_length;
    let b = 1.0 / (1.0 + squared_length);
    let b_squared = b * b;
    let v_1 = 2.0 * a * b_squared;
    let v_2 = Vec3::new(a, 2.0 * swing_rotation_axis.z, -2.0 * swing_rotation_axis.y);
    let radial_dot = swing_rotation_axis.dot(ellipse_normal);
    let derivative_v_1 = -4.0 * radial_dot * (3.0 - squared_length) * b_squared * b;
    let derivative_v_2 = Vec3::new(
        -2.0 * radial_dot,
        2.0 * ellipse_normal.z,
        -2.0 * ellipse_normal.y,
    );
    let line = v_1 * v_2 - Vec3::X;
    let normal = (v_1 * derivative_v_2 + derivative_v_1 * v_2).normalize_or_zero();
    if normal == Vec3::ZERO {
        return None;
    }
    let axis_local = line.cross(normal);
    let error = -line.cross(axis_local).dot(swing * Vec3::X);
    (error > 1e-6).then_some((frame_1 * axis_local, error))
}

fn locked_swing_correction(
    (frame_1, frame_2): (Quat, Quat),
    locked_axis: SwingAxis,
) -> Option<(Vec3, f32)> {
    let swing_0 = frame_1 * locked_axis.other_vector();
    let twist_1 = frame_2 * Vec3::X;
    let axis = swing_0.cross(twist_1);
    let error = -swing_0.dot(twist_1);
    (axis.length_squared() > 1e-12 && error.abs() > 1e-6).then_some((axis, error))
}

fn limited_swing_correction(
    (frame_1, frame_2): (Quat, Quat),
    swing_axis: SwingAxis,
    limit: f32,
) -> Option<(Vec3, f32)> {
    let swing = relative_swing(frame_1, frame_2)?;
    let component = match swing_axis {
        SwingAxis::One => swing.y,
        SwingAxis::Two => swing.z,
    };
    let angle = swing_angle(component, swing.w);
    let axis = frame_1 * swing_axis.vector();
    if angle > limit {
        Some((axis, angle - limit))
    } else if angle < -limit {
        Some((-axis, -angle - limit))
    } else {
        None
    }
}

fn relative_swing(frame_1: Quat, frame_2: Quat) -> Option<Quat> {
    let relative = (frame_1.conjugate() * frame_2).normalize();
    let twist = Quat::from_xyzw(relative.x, 0.0, 0.0, relative.w);
    (twist.length_squared() > 1e-12).then(|| (relative * twist.normalize().conjugate()).normalize())
}

fn swing_angle(component: f32, w: f32) -> f32 {
    4.0 * component.atan2(1.0 + w)
}

fn near_point_on_ellipse(point: Vec2, radii: Vec2) -> Vec2 {
    let point_absolute = point.abs();
    if radii.x >= radii.y && point_absolute.y < 1e-6 {
        return Vec2::new(radii.x.copysign(point.x), 0.0);
    }
    if radii.x < radii.y && point_absolute.x < 1e-6 {
        return Vec2::new(0.0, radii.y.copysign(point.y));
    }
    let radii_squared = radii * radii;
    let radii_point = radii * point_absolute;
    let mut value = (radii_point.x - radii_squared.x).max(radii_point.y - radii_squared.y);
    let mut divisor = Vec2::ONE;
    for _ in 0..20 {
        divisor = Vec2::new(
            1.0 / (value + radii_squared.x),
            1.0 / (value + radii_squared.y),
        );
        let scaled = radii_point * divisor;
        let function = scaled * scaled;
        let residual = function.x + function.y - 1.0;
        if residual < 1e-4 {
            return (radii_squared * point) * divisor;
        }
        let derivative = -2.0 * function.dot(divisor);
        value -= residual / derivative;
    }
    let clamped = (radii_squared * point) * divisor;
    clamped / (clamped / radii).length()
}
