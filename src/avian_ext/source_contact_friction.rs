use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourcePositionBody {
    pub inverse_mass: f32,
    pub inverse_inertia: Mat3,
    pub arm: Vec3,
    pub dynamic: bool,
    pub delta_position: Vec3,
    pub delta_rotation: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourcePositionFriction {
    pub tangents: [Vec3; 2],
    pub initial_delta: Vec2,
    pub normal_pushout: f32,
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub net_pushout: Vec2,
    pub static_ratio: f32,
}

pub(crate) fn solve_source_position_friction(
    bodies: &mut [SourcePositionBody; 2],
    contact: &mut SourcePositionFriction,
    stiffness: f32,
) {
    let [mass_tolerance, impulse_tolerance]: [f32; 2] = include!("contact_friction_tolerances.rs");
    let normal_pushout = contact.normal_pushout.max(0.0);
    if stiffness <= 0.0 || (normal_pushout == 0.0 && contact.net_pushout == Vec2::ZERO) { return; }
    let mut delta = Vec2::ZERO;
    for axis in 0..2 {
        let tangent = contact.tangents[axis];
        let mut error = contact.initial_delta[axis];
        let mut inverse_mass = 0.0;
        for (index, body) in bodies.iter().enumerate() {
            if !body.dynamic { continue; }
            let cross = body.arm.cross(tangent);
            let sign = if index == 0 { 1.0 } else { -1.0 };
            error += sign * (body.delta_position.dot(tangent) + body.delta_rotation.dot(cross));
            inverse_mass += body.inverse_mass + cross.dot(body.inverse_inertia * cross);
        }
        let effective_mass = if inverse_mass > mass_tolerance { inverse_mass.recip() } else { 0.0 };
        delta[axis] = -stiffness * effective_mass * error;
    }
    let old = contact.net_pushout;
    let mut next = old + delta;
    contact.static_ratio = 1.0;
    if normal_pushout < impulse_tolerance {
        next = Vec2::ZERO;
        contact.static_ratio = 0.0;
    } else if next.length_squared() > (contact.static_friction * normal_pushout).powi(2) {
        contact.static_ratio = contact.dynamic_friction * normal_pushout / next.length();
        next *= contact.static_ratio;
    }
    contact.net_pushout = next;
    let applied = next - old;
    let impulse = contact.tangents[0] * applied.x + contact.tangents[1] * applied.y;
    for (index, body) in bodies.iter_mut().enumerate() {
        if !body.dynamic { continue; }
        let sign = if index == 0 { 1.0 } else { -1.0 };
        body.delta_position += sign * body.inverse_mass * impulse;
        body.delta_rotation += sign * (body.inverse_inertia * body.arm.cross(impulse));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(inverse_mass: f32, inverse_inertia: Mat3, arm: Vec3, dynamic: bool) -> SourcePositionBody {
        SourcePositionBody { inverse_mass, inverse_inertia, arm, dynamic,
            delta_position: Vec3::ZERO, delta_rotation: Vec3::ZERO }
    }

    fn point(delta: Vec2, normal_pushout: f32) -> SourcePositionFriction {
        SourcePositionFriction { tangents: [Vec3::X, Vec3::Z], initial_delta: delta,
            normal_pushout, static_friction: 0.7, dynamic_friction: 0.7,
            net_pushout: Vec2::ZERO, static_ratio: 0.0 }
    }

    #[test]
    fn source_position_friction_matches_native_head_single_axis() {
        let profile = crate::pa_als::parse_pa_als_profile();
        let native = profile.bodies.iter().find(|body| body.label == "head").unwrap().mass_properties.unwrap();
        let inverse_mass = native.mass_kg.recip();
        let inverse_inertia = native.inertia_tensor_kg_m2.inverse();
        let arm = Vec3::NEG_Y * 0.2;
        let angle = 0.5 * (2.0 * inverse_inertia.z_axis.x).atan2(inverse_inertia.x_axis.x - inverse_inertia.z_axis.z);
        let tangent = Vec3::new(angle.cos(), 0.0, angle.sin());
        let mut bodies = [body(inverse_mass, inverse_inertia, arm, true), body(0.0, Mat3::ZERO, Vec3::ZERO, false)];
        let mut contact = point(Vec2::new(1.0 / 60.0, 0.0), 1.0);
        contact.tangents = [tangent, Vec3::Y.cross(tangent)];
        for _ in 0..4 { solve_source_position_friction(&mut bodies, &mut contact, 0.5); }
        let cross = arm.cross(tangent);
        let inverse_effective_mass = inverse_mass + cross.dot(inverse_inertia * cross);
        let expected = -(15.0 / 16.0) * (1.0 / 60.0) / inverse_effective_mass;
        assert!((contact.net_pushout.x - expected).abs() < 1e-6);
        assert!(bodies[0].delta_position.abs_diff_eq(tangent * (inverse_mass * expected), 1e-6));
        assert!(bodies[0].delta_rotation.abs_diff_eq(inverse_inertia * cross * expected, 1e-6));
        assert_eq!(bodies[1].delta_position, Vec3::ZERO);
    }

    #[test]
    fn source_position_friction_clips_and_unwinds_coulomb_cone() {
        let mut bodies = [body(1.0, Mat3::IDENTITY, Vec3::NEG_Y, true), body(0.0, Mat3::ZERO, Vec3::ZERO, false)];
        let mut contact = point(Vec2::new(0.1, 0.0), 0.01);
        for _ in 0..4 { solve_source_position_friction(&mut bodies, &mut contact, 0.5); }
        assert!((contact.net_pushout.length() - 0.007).abs() < 1e-7);
        assert!(contact.static_ratio < 1.0);
        contact.normal_pushout = 0.0;
        solve_source_position_friction(&mut bodies, &mut contact, 0.5);
        assert_eq!(contact.net_pushout, Vec2::ZERO);
        assert!(bodies[0].delta_position.abs_diff_eq(Vec3::ZERO, 1e-7));
        assert!(bodies[0].delta_rotation.abs_diff_eq(Vec3::ZERO, 1e-7));
        assert_eq!(contact.static_ratio, 0.0);
    }

    #[test]
    fn source_position_friction_handles_two_bodies_and_independent_axes() {
        let mut bodies = [body(0.5, Mat3::ZERO, Vec3::ZERO, true), body(0.25, Mat3::ZERO, Vec3::ZERO, true)];
        let mut contact = point(Vec2::new(0.04, -0.02), 1.0);
        for _ in 0..4 { solve_source_position_friction(&mut bodies, &mut contact, 0.5); }
        assert!(contact.net_pushout.abs_diff_eq(Vec2::new(-0.05, 0.025), 1e-6));
        assert!(bodies[0].delta_position.abs_diff_eq(Vec3::new(-0.025, 0.0, 0.0125), 1e-6));
        assert!(bodies[1].delta_position.abs_diff_eq(Vec3::new(0.0125, 0.0, -0.00625), 1e-6));
        let mut immovable = [body(1e-8, Mat3::ZERO, Vec3::ZERO, true), body(0.0, Mat3::ZERO, Vec3::ZERO, false)];
        let mut zero = point(Vec2::ONE, 1.0);
        solve_source_position_friction(&mut immovable, &mut zero, 0.5);
        assert_eq!(zero.net_pushout, Vec2::ZERO);
        let mut frictionless = point(Vec2::ONE, 1.0);
        frictionless.static_friction = 0.0;
        frictionless.dynamic_friction = 0.0;
        solve_source_position_friction(&mut bodies, &mut frictionless, 0.5);
        assert_eq!(frictionless.net_pushout, Vec2::ZERO);
    }
}
