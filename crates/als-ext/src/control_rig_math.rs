use als_core::animation::inv_exp_approx;
use bevy::{
    math::{DQuat, DVec3, EulerRot},
    prelude::Reflect,
};
use serde::{Deserialize, Serialize};

pub fn rig_damper_alpha(delta_seconds: f32, half_life: f32) -> f32 {
    1.0 - inv_exp_approx(std::f32::consts::LN_2 / (half_life + 1e-8) * delta_seconds)
}

pub fn rig_clamp01(value: f32) -> f32 {
    if value > 0.0 {
        if value < 1.0 { value } else { 1.0 }
    } else {
        0.0
    }
}

pub fn rig_quaternion(euler: DVec3) -> DQuat {
    DQuat::from_euler(
        EulerRot::YZX,
        (euler.y % 360.0).to_radians(),
        (euler.z % 360.0).to_radians(),
        (euler.x % 360.0).to_radians(),
    )
}

pub fn rig_euler(rotation: DQuat) -> DVec3 {
    let DQuat { x, y, z, w } = rotation;
    let singularity = x * y + w * z;
    if singularity.abs() > 0.4999995 {
        let sign = if singularity < 0.0 { -1.0 } else { 1.0 };
        let mut yaw = (-sign * 2.0 * x.atan2(w)).to_degrees() % 360.0;
        if yaw < 0.0 {
            yaw += 360.0;
        }
        if yaw > 180.0 {
            yaw -= 360.0;
        }
        DVec3::new(0.0, -yaw, sign * 90.0)
    } else {
        DVec3::new(
            (2.0 * (w * x - y * z))
                .atan2(1.0 - 2.0 * (x * x + z * z))
                .to_degrees(),
            -(2.0 * (x * z - w * y))
                .atan2(1.0 - 2.0 * (z * z + y * y))
                .to_degrees(),
            (2.0 * singularity).asin().to_degrees(),
        )
    }
}

pub fn rig_sin_cos(value: f32) -> (f32, f32) {
    let quotient = (std::f32::consts::FRAC_1_PI * 0.5) * value;
    let quotient = (if value >= 0.0 {
        quotient + 0.5
    } else {
        quotient - 0.5
    }) as i64 as f32;
    let mut y = value - std::f32::consts::TAU * quotient;
    let sign = if y > std::f32::consts::FRAC_PI_2 {
        y = std::f32::consts::PI - y;
        -1.0
    } else if y < -std::f32::consts::FRAC_PI_2 {
        y = -std::f32::consts::PI - y;
        -1.0
    } else {
        1.0
    };
    let y2 = y * y;
    let sine = (((((-2.3889859e-8 * y2 + 2.7525562e-6) * y2 - 0.00019840874) * y2 + 0.0083333310)
        * y2
        - 0.16666667)
        * y2
        + 1.0)
        * y;
    let cosine =
        ((((-2.6051615e-7 * y2 + 2.4760495e-5) * y2 - 0.0013888378) * y2 + 0.041666638) * y2 - 0.5)
            * y2
            + 1.0;
    (sine, sign * cosine)
}

