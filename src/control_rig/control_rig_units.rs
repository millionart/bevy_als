use bevy::{
    math::{DQuat, DVec3},
};

use crate::extensions::{
    control_rig_hierarchy::{
        RigHierarchy, RigSpace, RigTransform, rig_normalize, rig_quat_near, rig_slerp,
    },
    control_rig_ik::{RigGeometry, source_vector_dot},
    control_rig_math::{
        RigValue, rig_clamp01, rig_damper_alpha, rig_euler, rig_quaternion, rig_spring_damper,
    },
};

pub type RigLineTrace<'a> =
    dyn FnMut(DVec3, DVec3, &str, bool) -> Result<Option<(DVec3, DVec3)>, String> + 'a;

pub struct RigUnitContext<'a> {
    pub hierarchy: &'a mut RigHierarchy,
    pub curves: &'a crate::extensions::SourceCurveValues,
    pub game_world: bool,
    pub construction: bool,
    pub delta_seconds: f64,
    pub geometry: &'a RigGeometry,
    pub to_world: RigTransform,
    pub line_trace: Option<&'a mut RigLineTrace<'a>>,
}

fn branch_label(condition: bool, label: &mut String) {
    *label = if label.is_empty() {
        if condition { "True" } else { "False" }
    } else {
        "Completed"
    }
    .to_owned();
}

fn constrained_angle(current: f64, target: f64, interval: &[RigValue]) -> Result<f64, String> {
    let [RigValue::F32(minimum), RigValue::F32(maximum)] = interval else {
        return Err("rig angle interval layout is invalid".to_owned());
    };
    let current = current as f32;
    Ok((target as f32).clamp(
        current.min(*minimum).min(*maximum),
        current.max(*minimum).max(*maximum),
    ) as f64)
}

