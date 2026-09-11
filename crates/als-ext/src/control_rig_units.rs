use bevy::{math::DQuat, platform::collections::HashMap};

use crate::{
    control_rig_hierarchy::{RigHierarchy, RigSpace, RigTransform, rig_quat_near, rig_slerp},
    control_rig_ik::RigGeometry,
    control_rig_math::{RigValue, rig_spring_damper},
};

pub struct RigUnitContext<'a> {
    pub hierarchy: &'a mut RigHierarchy,
    pub curves: &'a HashMap<String, f32>,
    pub game_world: bool,
    pub construction: bool,
    pub delta_seconds: f64,
    pub geometry: &'a RigGeometry,
}

fn branch_label(condition: bool, label: &mut String) {
    *label = if label.is_empty() {
        if condition { "True" } else { "False" }
    } else {
        "Completed"
    }
    .to_owned();
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
                let alpha = 1.0
                    - als_core::animation::inv_exp_approx(
                        std::f32::consts::LN_2 / (*half_life + 1e-8) * self.delta_seconds as f32,
                    );
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
    use super::*;
    use crate::{
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
                    .execute("Solve", values, |function, arguments| context
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
                .execute("Solve", values, |function, arguments| {
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
        let e = als_core::animation::inv_exp_approx(std::f32::consts::TAU * 0.25);
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
            .execute("Solve", &mut fresh, |function, arguments| {
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
        let curves = [("Curve".to_owned(), 0.75), ("Zero".to_owned(), 0.0)].into();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: true,
            construction: false,
            delta_seconds: 0.0,
            geometry: &geometry(),
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
                .execute("Construction", &mut values, |function, arguments| {
                    context.call(function, arguments)
                })
                .unwrap();
            assert!(context.construction);
            assert_eq!(values[9], item(if condition { "Bone0" } else { "Bone3" }));
            assert_eq!(values[6], Name("Completed".to_owned()));
            program
                .execute("Forwards Solve", &mut values, |function, arguments| {
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
            .execute("Forwards Solve", &mut values, |function, arguments| {
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
