use std::{collections::BTreeMap, fmt};

use bevy::prelude::Reflect;
use serde::{Deserialize, Serialize};

use crate::extensions::{
    control_rig_hierarchy::RigBone,
    control_rig_ik::RigGeometry,
    control_rig_math::{RigMathOp, RigValue},
};

#[derive(Reflect, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RigValueRef {
    pub register: usize,
    pub path: Vec<usize>,
}

fn read_field(value: &RigValue, path: &[usize]) -> Result<RigValue, String> {
    let Some((&index, rest)) = path.split_first() else {
        return Ok(value.clone());
    };
    match value {
        RigValue::List(values) => {
            read_field(values.get(index).ok_or("rig list index is invalid")?, rest)
        }
        RigValue::Vector(value) | RigValue::EulerDegrees(value) if index < 3 && rest.is_empty() => {
            Ok(RigValue::F64(value[index]))
        }
        _ => Err("rig value field path is invalid".to_owned()),
    }
}

fn write_field(target: &mut RigValue, path: &[usize], value: RigValue) -> Result<(), String> {
    if let Some((&index, rest)) = path.split_first() {
        match target {
            RigValue::List(values) => {
                return write_field(
                    values.get_mut(index).ok_or("rig list index is invalid")?,
                    rest,
                    value,
                );
            }
            RigValue::Vector(target) | RigValue::EulerDegrees(target)
                if index < 3 && rest.is_empty() =>
            {
                target[index] = match value {
                    RigValue::F64(value) => value,
                    RigValue::F32(value) => value as f64,
                    _ => return Err("rig vector component type is invalid".to_owned()),
                };
                return Ok(());
            }
            _ => return Err("rig value field path is invalid".to_owned()),
        }
    }
    match (&mut *target, value) {
        (RigValue::F32(target), RigValue::F64(value)) => *target = value as f32,
        (RigValue::F64(target), RigValue::F32(value)) => *target = value as f64,
        (target, value) if std::mem::discriminant(target) == std::mem::discriminant(&value) => {
            *target = value
        }
        _ => return Err("rig value assignment types do not match".to_owned()),
    }
    Ok(())
}

impl RigValueRef {
    pub fn read(&self, values: &[RigValue]) -> Result<RigValue, String> {
        read_field(
            values.get(self.register).ok_or("rig register is invalid")?,
            &self.path,
        )
    }

    pub fn write(
        &self,
        values: &mut [RigValue],
        literal_count: usize,
        value: RigValue,
    ) -> Result<(), String> {
        if self.register < literal_count {
            return Err("rig literal memory is immutable".to_owned());
        }
        write_field(
            values
                .get_mut(self.register)
                .ok_or("rig register is invalid")?,
            &self.path,
            value,
        )
    }
}

#[derive(Reflect, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RigInstruction {
    Math {
        operation: RigMathOp,
        inputs: Vec<RigValueRef>,
        output: RigValueRef,
    },
    Call {
        function: String,
        arguments: Vec<RigValueRef>,
        outputs: Vec<usize>,
    },
    Copy {
        input: RigValueRef,
        output: RigValueRef,
    },
    Zero {
        output: RigValueRef,
    },
    Jump {
        instruction: usize,
    },
    Branch {
        selector: RigValueRef,
        branches: BTreeMap<String, usize>,
    },
    Exit,
}

#[derive(Reflect, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RigProgram {
    pub initial_values: Vec<RigValue>,
    pub literal_count: usize,
    pub entries: BTreeMap<String, usize>,
    pub instructions: Vec<RigInstruction>,
}

#[derive(Reflect, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigDefinition {
    pub program: RigProgram,
    pub externals: BTreeMap<String, RigValueRef>,
    pub bones: Vec<RigBone>,
    pub curves: BTreeMap<String, Option<f32>>,
    pub translation_tolerance: f64,
    pub geometry: RigGeometry,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RigExecutionError {
    pub entry: String,
    pub instruction: usize,
    pub executed_steps: usize,
    pub reason: String,
}