pub fn rig_spring_damper(
    value: &mut f32,
    velocity: &mut f32,
    target: f32,
    target_velocity: f32,
    delta_seconds: f32,
    frequency: f32,
    damping: f32,
) {
    if delta_seconds <= 0.0 {
        return;
    }
    let w = frequency * std::f32::consts::TAU;
    if w < 1e-8 {
        *value += *velocity * delta_seconds;
        return;
    }
    if damping < 1e-8 {
        let error = *value - target;
        let b = *velocity / w;
        let (sine, cosine) = rig_sin_cos(w * delta_seconds);
        *value = target + error * cosine + b * sine;
        *velocity = *velocity * cosine - error * (w * sine);
        return;
    }
    let smoothing_time = 2.0 / w;
    let adjusted_target = target + target_velocity * (damping * smoothing_time);
    let error = *value - adjusted_target;
    if damping > 1.0 {
        let wd = w * (damping * damping - 1.0).sqrt();
        let c2 = -(*velocity + (w * damping - wd) * error) / (2.0 * wd);
        let c1 = error - c2;
        let a1 = wd - damping * w;
        let a2 = -(wd + damping * w);
        let e1 = inv_exp_approx(-a1 * delta_seconds);
        let e2 = inv_exp_approx(-a2 * delta_seconds);
        *value = adjusted_target + e1 * c1 + e2 * c2;
        *velocity = e1 * c1 * a1 + e2 * c2 * a2;
    } else if damping < 1.0 {
        let wd = w * (1.0 - damping * damping).sqrt();
        let b = (*velocity + error * (damping * w)) / wd;
        let (sine, cosine) = rig_sin_cos(wd * delta_seconds);
        let e = inv_exp_approx(damping * w * delta_seconds);
        *value = e * (error * cosine + b * sine);
        *velocity = -*value * damping * w;
        *velocity += e * (b * (wd * cosine) - error * (wd * sine));
        *value += adjusted_target;
    } else {
        let c2 = *velocity + error * w;
        let e = inv_exp_approx(w * delta_seconds);
        *value = adjusted_target + (error + c2 * delta_seconds) * e;
        *velocity = (c2 - error * w - c2 * (w * delta_seconds)) * e;
    }
}

#[derive(Reflect, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RigValue {
    Bool(bool),
    I32(i32),
    Name(String),
    List(Vec<RigValue>),
    F32(f32),
    F64(f64),
    Vector(DVec3),
    EulerDegrees(DVec3),
    Quaternion(DQuat),
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RigMathOp {
    F32Add,
    F32Mul,
    F32Min,
    F32Clamp01,
    F64Add,
    F64Sub,
    F64Mul,
    F64Abs,
    F64Clamp,
    F64Lerp,
    F64GreaterEqual,
    VectorAdd,
    VectorScale,
    QuaternionFromEulerYzx,
}

