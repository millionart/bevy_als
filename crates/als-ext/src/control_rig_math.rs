use bevy::{
    math::{DQuat, DVec3, EulerRot},
    prelude::Reflect,
};
use serde::{Deserialize, Serialize};

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
            (Self::F32Clamp01, [F32(value)]) => F32(if *value > 0.0 {
                if *value < 1.0 { *value } else { 1.0 }
            } else {
                0.0
            }),
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
            (Self::QuaternionFromEulerYzx, [EulerDegrees(value)]) => Quaternion(DQuat::from_euler(
                EulerRot::YZX,
                (value.y % 360.0).to_radians(),
                (value.z % 360.0).to_radians(),
                (value.x % 360.0).to_radians(),
            )),
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
    use RigValue::*;

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
