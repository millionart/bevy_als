use bevy::math::{DQuat, DVec3};
use serde::Deserialize;

use crate::extensions::{
    control_rig_math::RigValue, control_rig_program::RigDefinition, control_rig_units::call_unit,
};
use RigValue::*;

#[test]
fn initial_globals_match_native_cdo_hierarchy() {
    use crate::extensions::control_rig_hierarchy::{RigHierarchy, RigSpace};

    let source: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../docs/audit/bf144/d242/native-cdo-bones.json"
    ))
    .unwrap();
    let definition: RigDefinition = serde_json::from_str(include_str!(
        "../../assets/ALS/Character/CR_Als.program.json"
    ))
    .unwrap();
    assert_eq!(source.len(), definition.bones.len());
    let mut hierarchy =
        RigHierarchy::new(definition.bones.clone(), definition.translation_tolerance).unwrap();
    for (index, (source, bone)) in source.iter().zip(&definition.bones).enumerate() {
        assert_eq!(source["name"].as_str(), Some(bone.name.as_str()));
        let transform = &source["initial_global_transform"];
        let component = |group: &str, axis: &str| transform[group][axis].as_f64().unwrap();
        let translation = DVec3::new(
            component("translation", "x") * 0.01,
            component("translation", "z") * 0.01,
            component("translation", "y") * 0.01,
        );
        let rotation = DQuat::from_xyzw(
            -component("rotation", "x"),
            -component("rotation", "z"),
            -component("rotation", "y"),
            component("rotation", "w"),
        );
        let scale = DVec3::new(
            component("scale", "x"),
            component("scale", "z"),
            component("scale", "y"),
        );
        assert_eq!(
            bone.initial_global.translation, translation,
            "{}",
            bone.name
        );
        assert_eq!(bone.initial_global.rotation, rotation, "{}", bone.name);
        assert_eq!(bone.initial_global.scale, scale, "{}", bone.name);
        assert_eq!(
            hierarchy.transform(index, RigSpace::Global, true).unwrap(),
            bone.initial_global,
            "{}",
            bone.name
        );
    }
}

#[test]
fn control_rig_publishes_the_native_pelvis_offset_to_both_feet() {
    use crate::extensions::{
        control_rig_hierarchy::{RigHierarchy, RigSpace, RigTransform},
        control_rig_units::RigUnitContext,
    };

    #[derive(Deserialize)]
    struct Fixture {
        histories: Vec<History>,
    }

    #[derive(Deserialize)]
    struct History {
        name: String,
        frames: Vec<Frame>,
    }

    #[derive(Deserialize)]
    struct Frame {
        query: usize,
        delta_seconds: f64,
        rig_input: RigValue,
        trace_offsets: [f32; 2],
        spring_target: f32,
        spring_result: f32,
        spring_state: (f32, f32, bool),
        writer: f32,
        pelvis_before: [f64; 3],
        pelvis_desired: [f64; 3],
        feet: Vec<usize>,
    }

    let definition: RigDefinition = serde_json::from_str(include_str!(
        "../../assets/ALS/Character/CR_Als.program.json"
    ))
    .unwrap();
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../docs/audit/bf144/d165c/pelvis-history.replay.json"
    ))
    .unwrap();
    let program = &definition.program;
    let pose: Vec<_> = definition
        .bones
        .iter()
        .map(|bone| bone.initial_local)
        .collect();
    let curves = crate::extensions::SourceCurveValues::from_iter([
        ("PoseMoving".into(), 1.0),
        ("FootLeftIk".into(), 1.0),
        ("FootRightIk".into(), 1.0),
    ]);
    for history in fixture.histories {
        let mut hierarchy = RigHierarchy::new(definition.bones.clone(), 0.0).unwrap();
        let pelvis = hierarchy.index("pelvis").unwrap();
        let mut values = program.initial_values.clone();
        let input = &definition.externals["RigInput"];
        input
            .write(
                &mut values,
                program.literal_count,
                history.frames[0].rig_input.clone(),
            )
            .unwrap();
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
        program
            .execute("Construction", &mut values, |_, name, args| {
                call_unit(&mut context, name, args)
            })
            .unwrap();
        for frame in history.frames {
            let label = (&history.name, frame.query);
            input
                .write(&mut values, program.literal_count, frame.rig_input)
                .unwrap();
            context.hierarchy.set_input_pose(&pose).unwrap();
            let mut before = context
                .hierarchy
                .transform(pelvis, RigSpace::Global, false)
                .unwrap();
            before.translation = DVec3::from_array(frame.pelvis_before);
            context
                .hierarchy
                .set_transform(pelvis, RigSpace::Global, false, before, true)
                .unwrap();
            context.delta_seconds = frame.delta_seconds;
            let mut feet = Vec::new();
            program
                .execute("Forwards Solve", &mut values, |pc, name, args| {
                    match pc {
                        31 | 40 => {
                            args[8] = Centimeters(frame.trace_offsets[usize::from(pc == 40)]);
                            args[9] = Vector(DVec3::Y);
                            return Ok(());
                        }
                        54 => {
                            assert_eq!(args[0], Centimeters(frame.spring_target), "{label:?}")
                        }
                        66 => {
                            let Vector(translation) = args[3] else {
                                panic!("pelvis translation is missing");
                            };
                            assert!(
                                translation.distance(DVec3::from_array(frame.pelvis_desired))
                                    < 1e-12,
                                "{label:?}: {translation:?}"
                            );
                        }
                        83 | 113 => {
                            feet.push(pc);
                            assert_eq!(args[4], Centimeters(frame.writer), "{label:?}");
                        }
                        _ => {}
                    }
                    call_unit(&mut context, name, args)?;
                    if pc == 54 {
                        let (previous, velocity, valid) = frame.spring_state;
                        assert_eq!(args[8], Centimeters(frame.spring_result), "{label:?}");
                        assert_eq!(args[9], Centimeters(velocity), "{label:?}");
                        assert_eq!(args[10], Centimeters(frame.spring_result), "{label:?}");
                        assert_eq!(
                            args[11],
                            List(vec![
                                Centimeters(previous),
                                Centimeters(velocity),
                                Bool(valid)
                            ]),
                            "{label:?}"
                        );
                    }
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                definition.externals["PelvisOffset"].read(&values).unwrap(),
                Centimeters(frame.writer),
                "{label:?}"
            );
            assert_eq!(feet, frame.feet, "{label:?}");
        }
    }
}