impl RigMathOp {
    pub fn evaluate(self, inputs: &[RigValue]) -> Result<RigValue, String> {
        use RigValue::*;
        Ok(match (self, inputs) {
            (Self::F32Add, [F32(a), F32(b)]) => F32(a + b),
            (Self::F32Mul, [F32(a), F32(b)]) => F32(a * b),
            (Self::F32Min, [F32(a), F32(b)]) => F32(if a < b { *a } else { *b }),
            (Self::F32Clamp01, [F32(value)]) => F32(rig_clamp01(*value)),
            (Self::F64Add, [F64(a), F64(b)]) => F64(a + b),
            (Self::F64Sub, [F64(a), F64(b)]) => F64(a - b),
            (Self::F64Mul, [F64(a), F64(b)]) => F64(a * b),
            (Self::F64Abs, [F64(value)]) => F64(value.abs()),
            (Self::F64Clamp, [F64(value), F64(minimum), F64(maximum)]) => {
                let upper = if value < maximum { *value } else { *maximum };
                F64(if *minimum < upper { upper } else { *minimum })
            }
            (Self::F64Lerp, [F64(a), F64(b), F64(t)]) => F64(a + t * (b - a)),
            (Self::F64GreaterEqual, [F64(a), F64(b)]) => Bool(a >= b),
            (Self::VectorAdd, [Vector(a), Vector(b)]) => Vector(*a + *b),
            (Self::VectorScale, [Vector(value), F32(factor)]) => Vector(*value * *factor as f64),
            (Self::QuaternionFromEulerYzx, [EulerDegrees(value)]) => {
                Quaternion(rig_quaternion(*value))
            }
            _ => {
                return Err(format!(
                    "control rig {self:?} input types or arity do not match"
                ));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RigMathOp::*;

    #[test]
    fn control_rig_euler_preserves_double_singularities_and_target_angles() {
        for value in [
            DVec3::new(21.0, 83.0, 37.0),
            DVec3::new(30.0, -70.0, 89.9),
            DVec3::new(30.0, -70.0, -89.9),
        ] {
            assert!(rig_euler(rig_quaternion(value)).distance(value) < 1e-8);
        }
        for (input, expected) in [
            (DVec3::new(30.0, -70.0, 90.0), DVec3::new(0.0, -40.0, 90.0)),
            (
                DVec3::new(30.0, -70.0, -90.0),
                DVec3::new(0.0, -100.0, -90.0),
            ),
        ] {
            let quaternion = rig_quaternion(input);
            for quaternion in [quaternion, -quaternion] {
                assert!(rig_euler(quaternion).distance(expected) < 1e-12);
            }
        }
        for pitch in [-89.95, 89.95] {
            let actual = rig_euler(rig_quaternion(DVec3::new(30.0, -70.0, pitch)));
            assert_eq!(actual.x, 0.0);
            assert_eq!(actual.z, 90.0_f64.copysign(pitch));
        }
        let actual = rig_euler(DQuat::from_xyzw(0.0, 0.0, 0.1, 1.0));
        assert_eq!(actual, DVec3::new(0.0, 0.0, 0.2_f64.asin().to_degrees()));
        assert_eq!(
            rig_euler(DQuat::from_xyzw(
                std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
                0.0,
                0.0
            )),
            DVec3::new(0.0, -180.0, 90.0)
        );
    }
    use RigValue::*;

    #[test]
    fn control_rig_spring_math_matches_native_float_branches() {
        for (frequency, damping, dt, expected, expected_velocity) in [
            (1.0, 0.0, 0.25, 1.03183114528656, 6.28318452835083),
            (1.0, 0.5, 0.25, 0.6947076320648193, 3.3561625480651855),
            (1.0, 1.0, 0.25, 0.5058176517486572, 2.2804200649261475),
            (1.0, 2.0, 0.25, 0.35566431283950806, 1.3820037841796875),
            (0.0, 1.0, 0.25, 0.05, 0.2),
            (-1.0, 1.0, 0.25, 0.05, 0.2),
            (1.0, -1.0, 0.25, 1.03183114528656, 6.28318452835083),
            (1.0, 1.0, 0.0, 0.0, 0.2),
            (1.0, 1.0, -0.25, 0.0, 0.2),
        ] {
            let (mut value, mut velocity) = (0.0, 0.2);
            rig_spring_damper(&mut value, &mut velocity, 1.0, 0.3, dt, frequency, damping);
            assert!(
                (value - expected).abs() < 2e-6,
                "{frequency} {damping} {dt}: {value}"
            );
            assert!(
                (velocity - expected_velocity).abs() < 2e-6,
                "{frequency} {damping} {dt}: {velocity}"
            );
        }
        for (value, sine, cosine) in [
            (0.0, 0.0, 1.0),
            (1.0, 0.8414709568023682, 0.5403022766113281),
            (-1.0, -0.8414709568023682, 0.5403022766113281),
            (100.0, -0.5063680410385132, 0.8623174428939819),
            (-100.0, 0.5063680410385132, 0.8623174428939819),
            (1000.0, 0.8268652558326721, 0.5624001026153564),
        ] {
            let actual = rig_sin_cos(value);
            assert!(
                (actual.0 - sine).abs() < 1e-7 && (actual.1 - cosine).abs() < 1e-7,
                "{value}: {actual:?}"
            );
        }
    }

    #[test]
    fn math_preserves_native_precision_boundaries_and_target_axes() {
        let cases = [
            (F32Add, vec![F32(16_777_216.0), F32(1.0)], F32(16_777_216.0)),
            (F32Mul, vec![F32(-0.25), F32(3.0)], F32(-0.75)),
            (F32Min, vec![F32(-0.25), F32(-0.5)], F32(-0.5)),
            (F32Min, vec![F32(f32::NAN), F32(2.0)], F32(2.0)),
            (F32Clamp01, vec![F32(-0.25)], F32(0.0)),
            (F32Clamp01, vec![F32(0.25)], F32(0.25)),
            (F32Clamp01, vec![F32(1.25)], F32(1.0)),
            (F32Clamp01, vec![F32(f32::NAN)], F32(0.0)),
            (F64Add, vec![F64(16_777_216.0), F64(1.0)], F64(16_777_217.0)),
            (F64Sub, vec![F64(2.0), F64(3.0)], F64(-1.0)),
            (F64Mul, vec![F64(-0.5), F64(3.0)], F64(-1.5)),
            (F64Abs, vec![F64(-1.5)], F64(1.5)),
            (F64Clamp, vec![F64(0.2), F64(0.3), F64(0.6)], F64(0.3)),
            (F64Clamp, vec![F64(0.9), F64(0.3), F64(0.6)], F64(0.6)),
            (F64Clamp, vec![F64(0.4), F64(0.6), F64(0.3)], F64(0.6)),
            (F64Clamp, vec![F64(f64::NAN), F64(0.3), F64(0.6)], F64(0.6)),
            (F64Lerp, vec![F64(1.0), F64(3.0), F64(1.5)], F64(4.0)),
            (F64Lerp, vec![F64(1.0), F64(3.0), F64(-0.5)], F64(0.0)),
            (F64GreaterEqual, vec![F64(0.5), F64(0.5)], Bool(true)),
            (F64GreaterEqual, vec![F64(0.4), F64(0.5)], Bool(false)),
            (
                VectorAdd,
                vec![
                    Vector(DVec3::new(1.0, 3.0, 2.0)),
                    Vector(DVec3::new(0.4, 0.0, 0.4)),
                ],
                Vector(DVec3::new(1.4, 3.0, 2.4)),
            ),
            (
                VectorScale,
                vec![Vector(DVec3::new(1.0, 3.0, 2.0)), F32(-0.5)],
                Vector(DVec3::new(-0.5, -1.5, -1.0)),
            ),
        ];
        for (operation, inputs, expected) in cases {
            assert_eq!(
                operation.evaluate(&inputs).unwrap(),
                expected,
                "{operation:?}"
            );
            let restored: RigMathOp =
                serde_json::from_str(&serde_json::to_string(&operation).unwrap()).unwrap();
            assert_eq!(restored, operation);
            assert!(operation.evaluate(&[]).is_err());
        }
        for inputs in [
            vec![F64(1.0), F64(2.0)],
            vec![F32(1.0)],
            vec![F32(1.0), F32(2.0), F32(3.0)],
        ] {
            assert!(F32Add.evaluate(&inputs).is_err());
        }
        assert!(
            QuaternionFromEulerYzx
                .evaluate(&[Vector(DVec3::ZERO)])
                .is_err()
        );
        for source in [
            DVec3::ZERO,
            DVec3::new(90.0, 0.0, 0.0),
            DVec3::new(0.0, 90.0, 0.0),
            DVec3::new(0.0, 0.0, 90.0),
            DVec3::new(37.0, -83.0, 21.0),
            DVec3::new(397.0, -83.0, 21.0),
            DVec3::new(757.0, -443.0, 381.0),
        ] {
            let (sp, cp) = ((source.x % 360.0).to_radians() * 0.5).sin_cos();
            let (sy, cy) = ((source.y % 360.0).to_radians() * 0.5).sin_cos();
            let (sr, cr) = ((source.z % 360.0).to_radians() * 0.5).sin_cos();
            let expected = DQuat::from_xyzw(
                -(cr * sp * sy - sr * cp * cy),
                -(cr * cp * sy - sr * sp * cy),
                -(-cr * sp * cy - sr * cp * sy),
                cr * cp * cy + sr * sp * sy,
            );
            let input = EulerDegrees(DVec3::new(source.z, -source.y, source.x));
            let Quaternion(actual) = QuaternionFromEulerYzx.evaluate(&[input.clone()]).unwrap()
            else {
                panic!("rotation output type");
            };
            assert!(
                actual.abs_diff_eq(expected, 1e-12),
                "{source:?}: {actual:?} != {expected:?}"
            );
            let round_trip: RigValue =
                serde_json::from_str(&serde_json::to_string(&input).unwrap()).unwrap();
            assert_eq!(round_trip, input);
        }
    }
}
