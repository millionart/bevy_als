use bevy::{math::DQuat, platform::collections::HashMap};

use crate::{
    control_rig_hierarchy::{RigHierarchy, RigSpace, rig_quat_near, rig_slerp},
    control_rig_math::{RigValue, rig_spring_damper},
};

pub struct RigUnitContext<'a> {
    pub hierarchy: &'a mut RigHierarchy,
    pub curves: &'a HashMap<String, f32>,
    pub game_world: bool,
    pub construction: bool,
    pub delta_seconds: f64,
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
