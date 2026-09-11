use std::{collections::BTreeMap, fmt};

use bevy::prelude::Reflect;
use serde::{Deserialize, Serialize};

use crate::control_rig_math::{RigMathOp, RigValue};

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

    fn write(
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
        mut call_unit: impl FnMut(&str, &mut [RigValue]) -> Result<(), String>,
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
                        call_unit(function, &mut arguments_values)?;
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

    fn branch(function: &str, arguments: &mut [RigValue]) -> Result<(), String> {
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
            .execute("solve", &mut mixed_case, |name, arguments| {
                branch(name, arguments)?;
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
}