#[test]
fn control_rig_complete_source_program_runs_construction_and_frames() {
    use crate::extensions::{
        control_rig_hierarchy::{RigHierarchy, RigSpace, RigTransform},
        control_rig_ik::source_vector_dot,
        control_rig_units::RigUnitContext,
    };
    use bevy::platform::collections::HashMap;

    let definition: RigDefinition = serde_json::from_str(include_str!(
        "../../assets/ALS/Character/CR_Als.program.json"
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
                .execute("Construction", &mut values, |_, name, args| call_unit(
                    &mut context,
                    name,
                    args
                ))
                .unwrap(),
            15
        );
        assert!(context.construction);
    }
    let external = |name: &str, state: &[RigValue]| definition.externals[name].read(state).unwrap();
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
        Centimeters64(initial("foot_l").translation.y * 100.0)
    );
    let segment_cm = |a, b| {
        let delta = initial(a).translation * 100.0 - initial(b).translation * 100.0;
        source_vector_dot(delta, delta).sqrt()
    };
    let upper_cm = segment_cm("thigh_l", "calf_l") as f32;
    let length_cm = (f64::from(upper_cm) + segment_cm("calf_l", "foot_l")) as f32;
    assert_eq!(
        external("LegLength", &values),
        Centimeters64(f64::from(length_cm))
    );
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
        ("PoseMoving".into(), 1.0),
        ("FootLeftIk".into(), 1.0),
        ("FootRightIk".into(), 1.0),
        ("LayerArmLeft".into(), 1.0),
        ("HandLeftIk".into(), 1.0),
        ("LayerArmRight".into(), 1.0),
        ("HandRightIk".into(), 1.0),
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
                .execute("Forwards Solve", &mut values, |_, name, args| call_unit(
                    &mut context,
                    name,
                    args
                ))
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
        let Centimeters(offset) = external("PelvisOffset", &values) else {
            panic!("invalid pelvis offset type")
        };
        let offset = f64::from(offset) * 0.01;
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
                - offset)
                .abs()
                < 1e-7
        );
        for index in 0..definition.bones.len() {
            let transform = hierarchy.transform(index, RigSpace::Global, false).unwrap();
            assert!(transform.translation.is_finite() && transform.rotation.is_finite());
        }
    }
    assert_eq!(external("PelvisOffset", &constructed), Centimeters(0.0));
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
            .execute("Forwards Solve", &mut values, |_, name, args| call_unit(
                &mut context,
                name,
                args
            ))
            .unwrap()
            < 10
    );
    context.game_world = true;
    let failure = program
        .execute("Forwards Solve", &mut values, |_, name, args| {
            call_unit(&mut context, name, args)
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
                call_unit(&mut context, name, args)
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
    assert_eq!(
        external("PelvisOffset", &independent_values),
        Centimeters(0.0)
    );
    assert_eq!(
        external("FootLeftOffsetLocationZ", &independent_values),
        Centimeters64(0.0)
    );
}