impl RigUnitContext<'_> {
    fn chain_length(
        &mut self,
        ancestor: usize,
        descendant: usize,
        initial: bool,
    ) -> Result<Option<f32>, String> {
        if ancestor == descendant {
            return Ok(Some(0.0));
        }
        let Some(parent) = self.hierarchy.parent(descendant) else {
            return Ok(None);
        };
        let Some(length) = self.chain_length(ancestor, parent, initial)? else {
            return Ok(None);
        };
        let a = self
            .hierarchy
            .transform(parent, RigSpace::Global, initial)?
            .translation;
        let b = self
            .hierarchy
            .transform(descendant, RigSpace::Global, initial)?
            .translation;
        Ok(Some((length as f64 + a.distance(b)) as f32))
    }

    pub fn call(&mut self, function: &str, arguments: &mut [RigValue]) -> Result<(), String> {
        use RigValue::*;
        match (function, arguments) {
            (
                "FAlsRigUnit_FootOffsetTrace::Execute",
                [
                    Vector(foot),
                    Name(channel),
                    F32(upward),
                    F32(downward),
                    F32(walkable_angle),
                    F32(height),
                    Bool(enabled),
                    Bool(_debug),
                    F32(offset),
                    Vector(normal),
                ],
            ) => {
                let up = self.geometry.basis[2];
                let hit = if *enabled {
                    let horizontal = *foot - up * foot.dot(up);
                    let start = self
                        .to_world
                        .transform_point(horizontal + up * *upward as f64);
                    let end = self
                        .to_world
                        .transform_point(horizontal - up * *downward as f64);
                    let trace = self
                        .line_trace
                        .as_deref_mut()
                        .ok_or("rig line trace is unavailable")?;
                    trace(start, end, channel, true)?
                } else {
                    None
                };
                let Some((point, hit_normal)) = hit else {
                    *offset = 0.0;
                    *normal = up;
                    return Ok(());
                };
                let hit_normal = self.to_world.inverse_transform_vector(hit_normal);
                let slope_cos = hit_normal.dot(up);
                if slope_cos < walkable_angle.to_radians().cos() as f64 {
                    *offset = 0.0;
                    *normal = up;
                    return Ok(());
                }
                let point = self.to_world.inverse_transform_point(point);
                let slope_cos = slope_cos as f32;
                let correction = if slope_cos > 1e-8 {
                    *height / slope_cos - *height
                } else {
                    0.0
                };
                *offset = (point.dot(up) + correction as f64) as f32;
                *normal = hit_normal;
            }
            (
                "FAlsRigUnit_ApplyFootOffsetLocation::Execute",
                [
                    pelvis,
                    thigh,
                    Vector(target),
                    F32(foot_offset),
                    F32(pelvis_offset),
                    F32(length),
                    F32(pelvis_distance),
                    F32(stretch),
                    F32(frequency),
                    F32(damping),
                    F32(target_velocity_amount),
                    Bool(initialized),
                    _cached_pelvis,
                    _cached_thigh,
                    List(state),
                    F32(offset),
                    Vector(location),
                ],
            ) => {
                let Some(pelvis) = self.hierarchy.item_index(pelvis)? else {
                    return Ok(());
                };
                let Some(thigh) = self.hierarchy.item_index(thigh)? else {
                    return Ok(());
                };
                let [F32(velocity), F32(previous), Bool(valid)] = state.as_mut_slice() else {
                    return Err("rig offset spring state layout is invalid".to_owned());
                };
                let up = self.geometry.basis[2];
                let pelvis = self
                    .hierarchy
                    .transform(pelvis, RigSpace::Global, false)?
                    .translation
                    .dot(up) as f32;
                let thigh = self
                    .hierarchy
                    .transform(thigh, RigSpace::Global, false)?
                    .translation;
                let maximum = pelvis - *pelvis_distance - target.dot(up) as f32;
                let target_offset = foot_offset.min(maximum).max(*pelvis_offset);
                if !*initialized {
                    *initialized = true;
                    *velocity = 0.0;
                    *previous = 0.0;
                    *valid = false;
                    *offset = target_offset;
                } else {
                    let delta_seconds = self.delta_seconds as f32;
                    if delta_seconds > 1e-8 {
                        if !*valid {
                            *velocity = 0.0;
                            *previous = target_offset;
                            *valid = true;
                            *offset = target_offset;
                        } else {
                            let target_velocity = (target_offset - *previous)
                                * rig_clamp01(*target_velocity_amount)
                                / delta_seconds;
                            rig_spring_damper(
                                offset,
                                velocity,
                                target_offset,
                                target_velocity,
                                delta_seconds,
                                *frequency,
                                *damping,
                            );
                            *previous = target_offset;
                        }
                    }
                }
                let leg = *target + up * *offset as f64 - thigh;
                let maximum = (*length * *stretch) as f64;
                let leg = if maximum < self.geometry.near_zero {
                    DVec3::ZERO
                } else {
                    let squared = source_vector_dot(leg, leg);
                    if squared > maximum * maximum {
                        leg * (maximum * (1.0 / squared.sqrt()))
                    } else {
                        leg
                    }
                };
                *location = thigh + leg;
            }
            (
                "FAlsRigUnit_ApplyFootOffsetRotation::Execute",
                [
                    calf,
                    foot,
                    Quaternion(target),
                    Vector(normal),
                    List(swing_y),
                    List(swing_z),
                    List(twist_x),
                    F32(half_life),
                    Bool(initialized),
                    _cached_calf,
                    _cached_foot,
                    Vector(offset_normal),
                    Quaternion(rotation),
                    Quaternion(initial_rotation),
                ],
            ) => {
                let Some(calf) = self.hierarchy.item_index(calf)? else {
                    return Ok(());
                };
                let Some(foot) = self.hierarchy.item_index(foot)? else {
                    return Ok(());
                };
                if !*initialized {
                    *initialized = true;
                    *offset_normal = *normal;
                    let calf = self
                        .hierarchy
                        .transform(calf, RigSpace::Global, true)?
                        .rotation;
                    let foot = self
                        .hierarchy
                        .transform(foot, RigSpace::Global, true)?
                        .rotation;
                    *initial_rotation = calf.conjugate() * foot;
                }
                *offset_normal += (*normal - *offset_normal)
                    * rig_damper_alpha(self.delta_seconds as f32, *half_life) as f64;
                let offset_rotation = self
                    .geometry
                    .between_vectors(self.geometry.basis[2], *offset_normal);
                let calf = self
                    .hierarchy
                    .transform(calf, RigSpace::Global, false)?
                    .rotation;
                let foot = self
                    .hierarchy
                    .transform(foot, RigSpace::Global, false)?
                    .rotation;
                let current = rig_euler(calf.conjugate() * foot * initial_rotation.conjugate());
                let target = rig_euler(
                    calf.conjugate() * (offset_rotation * *target) * initial_rotation.conjugate(),
                );
                let euler = DVec3::new(
                    constrained_angle(current.x, target.x, twist_x)?,
                    constrained_angle(current.y, target.y, swing_y)?,
                    constrained_angle(current.z, target.z, swing_z)?,
                );
                *rotation = rig_normalize(calf * (rig_quaternion(euler) * *initial_rotation));
            }
            (
                "FAlsRigUnit_CalculatePoleVector::Execute",
                [
                    a,
                    b,
                    c,
                    Bool(initial),
                    Bool(success),
                    Vector(location),
                    Vector(projection),
                    Vector(direction),
                    _cached_a,
                    _cached_b,
                    _cached_c,
                ],
            ) => {
                if let Some(a) = self.hierarchy.item_index(a)?
                    && let Some(b) = self.hierarchy.item_index(b)?
                    && let Some(c) = self.hierarchy.item_index(c)?
                {
                    let a = self
                        .hierarchy
                        .transform(a, RigSpace::Global, *initial)?
                        .translation;
                    let b = self
                        .hierarchy
                        .transform(b, RigSpace::Global, *initial)?
                        .translation;
                    let c = self
                        .hierarchy
                        .transform(c, RigSpace::Global, *initial)?
                        .translation;
                    let pole = self.geometry.pole(a, b, c);
                    *success = pole.is_some();
                    if let Some((new_projection, new_direction)) = pole {
                        *location = b;
                        *projection = new_projection;
                        *direction = new_direction;
                    }
                }
            }
            (
                "FRigUnit_TwoBoneIKSimplePerItem::Execute",
                [
                    a,
                    b,
                    end,
                    effector,
                    Vector(primary),
                    Vector(secondary),
                    F32(secondary_weight),
                    Vector(pole),
                    Name(pole_kind),
                    pole_space,
                    Bool(stretch),
                    F32(stretch_start),
                    F32(stretch_max),
                    F32(weight),
                    F32(length_a),
                    F32(length_b),
                    Bool(propagate),
                    _debug,
                    _cached_a,
                    _cached_b,
                    _cached_end,
                    _cached_pole,
                ],
            ) => {
                let Some(a) = self.hierarchy.item_index(a)? else {
                    return Ok(());
                };
                let Some(b) = self.hierarchy.item_index(b)? else {
                    return Ok(());
                };
                let end = self.hierarchy.item_index(end)?;
                let pole_space = self.hierarchy.item_index(pole_space)?;
                if *weight <= 1e-8 {
                    return Ok(());
                }
                let mut pole = *pole;
                if let Some(space) = pole_space {
                    let transform = self.hierarchy.transform(space, RigSpace::Global, false)?;
                    pole = transform.rotation * pole;
                    if pole_kind != "Direction" {
                        pole += transform.translation;
                    }
                }
                let transform_a = self.hierarchy.transform(a, RigSpace::Global, false)?;
                let transform_b = self.hierarchy.transform(b, RigSpace::Global, false)?;
                let mut bones = [
                    transform_a,
                    RigTransform {
                        translation: transform_b.translation,
                        ..transform_a
                    },
                    RigTransform::from_value(effector)?,
                ];
                let mut lengths = [*length_a, *length_b];
                for (index, (parent, child)) in [(a, Some(b)), (b, end)].into_iter().enumerate() {
                    if lengths[index] < self.geometry.minimum_length
                        && let Some(child) = child
                    {
                        let initial = self.hierarchy.transform(parent, RigSpace::Global, true)?;
                        let scale = if initial.scale.length_squared() > 1e-8_f32 as f64 {
                            bones[index].scale / initial.scale
                        } else {
                            bevy::math::DVec3::ONE
                        };
                        let delta = initial.translation
                            - self
                                .hierarchy
                                .transform(child, RigSpace::Global, true)?
                                .translation;
                        lengths[index] = (delta * scale).length() as f32;
                    }
                }
                if lengths
                    .iter()
                    .any(|&length| length < self.geometry.minimum_length)
                {
                    return Ok(());
                }
                self.geometry.solve(
                    &mut bones,
                    pole,
                    *primary,
                    *secondary,
                    *secondary_weight,
                    lengths,
                    *stretch,
                    *stretch_start,
                    *stretch_max,
                );
                if *weight < 1.0 {
                    let joint = bones[0].inverse_transform_point(bones[1].translation);
                    let effector = bones[1].inverse_transform_point(bones[2].translation);
                    bones[0].rotation =
                        rig_slerp(transform_a.rotation, bones[0].rotation, *weight as f64);
                    bones[1].rotation =
                        rig_slerp(transform_b.rotation, bones[1].rotation, *weight as f64);
                    if let Some(end) = end {
                        let current = self.hierarchy.transform(end, RigSpace::Global, false)?;
                        bones[2].rotation =
                            rig_slerp(current.rotation, bones[2].rotation, *weight as f64);
                    }
                    bones[1].translation = bones[0].transform_point(joint);
                    bones[2].translation = bones[1].transform_point(effector);
                }
                for (index, transform) in [Some(a), Some(b), end].into_iter().zip(bones) {
                    if let Some(index) = index {
                        self.hierarchy.set_transform(
                            index,
                            RigSpace::Global,
                            false,
                            transform,
                            *propagate,
                        )?;
                    }
                }
            }
            (
                "FAlsRigVMFunction_DamperExactVector::Execute",
                [
                    Vector(target),
                    F32(half_life),
                    Vector(current),
                    Bool(initialized),
                ],
            ) => {
                if !*initialized {
                    *initialized = true;
                    *current = *target;
                }
                let alpha = rig_damper_alpha(self.delta_seconds as f32, *half_life);
                *current += (*target - *current) * alpha as f64;
            }
            (
                "FRigUnit_SpringInterpV2::Execute",
                [
                    F32(target),
                    F32(strength),
                    F32(damping),
                    F32(force),
                    Bool(use_current),
                    F32(current),
                    F32(target_velocity_amount),
                    Bool(initialize_from_target),
                    F32(result),
                    F32(velocity),
                    F32(simulated),
                    List(state),
                ],
            ) => {
                let [
                    F32(previous_target),
                    F32(state_velocity),
                    Bool(previous_valid),
                ] = state.as_mut_slice()
                else {
                    return Err("rig float spring state layout is invalid".to_owned());
                };
                let angular_frequency = *strength * 2.0 * std::f32::consts::PI;
                let stiffness = angular_frequency * angular_frequency;
                let mut adjusted_target = *target;
                if stiffness.abs() > 1e-8 {
                    adjusted_target += *force / stiffness;
                } else {
                    *state_velocity =
                        (*state_velocity as f64 + *force as f64 * self.delta_seconds) as f32;
                }
                let mut value = if *use_current { *current } else { *simulated };
                if (!*use_current || *initialize_from_target) && !*previous_valid {
                    value = adjusted_target;
                }
                let delta_seconds = self.delta_seconds as f32;
                if delta_seconds > 1e-8 {
                    let target_velocity = if *previous_valid {
                        (adjusted_target - *previous_target)
                            * (*target_velocity_amount / delta_seconds)
                    } else {
                        0.0
                    };
                    rig_spring_damper(
                        &mut value,
                        state_velocity,
                        adjusted_target,
                        target_velocity,
                        delta_seconds,
                        stiffness.sqrt() / (2.0 * std::f32::consts::PI),
                        *damping,
                    );
                    *previous_target = adjusted_target;
                    *previous_valid = true;
                }
                *simulated = value;
                *result = value;
                *velocity = *state_velocity;
            }
            ("FRigUnit_BeginExecution::Execute", []) => self.construction = false,
            ("FRigUnit_PrepareForExecution::Execute", []) => self.construction = true,
            ("FAlsRigVMFunction_IsGameWorld::Execute", [Name(label)]) => {
                branch_label(self.game_world, label)
            }
            ("FRigVMFunction_ControlFlowBranch::Execute", [Bool(condition), Name(label)]) => {
                branch_label(*condition, label)
            }
            (
                "DISPATCH_RigVMDispatch_If::Condition:bool,True:FRigElementKey,False:FRigElementKey,Result:FRigElementKey",
                [Bool(condition), List(a), List(b), List(output)],
            ) => {
                if a.len() != 2
                    || b.len() != 2
                    || !a
                        .iter()
                        .chain(b.iter())
                        .all(|field| matches!(field, Name(_)))
                {
                    return Err("rig selection element key is invalid".to_owned());
                }
                *output = if *condition { a.clone() } else { b.clone() };
            }
            (
                "FRigUnit_GetCurveValue::Execute",
                [Name(name), Bool(valid), F32(output), _cached],
            ) => {
                let value = self
                    .curves
                    .iter()
                    .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(*value));
                *valid = value.is_some();
                *output = value.unwrap_or(0.0);
            }
            (
                "FAlsRigUnit_ChainLength::Execute",
                [
                    ancestor,
                    descendant,
                    Bool(initial),
                    F32(output),
                    _cached_ancestor,
                    _cached_descendant,
                ],
            ) => {
                if let Some(ancestor) = self.hierarchy.item_index(ancestor)?
                    && let Some(descendant) = self.hierarchy.item_index(descendant)?
                {
                    *output =
                        if let Some(length) = self.chain_length(ancestor, descendant, *initial)? {
                            length
                        } else {
                            self.chain_length(descendant, ancestor, *initial)?
                                .unwrap_or(0.0)
                        };
                }
            }
            (
                "FAlsRigUnit_DistributeRotationSimple::Execute",
                [List(items), Quaternion(rotation), _cached],
            ) => {
                let tolerance = 1e-4_f32 as f64;
                if items.is_empty() || rig_quat_near(*rotation, DQuat::IDENTITY, tolerance) {
                    return Ok(());
                }
                let delta = rig_slerp(
                    DQuat::IDENTITY,
                    *rotation,
                    (1.0_f32 / items.len() as f32) as f64,
                );
                if rig_quat_near(delta, DQuat::IDENTITY, tolerance) {
                    return Ok(());
                }
                for item in items {
                    if let Some(index) = self.hierarchy.item_index(item)? {
                        let mut transform =
                            self.hierarchy.transform(index, RigSpace::Global, false)?;
                        transform.rotation = delta * transform.rotation;
                        self.hierarchy.set_transform(
                            index,
                            RigSpace::Global,
                            false,
                            transform,
                            true,
                        )?;
                    }
                }
            }
            (function, arguments) => {
                self.hierarchy
                    .call_transform_unit(function, arguments, self.construction)?
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy::platform::collections::HashMap;
    use super::*;
    use crate::extensions::{
        control_rig_hierarchy::{RigBone, RigTransform},
        control_rig_program::{RigInstruction, RigProgram, RigValueRef},
    };
    use bevy::math::DVec3;

    fn geometry() -> RigGeometry {
        RigGeometry {
            near_zero: 1e-4_f32 as f64 * 0.01,
            normalize_squared: 1e-8_f32 as f64 * 0.0001,
            minimum_length: 1e-8_f32 * 0.01,
            solve_near_zero: 1e-4 * 0.01,
            basis: [DVec3::X, DVec3::Z, DVec3::Y],
        }
    }

    #[test]
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    fn control_rig_pole_and_ik_preserve_source_vector_order() {
        let geometry = geometry();
        let (projection, direction) = geometry.pole(
            DVec3::ZERO, DVec3::new(0.5, 2.0, 1.0), DVec3::new(3.0, 5.0, 4.0),
        ).unwrap();
        assert_eq!(projection, DVec3::new(0.93, 1.55, 1.24));
        assert_eq!(direction, DVec3::new(-0.644_597_252_536_441, 0.674_578_520_096_275_3,
            -0.359_775_210_718_013_55));

        let (projection, direction) = geometry.pole(
            DVec3::new(8.925_669_401_805_589, 94.925_253_710_169_7, -1.717_870_670_449_023_2),
            DVec3::new(15.873_860_534_693_453, 53.219_259_420_003_425, 3.023_558_198_420_145_6),
            DVec3::new(18.975_329_039_394_797, 13.409_921_254_359_077, -1.599_313_650_113_666_4),
        ).unwrap();
        assert_eq!(projection, DVec3::new(14.095_286_683_713_653, 52.993_179_729_490_16,
            -1.656_884_085_497_952));
        assert_eq!(direction, DVec3::new(0.354_857_215_564_911_07, 0.045_106_931_841_553_17,
            0.933_831_741_408_140_3));

        let mut bones = [RigTransform::default(); 3];
        bones[0].translation = DVec3::new(-9.017_389_605_395_621, 94.903_783_166_794_78, -3.242_183_316_710_252_5);
        bones[2].translation = DVec3::new(-17.076_301_574_707_03, 13.465_195_894_241_333, -5.072_212_219_238_281);
        geometry.solve(&mut bones,
            DVec3::new(-35.484_576_782_270_3, 52.930_200_444_555, 33.857_362_981_432_14),
            DVec3::X, DVec3::Y, 0.0, [42.572_25, 40.196_82], false, 0.75, 1.25);
        assert_eq!(bones[1].translation,
            DVec3::new(-16.246_864_416_758_697, 53.175_769_941_310_58, 1.105_702_083_001_855_1));
    }

    #[test]
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    fn control_rig_foot_limit_preserves_source_reciprocal_order() {
        use RigValue::*;
        let mut hierarchy = hierarchy();
        for (index, translation) in [
            (0, DVec3::new(1.700_763_719_368_582_5e-28, 96.750_599_145_889_28, -3.056_153_282_523_155)),
            (1, DVec3::new(-9.005_802_624_670_832, 95.300_045_790_121, -2.529_951_274_917_837)),
        ] {
            hierarchy.set_transform(index, RigSpace::Global, false,
                RigTransform { translation, ..Default::default() }, true).unwrap();
        }
        let curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy, curves: &curves, game_world: true, construction: false,
            delta_seconds: 0.0, geometry: &geometry(), to_world: RigTransform::default(), line_trace: None,
        };
        let mut args = [
            item("Bone0"), item("Bone1"),
            Vector(DVec3::new(-17.076_301_574_707_03, 13.465_189_933_776_855, -5.072_212_219_238_281)),
            F32(5.960_464_5e-6), F32(5.960_464_5e-6), F32(82.768_73), F32(20.0), F32(0.99),
            F32(12.0), F32(2.0), F32(0.0), Bool(false), List(vec![]), List(vec![]),
            List(vec![F32(0.0), F32(0.0), Bool(false)]), F32(0.0), Vector(DVec3::ZERO),
        ];
        context.call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args).unwrap();
        assert_eq!(args[16], Vector(DVec3::new(-17.043_921_025_986_54, 13.793_534_620_321_694,
            -5.062_012_130_634_415_5)));
    }

    #[test]
    fn control_rig_foot_location_preserves_two_stage_spring_and_final_leg_limit() {
        use RigValue::*;
        let mut hierarchy = hierarchy();
        let input = [
            DVec3::Y * 2.0,
            DVec3::NEG_Y * 0.4,
            DVec3::NEG_Y,
            DVec3::ZERO,
        ]
        .map(|translation| RigTransform {
            translation,
            ..Default::default()
        });
        hierarchy.set_input_pose(&input).unwrap();
        let curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.0,
            geometry: &geometry(),
            to_world: RigTransform::default(),
            line_trace: None,
        };
        let mut args = [
            item("Bone0"),
            item("Bone1"),
            Vector(DVec3::ZERO),
            F32(3.0),
            F32(-0.2),
            F32(2.0),
            F32(0.5),
            F32(1.0),
            F32(1.0),
            F32(1.0),
            F32(0.0),
            Bool(false),
            List(vec![]),
            List(vec![]),
            List(vec![F32(8.0), F32(9.0), Bool(true)]),
            F32(9.0),
            Vector(DVec3::splat(9.0)),
        ];
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[11], Bool(true));
        assert_eq!(args[14], List(vec![F32(0.0), F32(0.0), Bool(false)]));
        assert_eq!(args[15], F32(1.5));
        assert_eq!(args[16], Vector(DVec3::Y * 1.5));
        args[3] = F32(0.4);
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[15], F32(1.5));
        context.delta_seconds = 0.1;
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[15], F32(0.4));
        assert_eq!(args[14], List(vec![F32(0.0), F32(0.4), Bool(true)]));
        args[3] = F32(0.0);
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        let expected = (0.4 + std::f32::consts::TAU * 0.4 * 0.1)
            * crate::als::animation::inv_exp_approx(std::f32::consts::TAU * 0.1);
        let F32(actual) = args[15] else {
            panic!("foot offset is missing");
        };
        assert!((actual - expected).abs() < 1e-7);
        context.delta_seconds = 0.0;
        args[5] = F32(0.5);
        args[7] = F32(0.8);
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        let Vector(location) = args[16] else {
            panic!("foot location is missing");
        };
        assert!(location.distance(DVec3::Y * (1.6 - (0.5_f32 * 0.8) as f64)) < 1e-12);
        assert_eq!(args[15], F32(actual));
        let mut unclamped = args.clone();
        unclamped[3] = F32(1.0);
        unclamped[10] = F32(1.0);
        args[3] = F32(1.0);
        args[10] = F32(2.0);
        context.delta_seconds = 0.1;
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        context
            .call(
                "FAlsRigUnit_ApplyFootOffsetLocation::Execute",
                &mut unclamped,
            )
            .unwrap();
        assert_eq!(&args[14..17], &unclamped[14..17]);
        args[11] = Bool(false);
        args[4] = F32(2.0);
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[15], F32(2.0));
        args[5] = F32(0.0);
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[16], Vector(DVec3::Y * 1.6));
        args[0] = item("Missing");
        args[11] = Bool(false);
        args[14] = List(vec![]);
        context
            .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[11], Bool(false));
        args[0] = item("Bone0");
        assert!(
            context
                .call("FAlsRigUnit_ApplyFootOffsetLocation::Execute", &mut args)
                .unwrap_err()
                .contains("spring state layout")
        );
    }

    #[test]
    fn control_rig_foot_trace_preserves_query_spaces_normals_and_slope_offset() {
        use RigValue::*;
        use std::cell::Cell;
        let to_world = RigTransform {
            rotation: DQuat::from_rotation_z(0.4),
            translation: DVec3::new(3.0, 4.0, 5.0),
            scale: DVec3::new(2.0, 0.5, 3.0),
        };
        let hit = Cell::new(Some((
            to_world.translation + to_world.rotation * DVec3::new(2.0, 0.1, 6.0),
            to_world.rotation * DVec3::new(0.0, 0.8, 0.6),
        )));
        let count = Cell::new(0);
        let mut query = |start: DVec3, end: DVec3, channel: &str, complex: bool| {
            if count.get() == 0 {
                assert!(
                    start.distance(
                        to_world.translation + to_world.rotation * DVec3::new(2.0, 0.25, 6.0)
                    ) < 1e-12
                );
                assert!(
                    end.distance(
                        to_world.translation
                            + to_world.rotation * DVec3::new(2.0, -0.8_f32 as f64 * 0.5, 6.0)
                    ) < 1e-12
                );
            }
            assert_eq!(channel, "Visibility");
            assert!(complex);
            count.set(count.get() + 1);
            Ok(hit.get())
        };
        let mut hierarchy = hierarchy();
        let curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.1,
            geometry: &geometry(),
            to_world,
            line_trace: Some(&mut query),
        };
        let args = [
            Vector(DVec3::new(1.0, 10.0, 2.0)),
            Name("Visibility".to_owned()),
            F32(0.5),
            F32(0.8),
            F32(45.0),
            F32(0.135),
            Bool(true),
            Bool(false),
            F32(9.0),
            Vector(DVec3::splat(9.0)),
        ];
        let program = RigProgram {
            initial_values: args.into(),
            literal_count: 0,
            entries: [("Trace".to_owned(), 0)].into(),
            instructions: vec![
                RigInstruction::Call {
                    function: "FAlsRigUnit_FootOffsetTrace::Execute".to_owned(),
                    arguments: (0..10)
                        .map(|register| RigValueRef {
                            register,
                            path: vec![],
                        })
                        .collect(),
                    outputs: vec![8, 9],
                },
                RigInstruction::Exit,
            ],
        };
        let mut values = program.initial_values.clone();
        let run = |context: &mut RigUnitContext, values: &mut Vec<RigValue>| {
            program
                .execute("Trace", values, |_, function, arguments| {
                    context.call(function, arguments)
                })
                .unwrap();
        };
        run(&mut context, &mut values);
        let F32(offset) = values[8] else {
            panic!("trace offset is missing");
        };
        let Vector(normal) = values[9] else {
            panic!("trace normal is missing");
        };
        assert!((offset - (0.2 + (0.135_f32 / 1.6 - 0.135) as f64) as f32).abs() < 1e-7);
        assert!(normal.distance(DVec3::new(0.0, 1.6, 0.2)) < 1e-12);
        hit.set(None);
        run(&mut context, &mut values);
        assert_eq!(values[8], F32(0.0));
        assert_eq!(values[9], Vector(DVec3::Y));
        context.to_world = RigTransform::default();
        let cosine = 45_f32.to_radians().cos() as f64;
        hit.set(Some((
            DVec3::ZERO,
            DVec3::new(0.0, cosine, (1.0 - cosine * cosine).sqrt()),
        )));
        run(&mut context, &mut values);
        let F32(offset) = values[8] else {
            panic!("trace offset is missing");
        };
        assert!(offset > 0.0);
        hit.set(Some((DVec3::ZERO, DVec3::new(0.0, cosine - 1e-6, 0.0))));
        run(&mut context, &mut values);
        assert_eq!(values[8], F32(0.0));
        assert_eq!(values[9], Vector(DVec3::Y));
        hit.set(Some((DVec3::Y * 0.25, DVec3::new(1.0, -1e-8, 0.0))));
        values[4] = F32(90.0);
        run(&mut context, &mut values);
        assert_eq!(values[8], F32(0.25));
        assert_eq!(values[9], Vector(DVec3::new(1.0, -1e-8, 0.0)));
        let before = count.get();
        values[6] = Bool(false);
        run(&mut context, &mut values);
        assert_eq!(count.get(), before);
        assert_eq!(values[8], F32(0.0));
        assert_eq!(values[9], Vector(DVec3::Y));
        context.line_trace = None;
        values[6] = Bool(true);
        let error = program
            .execute("Trace", &mut values, |_, function, arguments| {
                context.call(function, arguments)
            })
            .unwrap_err();
        assert_eq!(error.instruction, 0);
        assert_eq!(error.reason, "rig line trace is unavailable");
    }

    #[test]
    fn control_rig_foot_rotation_preserves_initial_space_animation_limits_and_normal_state() {
        use RigValue::*;
        let mut hierarchy = hierarchy();
        let calf = DQuat::from_rotation_y(0.7);
        let mut input = [RigTransform::default(); 4];
        input[1].rotation = calf;
        input[2].rotation = rig_quaternion(DVec3::new(12.0, -50.0, 45.0));
        hierarchy.set_input_pose(&input).unwrap();
        let curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.0,
            geometry: &geometry(),
            to_world: RigTransform::default(),
            line_trace: None,
        };
        let mut args = [
            item("Bone1"),
            item("Bone2"),
            Quaternion(calf * rig_quaternion(DVec3::new(30.0, -70.0, 60.0))),
            Vector(DVec3::Y),
            List(vec![F32(-40.0), F32(20.0)]),
            List(vec![F32(-15.0), F32(5.0)]),
            List(vec![F32(0.0), F32(0.0)]),
            F32(0.1),
            Bool(false),
            List(vec![]),
            List(vec![]),
            Vector(DVec3::ZERO),
            Quaternion(DQuat::IDENTITY),
            Quaternion(DQuat::from_rotation_x(1.0)),
        ];
        context
            .call("FAlsRigUnit_ApplyFootOffsetRotation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[8], Bool(true));
        assert_eq!(args[11], Vector(DVec3::Y));
        assert_eq!(args[13], Quaternion(DQuat::IDENTITY));
        let Quaternion(actual) = args[12] else {
            panic!("foot rotation is missing");
        };
        assert!(rig_quat_near(actual, calf * input[2].rotation, 1e-12));
        context
            .hierarchy
            .set_transform(
                2,
                RigSpace::Global,
                true,
                RigTransform {
                    rotation: DQuat::from_rotation_x(0.4),
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        args[3] = Vector(DVec3::Z);
        context.delta_seconds = 0.1;
        context
            .call("FAlsRigUnit_ApplyFootOffsetRotation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[13], Quaternion(DQuat::IDENTITY));
        let Vector(normal) = args[11] else {
            panic!("foot normal is missing");
        };
        assert_eq!(
            normal,
            DVec3::Y + (DVec3::Z - DVec3::Y) * 0.5001126527786255
        );
        assert!(normal.length() < 0.8);
        let inverse_reference = DQuat::from_rotation_x(0.4);
        context.delta_seconds = 0.0;
        args[8] = Bool(false);
        args[3] = Vector(DVec3::Y);
        args[2] = Quaternion(calf * input[2].rotation);
        for index in 4..7 {
            args[index] = List(vec![F32(-180.0), F32(180.0)]);
        }
        context
            .call("FAlsRigUnit_ApplyFootOffsetRotation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[13], Quaternion(inverse_reference));
        let Quaternion(actual) = args[12] else {
            panic!("foot rotation is missing");
        };
        assert!(rig_quat_near(actual, calf * input[2].rotation, 1e-6));
        context
            .hierarchy
            .set_transform(2, RigSpace::Global, true, RigTransform::default(), true)
            .unwrap();
        input[1].rotation = DQuat::IDENTITY;
        input[2].rotation = DQuat::IDENTITY;
        context.hierarchy.set_input_pose(&input).unwrap();
        args[2] = Quaternion(DQuat::IDENTITY);
        args[3] = Vector(DVec3::Z * 1e-8);
        args[8] = Bool(false);
        context
            .call("FAlsRigUnit_ApplyFootOffsetRotation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[12], Quaternion(DQuat::IDENTITY));
        assert_eq!(args[11], Vector(DVec3::Z * 1e-8));
        args[0] = item("Missing");
        args[8] = Bool(false);
        context
            .call("FAlsRigUnit_ApplyFootOffsetRotation::Execute", &mut args)
            .unwrap();
        assert_eq!(args[8], Bool(false));
    }

    #[test]
    fn control_rig_ik_units_preserve_pole_history_and_solve_current_hierarchy() {
        use RigValue::*;
        let mut hierarchy = hierarchy();
        let curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.0,
            geometry: &geometry(),
            to_world: RigTransform::default(),
            line_trace: None,
        };
        let mut pole = [
            item("Bone0"),
            item("Bone1"),
            item("Bone2"),
            Bool(true),
            Bool(false),
            Vector(DVec3::splat(9.0)),
            Vector(DVec3::splat(9.0)),
            Vector(DVec3::X),
            List(vec![]),
            List(vec![]),
            List(vec![]),
        ];
        context
            .call("FAlsRigUnit_CalculatePoleVector::Execute", &mut pole)
            .unwrap();
        assert_eq!(pole[4], Bool(true));
        assert_eq!(pole[5], Vector(DVec3::X));
        let Vector(projection) = pole[6] else {
            panic!("pole projection is missing");
        };
        assert!(projection.distance(DVec3::new(0.5, 0.5, 0.0)) < 1e-12);
        let Vector(direction) = pole[7] else {
            panic!("pole direction is missing");
        };
        assert!(direction.distance(DVec3::new(1.0, -1.0, 0.0).normalize()) < 1e-12);
        let remembered = pole[5..8].to_vec();
        context
            .hierarchy
            .set_input_pose(
                &[DVec3::ZERO, DVec3::X, DVec3::X, DVec3::Y].map(|translation| RigTransform {
                    translation,
                    ..Default::default()
                }),
            )
            .unwrap();
        pole[3] = Bool(false);
        context
            .call("FAlsRigUnit_CalculatePoleVector::Execute", &mut pole)
            .unwrap();
        assert_eq!(pole[4], Bool(false));
        assert_eq!(&pole[5..8], remembered);
        pole[4] = Bool(true);
        pole[0] = item("Missing");
        context
            .call("FAlsRigUnit_CalculatePoleVector::Execute", &mut pole)
            .unwrap();
        assert_eq!(pole[4], Bool(true));
        let effector = RigTransform {
            translation: DVec3::new(1.0, 1.0, 0.0),
            rotation: DQuat::from_rotation_y(0.4),
            ..Default::default()
        };
        let ik = [
            item("Bone0"),
            item("Bone1"),
            item("Bone2"),
            effector.value(),
            Vector(DVec3::X),
            Vector(DVec3::Y),
            F32(1.0),
            Vector(DVec3::Y * 2.0),
            Name("Location".to_owned()),
            item(""),
            Bool(false),
            F32(0.75),
            F32(1.25),
            F32(1.0),
            F32(1.0),
            F32(1.0),
            Bool(true),
            List(vec![]),
            List(vec![]),
            List(vec![]),
            List(vec![]),
            List(vec![]),
        ];
        let program = RigProgram {
            initial_values: ik.into(),
            literal_count: 0,
            entries: [("Solve".to_owned(), 0)].into(),
            instructions: vec![
                RigInstruction::Call {
                    function: "FRigUnit_TwoBoneIKSimplePerItem::Execute".to_owned(),
                    arguments: (0..22)
                        .map(|register| RigValueRef {
                            register,
                            path: vec![],
                        })
                        .collect(),
                    outputs: vec![],
                },
                RigInstruction::Exit,
            ],
        };
        let mut values = program.initial_values.clone();
        let run = |context: &mut RigUnitContext, values: &mut Vec<RigValue>| {
            assert_eq!(
                program
                    .execute("Solve", values, |_, function, arguments| context
                        .call(function, arguments))
                    .unwrap(),
                2
            );
        };
        run(&mut context, &mut values);
        let root = context
            .hierarchy
            .transform(0, RigSpace::Global, false)
            .unwrap();
        let joint = context
            .hierarchy
            .transform(1, RigSpace::Global, false)
            .unwrap();
        let end = context
            .hierarchy
            .transform(2, RigSpace::Global, false)
            .unwrap();
        assert!(joint.translation.distance(DVec3::Y) < 1e-12);
        assert!(end.translation.distance(effector.translation) < 1e-12);
        assert!(rig_quat_near(
            root.rotation,
            DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2),
            1e-12
        ));
        assert!(rig_quat_near(joint.rotation, DQuat::IDENTITY, 1e-12));
        assert!(rig_quat_near(end.rotation, effector.rotation, 1e-12));
        let input = [DVec3::ZERO, DVec3::X, DVec3::X, DVec3::Y].map(|translation| RigTransform {
            translation,
            ..Default::default()
        });
        for propagate in [true, false] {
            context.hierarchy.set_input_pose(&input).unwrap();
            values[13] = F32(0.5);
            values[16] = Bool(propagate);
            run(&mut context, &mut values);
            let root = context
                .hierarchy
                .transform(0, RigSpace::Global, false)
                .unwrap();
            let joint = context
                .hierarchy
                .transform(1, RigSpace::Global, false)
                .unwrap();
            let end = context
                .hierarchy
                .transform(2, RigSpace::Global, false)
                .unwrap();
            let child = context
                .hierarchy
                .transform(3, RigSpace::Global, false)
                .unwrap();
            let expected_joint = DVec3::new(1.0, 1.0, 0.0) * std::f64::consts::FRAC_1_SQRT_2;
            assert!(rig_quat_near(
                root.rotation,
                DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4),
                1e-12
            ));
            assert!(joint.translation.distance(expected_joint) < 1e-12);
            assert!(end.translation.distance(expected_joint + DVec3::X) < 1e-12);
            assert!(rig_quat_near(
                end.rotation,
                DQuat::from_rotation_y(0.2),
                1e-12
            ));
            let expected_child = if propagate {
                end.transform_point(DVec3::Y)
            } else {
                DVec3::new(2.0, 1.0, 0.0)
            };
            assert!(child.translation.distance(expected_child) < 1e-12);
        }
        context.hierarchy.set_input_pose(&input).unwrap();
        values[13] = F32(0.0);
        run(&mut context, &mut values);
        assert_eq!(
            context
                .hierarchy
                .transform(1, RigSpace::Local, false)
                .unwrap(),
            input[1]
        );
        values[13] = F32(1.0);
        values[14] = F32(0.0);
        values[15] = F32(0.0);
        run(&mut context, &mut values);
        assert!(
            context
                .hierarchy
                .transform(1, RigSpace::Global, false)
                .unwrap()
                .translation
                .distance(DVec3::Y)
                < 1e-12
        );
        context.hierarchy.set_input_pose(&input).unwrap();
        values[2] = item("Missing");
        run(&mut context, &mut values);
        assert_eq!(
            context
                .hierarchy
                .transform(1, RigSpace::Local, false)
                .unwrap(),
            input[1]
        );
        values[15] = F32(1.0);
        run(&mut context, &mut values);
        assert!(
            context
                .hierarchy
                .transform(1, RigSpace::Global, false)
                .unwrap()
                .translation
                .distance(DVec3::Y)
                < 1e-12
        );
        assert_eq!(
            context
                .hierarchy
                .transform(2, RigSpace::Global, false)
                .unwrap()
                .translation,
            DVec3::X * 2.0
        );
        values[0] = item("Missing");
        context.hierarchy.set_input_pose(&input).unwrap();
        run(&mut context, &mut values);
        assert_eq!(
            context
                .hierarchy
                .transform(1, RigSpace::Local, false)
                .unwrap(),
            input[1]
        );
        values = program.initial_values.clone();
        values[14] = F32(0.0);
        values[15] = F32(0.0);
        values[3] = RigTransform {
            translation: DVec3::X * 10.0,
            ..Default::default()
        }
        .value();
        let mut scaled = input;
        scaled[0].scale = DVec3::new(2.0, 3.0, 4.0);
        scaled[1].scale = DVec3::new(5.0, 6.0, 7.0);
        scaled[1].rotation = DQuat::from_rotation_x(0.8);
        context.hierarchy.set_input_pose(&scaled).unwrap();
        run(&mut context, &mut values);
        let joint = context
            .hierarchy
            .transform(1, RigSpace::Global, false)
            .unwrap();
        let end = context
            .hierarchy
            .transform(2, RigSpace::Global, false)
            .unwrap();
        assert!(joint.translation.distance(DVec3::X * 2.0) < 1e-12);
        assert!(end.translation.distance(DVec3::X * 5.0) < 1e-12);
        assert_eq!(joint.scale, scaled[0].scale);
        assert!(rig_quat_near(joint.rotation, DQuat::IDENTITY, 1e-12));
        let mut translated = input;
        translated[0].translation = DVec3::new(10.0, 2.0, -4.0);
        translated[0].rotation = DQuat::from_rotation_y(0.7);
        translated[0].scale = DVec3::new(2.0, 3.0, 4.0);
        let local_pole = DVec3::new(0.0, 2.0, 3.0);
        for kind in ["Direction", "Location"] {
            let world_pole = translated[0].rotation * local_pole
                + if kind == "Location" {
                    translated[0].translation
                } else {
                    DVec3::ZERO
                };
            values = program.initial_values.clone();
            values[3] = RigTransform {
                translation: translated[0].translation + DVec3::new(1.0, 0.2, 0.0),
                ..Default::default()
            }
            .value();
            values[7] = Vector(world_pole);
            context.hierarchy.set_input_pose(&translated).unwrap();
            run(&mut context, &mut values);
            let expected = [0, 1, 2].map(|index| {
                context
                    .hierarchy
                    .transform(index, RigSpace::Global, false)
                    .unwrap()
            });
            values[7] = Vector(local_pole);
            values[8] = Name(kind.to_owned());
            values[9] = item("Bone0");
            context.hierarchy.set_input_pose(&translated).unwrap();
            run(&mut context, &mut values);
            for (index, expected) in expected.into_iter().enumerate() {
                let actual = context
                    .hierarchy
                    .transform(index, RigSpace::Global, false)
                    .unwrap();
                assert!(actual.translation.distance(expected.translation) < 1e-12);
                assert!(rig_quat_near(actual.rotation, expected.rotation, 1e-12));
            }
        }
    }

    #[test]
    fn control_rig_ik_geometry_preserves_native_degenerate_axes_stretch_and_roll() {
        let geometry = geometry();
        let mut threshold_case = geometry.clone();
        threshold_case.normalize_squared = 1.0;
        assert!(
            threshold_case
                .pole(DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0), DVec3::X * 2.0)
                .is_none()
        );
        assert!(
            geometry
                .pole(DVec3::ZERO, DVec3::splat(0.8e-6), DVec3::Y)
                .is_none()
        );
        assert!(
            geometry
                .pole(DVec3::ZERO, DVec3::X, DVec3::X * 2.0)
                .is_none()
        );
        assert_eq!(
            geometry.pole(DVec3::ZERO, DVec3::Y, DVec3::X * 1e-8),
            Some((DVec3::ZERO, DVec3::Y))
        );
        assert!(
            geometry
                .pole(DVec3::ZERO, DVec3::X + DVec3::Y * 0.9e-6, DVec3::X * 2.0)
                .is_none()
        );
        assert!(
            geometry
                .pole(DVec3::ZERO, DVec3::X + DVec3::Y * 1.1e-6, DVec3::X * 2.0)
                .is_some()
        );
        let cases = [
            (
                DVec3::X * 3.0,
                DVec3::Y,
                [1.0, 1.0],
                false,
                DVec3::X,
                DVec3::X * 2.0,
            ),
            (
                DVec3::X * 3.0,
                DVec3::Y,
                [1.0, 1.0],
                true,
                DVec3::X * 1.25,
                DVec3::X * 2.5,
            ),
            (
                DVec3::X * 2.2,
                DVec3::Y,
                [1.0, 1.0],
                true,
                DVec3::new(1.1, (1.175_f64.powi(2) - 1.1_f64.powi(2)).sqrt(), 0.0),
                DVec3::X * 2.2,
            ),
            (
                DVec3::X * 0.5,
                DVec3::Y,
                [0.5, 2.0],
                false,
                DVec3::NEG_X * 0.5,
                DVec3::X * 0.5,
            ),
            (
                DVec3::X,
                DVec3::ZERO,
                [1.0, 1.0],
                false,
                DVec3::new(0.5, 0.0, 3.0_f64.sqrt() * 0.5),
                DVec3::X,
            ),
            (
                DVec3::X,
                DVec3::X * 2.0,
                [1.0, 1.0],
                false,
                DVec3::new(0.5, 0.0, 3.0_f64.sqrt() * 0.5),
                DVec3::X,
            ),
            (
                DVec3::Y,
                DVec3::Y * 2.0,
                [1.0, 1.0],
                false,
                DVec3::new(0.0, 0.5, -3.0_f64.sqrt() * 0.5),
                DVec3::Y,
            ),
        ];
        for (target, pole, lengths, stretch, joint, end) in cases {
            let offset = DVec3::new(5.0, -3.0, 7.0);
            let mut bones = [RigTransform {
                translation: offset,
                ..Default::default()
            }; 3];
            bones[2].translation += target;
            geometry.solve(
                &mut bones,
                pole + offset,
                DVec3::ZERO,
                DVec3::ZERO,
                0.0,
                lengths,
                stretch,
                0.75,
                1.25,
            );
            assert!(
                bones[1].translation.distance(joint + offset) < 1e-12,
                "{target:?}: {:?}",
                bones[1]
            );
            assert!(bones[2].translation.distance(end + offset) < 1e-12);
        }
        let mut bones = [RigTransform::default(); 3];
        geometry.solve(
            &mut bones,
            DVec3::Y,
            DVec3::ZERO,
            DVec3::ZERO,
            0.0,
            [1.0, 1.0],
            false,
            0.75,
            1.25,
        );
        assert!(bones[1].translation.x > 0.0 && bones[1].translation.x < 1e-6);
        assert!((bones[1].translation.y - 1.0).abs() < 1e-12);
        assert_eq!(bones[2].translation, DVec3::ZERO);
        for weight in [0.0, 0.25, 1.0, 2.0] {
            let mut bones = [RigTransform::default(); 3];
            bones[2].translation = DVec3::X * 3.0;
            geometry.solve(
                &mut bones,
                DVec3::Z,
                DVec3::X,
                DVec3::Y,
                weight,
                [1.0, 1.0],
                false,
                0.75,
                1.25,
            );
            let angle = if weight == 1.0 {
                std::f64::consts::FRAC_PI_2
            } else {
                (std::f32::consts::FRAC_PI_2 * weight.clamp(0.0, 1.0)) as f64
            };
            for bone in &bones[..2] {
                assert!(
                    rig_quat_near(bone.rotation, DQuat::from_rotation_x(angle), 1e-12),
                    "{weight}: {:?}",
                    bone.rotation
                );
            }
        }
        let mut bones = [RigTransform::default(); 3];
        bones[2].translation = DVec3::NEG_X * 3.0;
        geometry.solve(
            &mut bones,
            DVec3::Z,
            DVec3::X,
            DVec3::Y,
            0.0,
            [1.0, 1.0],
            false,
            0.75,
            1.25,
        );
        assert!(rig_quat_near(
            bones[0].rotation,
            DQuat::from_xyzw(0.0, -1.0, 0.0, 0.0),
            1e-12
        ));
        let mut bones = [RigTransform::default(); 3];
        bones[2].translation = DVec3::Y * 3.0;
        geometry.solve(
            &mut bones,
            DVec3::Z,
            DVec3::X * 2.0,
            DVec3::Y,
            0.0,
            [1.0, 1.0],
            false,
            0.75,
            1.25,
        );
        assert!(rig_quat_near(
            bones[0].rotation,
            DQuat::from_xyzw(0.0, 0.0, 2.0, 1.0).normalize(),
            1e-12
        ));
    }

    #[test]
    fn control_rig_filters_preserve_native_initialization_force_time_and_private_state() {
        use RigValue::*;
        let mut hierarchy = hierarchy();
        let curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.0,
            geometry: &geometry(),
            to_world: RigTransform::default(),
            line_trace: None,
        };
        let first = DVec3::new(1.0000000001, 2.5, -4.25);
        let next = DVec3::new(5.0000000001, -1.5, 3.75);
        let mut damper = [
            Vector(first),
            F32(0.1),
            Vector(DVec3::splat(999.0)),
            Bool(false),
        ];
        context
            .call("FAlsRigVMFunction_DamperExactVector::Execute", &mut damper)
            .unwrap();
        assert_eq!(damper[2], Vector(first));
        assert_eq!(damper[3], Bool(true));
        context.delta_seconds = 0.1;
        damper[0] = Vector(next);
        context
            .call("FAlsRigVMFunction_DamperExactVector::Execute", &mut damper)
            .unwrap();
        let expected = first + (next - first) * 0.5001126527786255;
        assert_eq!(damper[2], Vector(expected));
        context.delta_seconds = -0.05;
        damper[0] = Vector(DVec3::ZERO);
        context
            .call("FAlsRigVMFunction_DamperExactVector::Execute", &mut damper)
            .unwrap();
        let Vector(after) = damper[2] else {
            panic!("damper vector is missing");
        };
        assert!(after.x > expected.x);

        let program = RigProgram {
            initial_values: vec![
                F32(4.0),
                F32(1.0),
                F32(1.0),
                F32(0.0),
                Bool(false),
                F32(8.0),
                F32(0.0),
                Bool(false),
                F32(-1.0),
                F32(-1.0),
                F32(0.0),
                List(vec![List(vec![F32(0.0), F32(0.0), Bool(false)])]),
            ],
            literal_count: 0,
            entries: [("Solve".to_owned(), 0)].into(),
            instructions: vec![
                RigInstruction::Call {
                    function: "FRigUnit_SpringInterpV2::Execute".to_owned(),
                    arguments: (0..12)
                        .map(|register| RigValueRef {
                            register,
                            path: if register == 11 { vec![0] } else { vec![] },
                        })
                        .collect(),
                    outputs: vec![8, 9, 10, 11],
                },
                RigInstruction::Exit,
            ],
        };
        let mut values = program.initial_values.clone();
        let run = |context: &mut RigUnitContext, values: &mut Vec<RigValue>| {
            program
                .execute("Solve", values, |_, function, arguments| {
                    context.call(function, arguments)
                })
                .unwrap();
        };
        context.delta_seconds = 0.0;
        run(&mut context, &mut values);
        assert_eq!(values[8], F32(4.0));
        assert_eq!(values[10], F32(4.0));
        assert_eq!(values[11], program.initial_values[11]);
        values[0] = F32(6.0);
        run(&mut context, &mut values);
        assert_eq!(values[8], F32(6.0));
        context.delta_seconds = 0.25;
        run(&mut context, &mut values);
        assert_eq!(values[8], F32(6.0));
        assert_eq!(
            values[11],
            List(vec![List(vec![F32(6.0), F32(0.0), Bool(true)])])
        );
        values[0] = F32(1.0);
        values[4] = Bool(true);
        values[5] = F32(0.0);
        values[11] = List(vec![List(vec![F32(1.0), F32(0.2), Bool(true)])]);
        values[6] = F32(2.0);
        run(&mut context, &mut values);
        let F32(actual) = values[8] else {
            panic!("spring output is missing");
        };
        let e = crate::als::animation::inv_exp_approx(std::f32::consts::TAU * 0.25);
        let expected = 1.0 + (-1.0 + (0.2 - std::f32::consts::TAU) * 0.25) * e;
        assert!((actual - expected).abs() < 1e-6);
        let mut without_target_velocity = values.clone();
        values[0] = F32(3.0);
        without_target_velocity[0] = F32(3.0);
        without_target_velocity[6] = F32(1.0);
        run(&mut context, &mut values);
        run(&mut context, &mut without_target_velocity);
        let (F32(full), F32(clamped)) = (&values[8], &without_target_velocity[8]) else {
            panic!("spring output is missing");
        };
        assert!(full > clamped);

        let mut fresh = program.initial_values.clone();
        run(&mut context, &mut fresh);
        assert_eq!(fresh[8], F32(4.0));
        fresh[8] = F32(999.0);
        run(&mut context, &mut fresh);
        assert_eq!(fresh[8], F32(4.0));
        let mut tiny_step = program.initial_values.clone();
        tiny_step[4] = Bool(true);
        context.delta_seconds = 5e-8;
        run(&mut context, &mut tiny_step);
        let List(state) = &tiny_step[11] else {
            panic!("spring slice is missing");
        };
        let List(state) = &state[0] else {
            panic!("spring state is missing");
        };
        assert_eq!(state[0], F32(4.0));
        assert_eq!(state[2], Bool(true));
        context.delta_seconds = 0.0;
        fresh = program.initial_values.clone();
        fresh[4] = Bool(true);
        run(&mut context, &mut fresh);
        assert_eq!(fresh[8], F32(8.0));
        fresh[7] = Bool(true);
        run(&mut context, &mut fresh);
        assert_eq!(fresh[8], F32(4.0));
        fresh[3] = F32(std::f32::consts::TAU * std::f32::consts::TAU);
        run(&mut context, &mut fresh);
        assert_eq!(fresh[8], F32(5.0));

        fresh[1] = F32(0.0);
        fresh[3] = F32(7.0);
        fresh[7] = Bool(false);
        fresh[11] = List(vec![List(vec![F32(0.0), F32(0.2), Bool(true)])]);
        context.delta_seconds = 1.0 / 3.0;
        run(&mut context, &mut fresh);
        assert_eq!(fresh[9], F32((0.2_f32 as f64 + 7.0 / 3.0) as f32));
        fresh[11] = List(vec![List(vec![])]);
        let error = program
            .execute("Solve", &mut fresh, |_, function, arguments| {
                context.call(function, arguments)
            })
            .unwrap_err();
        assert_eq!(error.instruction, 0);
        assert!(error.reason.contains("spring state layout"));
    }

    fn item(name: &str) -> RigValue {
        RigValue::List(vec![
            RigValue::Name("Bone".to_owned()),
            RigValue::Name(name.to_owned()),
        ])
    }

    fn hierarchy() -> RigHierarchy {
        let positions = [
            DVec3::ZERO,
            DVec3::X,
            DVec3::X + DVec3::Y,
            DVec3::new(1.0, 1.0, 1.0),
        ];
        RigHierarchy::new(
            (0..4)
                .map(|index| RigBone {
                    name: format!("Bone{index}"),
                    parent: (index > 0).then(|| index - 1),
                    initial_local: RigTransform {
                        translation: positions[index]
                            - if index > 0 {
                                positions[index - 1]
                            } else {
                                DVec3::ZERO
                            },
                        ..Default::default()
                    },
                    initial_global: RigTransform {
                        translation: positions[index],
                        ..Default::default()
                    },
                })
                .collect(),
            1e-6,
        )
        .unwrap()
    }

    #[test]
    fn control_rig_units_preserve_events_branch_cycles_and_curve_validity_in_vm() {
        use RigValue::*;
        let mut hierarchy = hierarchy();
        let curves = [("Curve".into(), 0.75), ("Zero".into(), 0.0)].into();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.0,
            geometry: &geometry(),
            to_world: RigTransform::default(),
            line_trace: None,
        };
        let reference = |register| RigValueRef {
            register,
            path: vec![],
        };
        let call = |function: &str, indices: &[usize], outputs| RigInstruction::Call {
            function: function.to_owned(),
            arguments: indices.iter().map(|&i| reference(i)).collect(),
            outputs,
        };
        let program = RigProgram {
            initial_values: vec![
                Name("old".to_owned()),
                Name("cUrVe".to_owned()),
                Bool(false),
                F32(-1.0),
                List(vec![]),
                Bool(true),
                Name(String::new()),
                item("Bone0"),
                item("Bone3"),
                item("Missing"),
            ],
            literal_count: 0,
            entries: [
                ("Forwards Solve".to_owned(), 0),
                ("Construction".to_owned(), 9),
            ]
            .into(),
            instructions: vec![
                call("FRigUnit_BeginExecution::Execute", &[], vec![]),
                RigInstruction::Zero {
                    output: reference(0),
                },
                call("FAlsRigVMFunction_IsGameWorld::Execute", &[0], vec![0]),
                RigInstruction::Branch {
                    selector: reference(0),
                    branches: [
                        ("True".to_owned(), 4),
                        ("False".to_owned(), 6),
                        ("Completed".to_owned(), 8),
                    ]
                    .into(),
                },
                call(
                    "FRigUnit_GetCurveValue::Execute",
                    &[1, 2, 3, 4],
                    vec![1, 2, 3],
                ),
                RigInstruction::Jump { instruction: 2 },
                RigInstruction::Copy {
                    input: reference(8),
                    output: reference(9),
                },
                RigInstruction::Jump { instruction: 2 },
                RigInstruction::Exit,
                call("FRigUnit_PrepareForExecution::Execute", &[], vec![]),
                RigInstruction::Zero {
                    output: reference(6),
                },
                call(
                    "FRigVMFunction_ControlFlowBranch::Execute",
                    &[5, 6],
                    vec![1],
                ),
                RigInstruction::Branch {
                    selector: reference(6),
                    branches: [
                        ("True".to_owned(), 13),
                        ("False".to_owned(), 13),
                        ("Completed".to_owned(), 15),
                    ]
                    .into(),
                },
                call(
                    "DISPATCH_RigVMDispatch_If::Condition:bool,True:FRigElementKey,False:FRigElementKey,Result:FRigElementKey",
                    &[5, 7, 8, 9],
                    vec![3],
                ),
                RigInstruction::Jump { instruction: 11 },
                RigInstruction::Exit,
            ],
        };
        let mut values = program.initial_values.clone();
        for condition in [true, false] {
            values[5] = Bool(condition);
            program
                .execute("Construction", &mut values, |_, function, arguments| {
                    context.call(function, arguments)
                })
                .unwrap();
            assert!(context.construction);
            assert_eq!(values[9], item(if condition { "Bone0" } else { "Bone3" }));
            assert_eq!(values[6], Name("Completed".to_owned()));
            program
                .execute("Forwards Solve", &mut values, |_, function, arguments| {
                    context.call(function, arguments)
                })
                .unwrap();
            assert!(!context.construction);
            assert_eq!(values[2], Bool(true));
            assert_eq!(values[3], F32(0.75));
            assert_eq!(values[0], Name("Completed".to_owned()));
        }
        context.game_world = false;
        values[3] = F32(-1.0);
        program
            .execute("Forwards Solve", &mut values, |_, function, arguments| {
                context.call(function, arguments)
            })
            .unwrap();
        assert_eq!(values[3], F32(-1.0));
        assert_eq!(values[9], item("Bone3"));
        for (name, valid) in [("Zero", true), ("Unset", false)] {
            let mut arguments = [Name(name.to_owned()), Bool(!valid), F32(9.0), List(vec![])];
            context
                .call("FRigUnit_GetCurveValue::Execute", &mut arguments)
                .unwrap();
            assert_eq!(arguments[1], Bool(valid));
            assert_eq!(arguments[2], F32(0.0));
        }
        assert!(
            context
                .call("FRigUnit_BeginExecution::Execute", &mut [Bool(true)])
                .is_err()
        );
        assert!(
            context
                .call("FRigUnit_GetCurveValue::Execute", &mut [])
                .is_err()
        );
    }

    #[test]
    fn control_rig_bone_units_preserve_chain_rounding_and_ordered_global_rotation() {
        use RigValue::*;
        let mut hierarchy = hierarchy();
        let curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.0,
            geometry: &geometry(),
            to_world: RigTransform::default(),
            line_trace: None,
        };
        let mut length = [
            item("Bone0"),
            item("Bone3"),
            Bool(true),
            F32(-1.0),
            List(vec![]),
            List(vec![]),
        ];
        context
            .call("FAlsRigUnit_ChainLength::Execute", &mut length)
            .unwrap();
        assert_eq!(length[3], F32(3.0));
        length.swap(0, 1);
        context
            .call("FAlsRigUnit_ChainLength::Execute", &mut length)
            .unwrap();
        assert_eq!(length[3], F32(3.0));
        length[0] = item("Missing");
        context
            .call("FAlsRigUnit_ChainLength::Execute", &mut length)
            .unwrap();
        assert_eq!(length[3], F32(3.0));
        length[0] = item("Bone0");
        context
            .call("FAlsRigUnit_ChainLength::Execute", &mut length)
            .unwrap();
        assert_eq!(length[3], F32(0.0));
        let pose = [0.0, 16777216.0, 1.0, 1.0].map(|x| RigTransform {
            translation: DVec3::new(x, 0.0, 0.0),
            ..Default::default()
        });
        context.hierarchy.set_input_pose(&pose).unwrap();
        length[1] = item("Bone3");
        length[2] = Bool(false);
        context
            .call("FAlsRigUnit_ChainLength::Execute", &mut length)
            .unwrap();
        assert_eq!(length[3], F32(16777216.0));
        length[2] = Bool(true);
        context
            .call("FAlsRigUnit_ChainLength::Execute", &mut length)
            .unwrap();
        assert_eq!(length[3], F32(3.0));

        let pose = [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z].map(|translation| RigTransform {
            translation,
            rotation: DQuat::from_rotation_y(0.1),
            ..Default::default()
        });
        context.hierarchy.set_input_pose(&pose).unwrap();
        let mut rotation = [
            List(vec![item("Bone0"), item("Missing"), item("Bone1")]),
            Quaternion(DQuat::from_rotation_z(0.9)),
            List(vec![]),
        ];
        context
            .call(
                "FAlsRigUnit_DistributeRotationSimple::Execute",
                &mut rotation,
            )
            .unwrap();
        let delta = DQuat::from_rotation_z(0.9 * (1.0_f32 / 3.0) as f64);
        let root = context
            .hierarchy
            .transform(0, RigSpace::Global, false)
            .unwrap();
        let child = context
            .hierarchy
            .transform(1, RigSpace::Global, false)
            .unwrap();
        assert!(rig_quat_near(
            root.rotation,
            delta * pose[0].rotation,
            1e-12
        ));
        assert!(rig_quat_near(
            child.rotation,
            delta * delta * pose[0].rotation * pose[1].rotation,
            1e-12
        ));
        assert!((child.translation - (delta * pose[0].rotation) * DVec3::X).length() < 1e-12);
        let before = child;
        rotation[1] = Quaternion(-DQuat::IDENTITY);
        context
            .call(
                "FAlsRigUnit_DistributeRotationSimple::Execute",
                &mut rotation,
            )
            .unwrap();
        assert_eq!(
            context
                .hierarchy
                .transform(1, RigSpace::Global, false)
                .unwrap(),
            before
        );
        let mut pose = pose;
        pose[0].rotation = DQuat::from_xyzw(0.5, 0.5, 0.5, 0.5);
        context.hierarchy.set_input_pose(&pose).unwrap();
        let before = context
            .hierarchy
            .transform(0, RigSpace::Global, false)
            .unwrap();
        rotation[1] = Quaternion(DQuat::from_axis_angle(DVec3::ONE.normalize(), 0.000935307));
        context
            .call(
                "FAlsRigUnit_DistributeRotationSimple::Execute",
                &mut rotation,
            )
            .unwrap();
        assert_eq!(
            context
                .hierarchy
                .transform(0, RigSpace::Global, false)
                .unwrap(),
            before
        );
        let mut disjoint = RigHierarchy::new(
            ["First", "Second"]
                .map(|name| RigBone {
                    name: name.to_owned(),
                    parent: None,
                    initial_local: RigTransform::default(),
                    initial_global: RigTransform::default(),
                })
                .into(),
            1e-6,
        )
        .unwrap();
        context.hierarchy = &mut disjoint;
        length[0] = item("First");
        length[1] = item("Second");
        context
            .call("FAlsRigUnit_ChainLength::Execute", &mut length)
            .unwrap();
        assert_eq!(length[3], F32(0.0));
    }
}