impl fmt::Display for RigExecutionError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "rig entry {} stopped at instruction {} after {} steps: {}",
            self.entry, self.instruction, self.executed_steps, self.reason
        )
    }
}

impl std::error::Error for RigExecutionError {}

fn named_instruction(entries: &BTreeMap<String, usize>, name: &str) -> Option<usize> {
    entries
        .iter()
        .find_map(|(key, &index)| key.eq_ignore_ascii_case(name).then_some(index))
}

impl RigProgram {
    pub fn execute(
        &self,
        entry: &str,
        values: &mut [RigValue],
        mut call_unit: impl FnMut(usize, &str, &mut [RigValue]) -> Result<(), String>,
    ) -> Result<usize, RigExecutionError> {
        let failure = |instruction, executed_steps, reason| RigExecutionError {
            entry: entry.to_owned(),
            instruction,
            executed_steps,
            reason,
        };
        let mut pc = named_instruction(&self.entries, entry)
            .ok_or_else(|| failure(0, 0, "entry is missing".to_owned()))?;
        if values.len() != self.initial_values.len() || self.literal_count > values.len() {
            return Err(failure(pc, 0, "register layout does not match".to_owned()));
        }
        let limit = self.instructions.len().saturating_mul(4).max(1);
        for step in 0..limit {
            let instruction = self
                .instructions
                .get(pc)
                .ok_or_else(|| failure(pc, step, "instruction is missing".to_owned()))?;
            if matches!(instruction, RigInstruction::Exit) {
                return Ok(step + 1);
            }
            let mut next = pc + 1;
            let result = (|| -> Result<(), String> {
                match instruction {
                    RigInstruction::Math {
                        operation,
                        inputs,
                        output,
                    } => {
                        let inputs = inputs
                            .iter()
                            .map(|input| input.read(values))
                            .collect::<Result<Vec<_>, _>>()?;
                        output.write(values, self.literal_count, operation.evaluate(&inputs)?)?;
                    }
                    RigInstruction::Call {
                        function,
                        arguments,
                        outputs,
                    } => {
                        let mut arguments_values = arguments
                            .iter()
                            .map(|input| input.read(values))
                            .collect::<Result<Vec<_>, _>>()?;
                        call_unit(pc, function, &mut arguments_values)?;
                        for &output in outputs {
                            let binding =
                                arguments.get(output).ok_or("rig call output is invalid")?;
                            binding.write(
                                values,
                                self.literal_count,
                                arguments_values[output].clone(),
                            )?;
                        }
                    }
                    RigInstruction::Copy { input, output } => {
                        output.write(values, self.literal_count, input.read(values)?)?
                    }
                    RigInstruction::Zero { output } => {
                        let value = match output.read(values)? {
                            RigValue::I32(_) => RigValue::I32(0),
                            RigValue::Name(_) => RigValue::Name(String::new()),
                            _ => return Err("rig zero requires a name or int32".to_owned()),
                        };
                        output.write(values, self.literal_count, value)?;
                    }
                    RigInstruction::Jump { instruction } => next = *instruction,
                    RigInstruction::Branch { selector, branches } => {
                        let RigValue::Name(label) = selector.read(values)? else {
                            return Err("rig branch requires a name".to_owned());
                        };
                        next = named_instruction(branches, &label)
                            .or_else(|| named_instruction(branches, "Completed"))
                            .ok_or("rig branch is missing")?;
                    }
                    RigInstruction::Exit => unreachable!(),
                }
                Ok(())
            })();
            result.map_err(|reason| failure(pc, step, reason))?;
            pc = next;
        }
        Err(failure(pc, limit, "instruction limit exceeded".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RigInstruction::*;
    use RigValue::*;
    use bevy::math::{DQuat, DVec3};

    fn slot(register: usize) -> RigValueRef {
        RigValueRef {
            register,
            path: vec![],
        }
    }

    fn branch(_instruction: usize, function: &str, arguments: &mut [RigValue]) -> Result<(), String> {
        if function != "Branch" {
            return Err("unknown fixture unit".to_owned());
        }
        let [Bool(condition), Name(block)] = arguments else {
            return Err("invalid fixture arguments".to_owned());
        };
        *block = if block.is_empty() {
            if *condition { "True" } else { "False" }
        } else {
            "Completed"
        }
        .to_owned();
        Ok(())
    }

    fn fixture() -> RigProgram {
        let yaw = RigValueRef {
            register: 5,
            path: vec![1],
        };
        RigProgram {
            initial_values: vec![
                F32(2.0),
                Bool(true),
                Name(String::new()),
                F32(0.0),
                F32(4.0),
                EulerDegrees(DVec3::ZERO),
                List(vec![
                    F64(0.0),
                    List(vec![
                        Quaternion(DQuat::IDENTITY),
                        Vector(DVec3::new(1.0, 3.0, 2.0)),
                    ]),
                ]),
                I32(42),
                F64(0.0),
            ],
            literal_count: 1,
            entries: BTreeMap::from([("Solve".to_owned(), 0)]),
            instructions: vec![
                Zero { output: slot(2) },
                Call {
                    function: "Branch".to_owned(),
                    arguments: vec![slot(1), slot(2)],
                    outputs: vec![1],
                },
                Branch {
                    selector: slot(2),
                    branches: BTreeMap::from([
                        ("True".to_owned(), 3),
                        ("False".to_owned(), 8),
                        ("Completed".to_owned(), 9),
                    ]),
                },
                Math {
                    operation: RigMathOp::F32Mul,
                    inputs: vec![slot(4), slot(0)],
                    output: slot(3),
                },
                Copy {
                    input: slot(3),
                    output: slot(4),
                },
                Copy {
                    input: slot(3),
                    output: yaw.clone(),
                },
                Copy {
                    input: yaw,
                    output: slot(8),
                },
                Jump { instruction: 1 },
                Jump { instruction: 1 },
                Copy {
                    input: slot(8),
                    output: RigValueRef {
                        register: 6,
                        path: vec![0],
                    },
                },
                Zero { output: slot(7) },
                Exit,
            ],
        }
    }

    #[test]
    fn program_executes_typed_fields_and_keeps_instance_state() {
        use bevy::reflect::{
            FromReflect, TypeRegistry,
            serde::{ReflectDeserializer, ReflectSerializer},
        };
        use serde::de::DeserializeSeed;

        let program = fixture();
        let decoded: RigProgram =
            serde_json::from_str(&serde_json::to_string(&program).unwrap()).unwrap();
        assert_eq!(decoded, program);
        let mut registry = TypeRegistry::default();
        registry.register::<RigProgram>();
        let reflected_text =
            ron::ser::to_string(&ReflectSerializer::new(&program, &registry)).unwrap();
        let mut reader = ron::de::Deserializer::from_str(&reflected_text).unwrap();
        let reflected = ReflectDeserializer::new(&registry)
            .deserialize(&mut reader)
            .unwrap();
        assert_eq!(
            RigProgram::from_reflect(reflected.as_ref()).unwrap(),
            program
        );
        let mut first = program.initial_values.clone();
        let mut second = program.initial_values.clone();
        for expected in [8.0, 16.0] {
            assert_eq!(program.execute("Solve", &mut first, branch).unwrap(), 13);
            assert_eq!(first[4], F32(expected));
            assert_eq!(
                first[5],
                EulerDegrees(DVec3::new(0.0, expected as f64, 0.0))
            );
            assert_eq!(
                RigValueRef {
                    register: 6,
                    path: vec![0]
                }
                .read(&first)
                .unwrap(),
                F64(expected as f64)
            );
            assert_eq!(first[2], Name("Completed".to_owned()));
            assert_eq!(first[7], I32(0));
        }
        second[1] = Bool(false);
        assert_eq!(program.execute("Solve", &mut second, branch).unwrap(), 9);
        assert_eq!(second[4], F32(4.0));
        assert_eq!(second[5], EulerDegrees(DVec3::ZERO));
        assert_eq!(second[7], I32(0));
        assert_eq!(first[4], F32(16.0));

        let mut mixed_case = program.initial_values.clone();
        program
            .execute("solve", &mut mixed_case, |pc, name, arguments| {
                branch(pc, name, arguments)?;
                if let Name(block) = &mut arguments[1] {
                    *block = block.to_ascii_lowercase();
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(mixed_case[4], F32(8.0));

        let mut nested = first.clone();
        let vector = RigValueRef {
            register: 6,
            path: vec![1, 1, 2],
        };
        vector.write(&mut nested, 1, F32(0.25)).unwrap();
        assert_eq!(vector.read(&nested).unwrap(), F64(0.25));
        let quaternion = RigValueRef {
            register: 6,
            path: vec![1, 0],
        };
        assert_eq!(
            quaternion.read(&nested).unwrap(),
            Quaternion(DQuat::IDENTITY)
        );

        let copies = RigProgram {
            initial_values: vec![F64(16_777_217.0), F32(0.0), F64(0.0)],
            literal_count: 1,
            entries: BTreeMap::from([("Solve".to_owned(), 0)]),
            instructions: vec![
                Copy {
                    input: slot(0),
                    output: slot(1),
                },
                Copy {
                    input: slot(1),
                    output: slot(2),
                },
                Exit,
            ],
        };
        let mut values = copies.initial_values.clone();
        copies.execute("Solve", &mut values, branch).unwrap();
        assert_eq!(
            values,
            vec![F64(16_777_217.0), F32(16_777_216.0), F64(16_777_216.0)]
        );
    }

    #[test]
    fn program_reports_fixed_progress_limit_and_invalid_operations() {
        let mut program = fixture();
        let mut values = program.initial_values.clone();
        program.instructions = vec![Jump { instruction: 0 }];
        let failure = program.execute("Solve", &mut values, branch).unwrap_err();
        assert_eq!(failure.instruction, 0);
        assert_eq!(failure.executed_steps, 4);
        assert_eq!(failure.reason, "instruction limit exceeded");
        let report = serde_json::to_value(&failure).unwrap();
        assert_eq!(report["entry"], "Solve");
        assert_eq!(report["executed_steps"], 4);

        for instruction in [
            Jump { instruction: 99 },
            Copy {
                input: slot(3),
                output: slot(0),
            },
            Copy {
                input: slot(2),
                output: slot(3),
            },
            Copy {
                input: slot(3),
                output: RigValueRef {
                    register: 6,
                    path: vec![99],
                },
            },
            Zero { output: slot(3) },
            Branch {
                selector: slot(3),
                branches: BTreeMap::new(),
            },
            Call {
                function: "Missing".to_owned(),
                arguments: vec![],
                outputs: vec![],
            },
        ] {
            program.instructions = vec![instruction, Exit];
            assert!(program.execute("Solve", &mut values, branch).is_err());
        }
        assert!(program.execute("Missing", &mut values, branch).is_err());
        assert!(program.execute("Solve", &mut [], branch).is_err());
        program.instructions = vec![
            Branch {
                selector: slot(2),
                branches: BTreeMap::from([("Completed".to_owned(), 1)]),
            },
            Exit,
        ];
        values[2] = Name("Unknown".to_owned());
        assert_eq!(program.execute("Solve", &mut values, branch).unwrap(), 2);
    }

    #[test]
    fn control_rig_complete_source_program_runs_construction_and_frames() {
        use crate::extensions::{
            control_rig_hierarchy::{RigHierarchy, RigSpace, RigTransform},
            control_rig_units::RigUnitContext,
        };
        use bevy::platform::collections::HashMap;

        let definition: RigDefinition = serde_json::from_str(include_str!(
            "../../assets/control_rigs/cr_als.program.json"
        ))
        .unwrap();
        let program = &definition.program;
        assert_eq!(program.instructions.len(), 165);
        assert_eq!(definition.bones.len(), 79);
        assert_eq!(definition.curves.len(), 43);
        assert!(definition.curves.values().all(Option::is_none));
        let mut hierarchy =
            RigHierarchy::new(definition.bones.clone(), definition.translation_tolerance).unwrap();
        let mut values = program.initial_values.clone();
        let mut input = definition.externals["RigInput"].clone();
        for index in [0, 1] {
            input.path = vec![index];
            input
                .write(&mut values, program.literal_count, Bool(true))
                .unwrap();
        }
        let mut curves = HashMap::default();
        {
            let mut context = RigUnitContext {
                hierarchy: &mut hierarchy,
                curves: &curves,
                game_world: true,
                construction: false,
                delta_seconds: 0.0,
                geometry: &definition.geometry,
                to_world: RigTransform::default(),
                line_trace: None,
            };
            assert_eq!(
                program
                    .execute("Construction", &mut values, |_, name, args| context
                        .call(name, args))
                    .unwrap(),
                15
            );
            assert!(context.construction);
        }
        let external =
            |name: &str, state: &[RigValue]| definition.externals[name].read(state).unwrap();
        assert_eq!(
            external("FootRootBone", &values),
            List(vec![
                Name("Bone".to_owned()),
                Name("ik_foot_root".to_owned())
            ])
        );
        assert_eq!(
            external("HandLeftIkTargetBone", &values),
            List(vec![
                Name("Bone".to_owned()),
                Name("VB ik_hand_l".to_owned())
            ])
        );
        let initial = |name: &str| {
            definition
                .bones
                .iter()
                .find(|bone| bone.name == name)
                .unwrap()
                .initial_global
        };
        assert_eq!(
            external("FootHeight", &values),
            F64(initial("foot_l").translation.y)
        );
        let upper = initial("thigh_l")
            .translation
            .distance(initial("calf_l").translation) as f32;
        let lower = initial("calf_l")
            .translation
            .distance(initial("foot_l").translation);
        let length = (upper as f64 + lower) as f32;
        assert_eq!(external("LegLength", &values), F64(length as f64));
        let constructed = values.clone();
        let mut pose: Vec<_> = definition
            .bones
            .iter()
            .map(|bone| bone.initial_local)
            .collect();
        let foot_root = hierarchy.index("ik_foot_root").unwrap();
        let pelvis = hierarchy.index("pelvis").unwrap();
        pose[foot_root].scale = DVec3::splat(2.0);
        curves.extend([
            ("PoseMoving".to_owned(), 1.0),
            ("FootLeftIk".to_owned(), 1.0),
            ("FootRightIk".to_owned(), 1.0),
            ("LayerArmLeft".to_owned(), 1.0),
            ("HandLeftIk".to_owned(), 1.0),
            ("LayerArmRight".to_owned(), 1.0),
            ("HandRightIk".to_owned(), 1.0),
        ]);
        for (index, value) in [
            (2, Bool(true)),
            (3, F32(0.5)),
            (6, F32(1.0)),
            (7, Vector(initial("foot_l").translation)),
            (8, Quaternion(initial("foot_l").rotation)),
            (9, Vector(initial("foot_r").translation)),
            (10, Quaternion(initial("foot_r").rotation)),
        ] {
            input.path = vec![index];
            input
                .write(&mut values, program.literal_count, value)
                .unwrap();
        }
        let mut trace_count = 0;
        for frame in 0..3 {
            hierarchy.set_input_pose(&pose).unwrap();
            let surface = if frame == 0 { -0.05 } else { -0.1 };
            let mut query = |start: DVec3, end: DVec3, channel: &str, complex: bool| {
                trace_count += 1;
                assert_eq!(channel, "ECC_Visibility");
                assert!(complex);
                assert!((start.y - 0.5).abs() < 1e-12);
                assert!((end.y + 0.8).abs() < 2e-8);
                Ok(Some((DVec3::new(start.x, surface, start.z), DVec3::Y)))
            };
            let mut context = RigUnitContext {
                hierarchy: &mut hierarchy,
                curves: &curves,
                game_world: true,
                construction: true,
                delta_seconds: 1.0 / 60.0,
                geometry: &definition.geometry,
                to_world: RigTransform::default(),
                line_trace: Some(&mut query),
            };
            assert!(
                program
                    .execute("Forwards Solve", &mut values, |_, name, args| context
                        .call(name, args))
                    .unwrap()
                    > 140
            );
            assert!(!context.construction);
            assert_eq!(trace_count, (frame + 1) * 2);
            assert!(
                (hierarchy
                    .transform(foot_root, RigSpace::Local, false)
                    .unwrap()
                    .scale
                    - DVec3::new(2.4, 2.0, 2.4))
                .length()
                    < 1e-12
            );
            let F32(offset) = external("PelvisOffset", &values) else {
                panic!("invalid pelvis offset type")
            };
            if frame == 0 {
                assert!((offset + 0.05).abs() < 1e-7);
            } else {
                assert!(offset < -0.05 && offset > -0.1);
            }
            assert!(
                (hierarchy
                    .transform(pelvis, RigSpace::Global, false)
                    .unwrap()
                    .translation
                    .y
                    - initial("pelvis").translation.y
                    - offset as f64)
                    .abs()
                    < 1e-7
            );
            for index in 0..definition.bones.len() {
                let transform = hierarchy.transform(index, RigSpace::Global, false).unwrap();
                assert!(transform.translation.is_finite() && transform.rotation.is_finite());
            }
        }
        assert_eq!(external("PelvisOffset", &constructed), F32(0.0));
        assert_eq!(
            external("FootRootBone", &program.initial_values),
            List(vec![Name("None".to_owned()), Name(String::new())])
        );
        hierarchy.set_input_pose(&pose).unwrap();
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world: false,
            construction: false,
            delta_seconds: 1.0 / 60.0,
            geometry: &definition.geometry,
            to_world: RigTransform::default(),
            line_trace: None,
        };
        assert!(
            program
                .execute("Forwards Solve", &mut values, |_, name, args| context
                    .call(name, args))
                .unwrap()
                < 10
        );
        context.game_world = true;
        let failure = program
            .execute("Forwards Solve", &mut values, |_, name, args| {
                context.call(name, args)
            })
            .unwrap_err();
        assert_eq!(failure.instruction, 31);
        assert_eq!(failure.reason, "rig line trace is unavailable");
        let mut independent_values = program.initial_values.clone();
        let mut independent_hierarchy =
            RigHierarchy::new(definition.bones.clone(), definition.translation_tolerance).unwrap();
        let independent_curves = HashMap::default();
        let mut context = RigUnitContext {
            hierarchy: &mut independent_hierarchy,
            curves: &independent_curves,
            game_world: true,
            construction: false,
            delta_seconds: 1.0 / 60.0,
            geometry: &definition.geometry,
            to_world: RigTransform::default(),
            line_trace: None,
        };
        for entry in ["Construction", "Forwards Solve"] {
            program
                .execute(entry, &mut independent_values, |_, name, args| {
                    context.call(name, args)
                })
                .unwrap();
        }
        assert_eq!(
            external("FootRootBone", &independent_values),
            List(vec![
                Name("Bone".to_owned()),
                Name("VB foot_root".to_owned())
            ])
        );
        assert_eq!(external("PelvisOffset", &independent_values), F32(0.0));
        assert_eq!(
            external("FootLeftOffsetLocationZ", &independent_values),
            F64(0.0)
        );
    }
}
