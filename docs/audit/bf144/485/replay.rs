use avian3d::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};
use bevy_als::extensions::control_rig_query::{RigQueryContext, RigQueryWorld};
use bevy_als::extensions::{
    SourceCurveValues,
    control_rig_hierarchy::{RigHierarchy, RigSpace, RigTransform},
    control_rig_math::RigValue,
    control_rig_program::RigDefinition,
    control_rig_units::RigUnitContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
fn dv(v: &Value) -> DVec3 {
    DVec3::from_array(std::array::from_fn(|i| v[i].as_f64().unwrap()))
}
fn local(v: &Value) -> RigTransform {
    RigTransform {
        translation: dv(&v["translation"]),
        rotation: DQuat::from_array(std::array::from_fn(|i| v["rotation"][i].as_f64().unwrap())),
        scale: dv(&v["scale"]),
    }
}
fn input(v: &Value) -> RigValue {
    let mut v = v.clone();
    let x = v.as_object_mut().unwrap();
    if let Some(a) = x.remove("Vec3") {
        x.insert("Vector".into(), a);
    }
    if let Some(a) = x.remove("Quat") {
        x.insert("Quaternion".into(), a);
    }
    serde_json::from_value(v).unwrap()
}
fn quat(v: &Value) -> DQuat {
    DQuat::from_array(std::array::from_fn(|i| v[i].as_f64().unwrap()))
}
fn query() -> RigQueryContext {
    let mut colliders = HashMap::default();
    let mut world = World::new();
    for x in [-20.0, 0.0] {
        colliders.insert(
            world.spawn_empty().id(),
            (
                Position(Vec3::new(x, -0.25, -20.0)),
                Rotation::default(),
                Collider::cuboid(20.0, 0.5, 20.0),
                CollisionLayers::ALL,
            ),
        );
    }
    RigQueryContext {
        world: Arc::new(RigQueryWorld { colliders }),
        to_world: RigTransform::default(),
        channel: "ECC_Visibility".into(),
        filter: SpatialQueryFilter::default(),
    }
}

mod native;
fn main() {
    let r = std::path::Path::new("docs/audit/bf144/485");
    let label = std::env::args().nth(1).unwrap();
    if label == "native" {
        native::run(r);
        return;
    }
    let data: Value =
        serde_json::from_slice(&std::fs::read("docs/audit/bf144/459/inputs.json").unwrap())
            .unwrap();
    let replacements: Vec<Value> =
        serde_json::from_slice(&std::fs::read(r.join("vm-input.json")).unwrap()).unwrap();
    let geometry = query();
    let mut changed_sides = 0;
    let mut changed_bones = 0;
    let mut changed_f32_bones = 0;
    let definition: RigDefinition = serde_json::from_value(data["definition"].clone()).unwrap();
    let frames = data["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 319);
    let mut hierarchy =
        RigHierarchy::new(definition.bones.clone(), definition.translation_tolerance).unwrap();
    let mut values = definition.program.initial_values.clone();
    let mut outputs = Vec::new();
    let mut total_calls = 0;
    let mut total_traces = 0;
    for (query, frame) in frames.iter().enumerate() {
        for b in frame["bindings"].as_array().unwrap() {
            let mut reference = definition.externals[b["external"].as_str().unwrap()].clone();
            reference.path.extend(
                b["path"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as usize),
            );
            let v = if b["resolved_player_property"].is_null() {
                &b["global"]
            } else {
                &b["resolved_player_property"]
            };
            reference
                .write(&mut values, definition.program.literal_count, input(v))
                .unwrap();
        }
        let delta = frame["delta_seconds"].as_f64().unwrap();
        let to_world: RigTransform = serde_json::from_value(frame["to_world"].clone()).unwrap();
        let game_world = frame["game_world"].as_bool().unwrap();
        if query == 0 {
            let curves = SourceCurveValues::default();
            let mut context = RigUnitContext {
                hierarchy: &mut hierarchy,
                curves: &curves,
                game_world,
                construction: false,
                delta_seconds: delta,
                geometry: &definition.geometry,
                to_world,
                line_trace: None,
            };
            definition
                .program
                .execute("Construction", &mut values, |_, name, args| {
                    context.call(name, args)
                })
                .unwrap();
        }
        let pose = frame["pose"]["bones"].as_array().unwrap();
        let local = definition
            .bones
            .iter()
            .map(|b| local(pose.iter().find(|p| p["name"] == b.name).unwrap()))
            .collect::<Vec<_>>();
        hierarchy.set_input_pose(&local).unwrap();
        let curves: SourceCurveValues = definition
            .curves
            .keys()
            .filter_map(|name| {
                frame["curves"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, v)| (name.clone().into(), v.as_f64().unwrap() as f32))
            })
            .collect();
        let mut corrected_hierarchy = hierarchy.clone();
        let mut corrected_values = values.clone();
        let mut sides = Vec::new();
        if query > 0 {
            let row = &replacements[query - 1];
            assert_eq!(row["query"], query);
            let mut valid = definition.externals["RigInput"].clone();
            valid.path.push(2);
            assert_eq!(valid.read(&values).unwrap(), RigValue::Bool(true));
            for (side, slot) in [8, 10].into_iter().enumerate() {
                let mut location = definition.externals["RigInput"].clone();
                location.path.push(slot - 1);
                assert_eq!(
                    location.read(&values).unwrap(),
                    RigValue::Vector(dv(&row["baseline"]["location"][side]))
                );
                let mut rotation = definition.externals["RigInput"].clone();
                rotation.path.push(slot);
                let baseline = quat(&row["baseline"]["rotation"][side]);
                let corrected = quat(&row["corrected"]["rotation"][side]);
                assert_eq!(
                    rotation.read(&values).unwrap(),
                    RigValue::Quaternion(baseline)
                );
                if corrected != baseline {
                    rotation
                        .write(
                            &mut corrected_values,
                            definition.program.literal_count,
                            RigValue::Quaternion(corrected),
                        )
                        .unwrap();
                    sides.push(side);
                }
            }
        }
        let expected_calls = frame["expected_calls"].as_array().unwrap();
        let expected_traces = frame["expected_traces"].as_array().unwrap();
        let pc = std::cell::Cell::new(0);
        let trace_index = std::cell::Cell::new(0);
        let mut call_index = 0;
        let mut trace = |start: DVec3, end: DVec3, channel: &str, complex: bool| {
            let i = trace_index.get();
            let expected = &expected_traces[i];
            assert_eq!(pc.get(), expected["instruction"].as_u64().unwrap() as usize);
            assert_eq!(
                start,
                dv(&expected["start"]),
                "Actual trace start query {query}"
            );
            assert_eq!(end, dv(&expected["end"]), "Actual trace end query {query}");
            let hit = geometry.trace(start, end, channel, complex)?;
            assert_eq!(
                hit,
                Some((dv(&expected["hit"][0]), dv(&expected["hit"][1])))
            );
            trace_index.set(i + 1);
            Ok(hit)
        };
        let mut context = RigUnitContext {
            hierarchy: &mut hierarchy,
            curves: &curves,
            game_world,
            construction: false,
            delta_seconds: delta,
            geometry: &definition.geometry,
            to_world,
            line_trace: if expected_traces.is_empty() {
                None
            } else {
                Some(&mut trace)
            },
        };
        let steps=definition.program.execute("Forwards Solve",&mut values,|instruction,name,args|{
   pc.set(instruction);let expected=&expected_calls[call_index];assert_eq!(instruction,expected["instruction"].as_u64().unwrap()as usize,"Actual call order query {query}");
   let indices=args.iter().filter_map(|v|context.hierarchy.item_index(v).ok().flatten()).collect::<Vec<_>>();let mut snapshot=context.hierarchy.clone();let before=indices.into_iter().map(|i|(i,snapshot.transform(i,RigSpace::Global,false).unwrap())).collect::<Vec<_>>();assert_eq!(serde_json::to_value(before).unwrap(),expected["bones_before_call"],"Actual pre-call bones query {query} instruction {instruction}");
   context.call(name,args)?;let actual=serde_json::to_value(&*args).unwrap();if actual!=expected["arguments_after_call"]{std::fs::write(r.join(format!("failure-{label}.json")),serde_json::to_vec_pretty(&json!({"query":query,"instruction":instruction,"function":name,"actual":actual,"expected":expected["arguments_after_call"]})).unwrap()).unwrap();panic!("Actual post-call arguments differ at query {query} instruction {instruction}");}call_index+=1;Ok(())
  }).unwrap();
        assert_eq!(steps, frame["expected_steps"].as_u64().unwrap() as usize);
        assert_eq!(call_index, expected_calls.len());
        assert_eq!(trace_index.get(), expected_traces.len());
        drop(context);
        let actual = serde_json::to_value(&values).unwrap();
        assert_eq!(
            actual, frame["expected_registers"],
            "Actual complete working state query {query}"
        );
        let local_output=definition.bones.iter().enumerate().map(|(i,b)|json!({"index":i,"name":b.name,"parent":b.parent,"local":hierarchy.transform(i,RigSpace::Local,false).unwrap()})).collect::<Vec<_>>();
        assert_eq!(
            json!(local_output),
            frame["expected_local"],
            "Actual local hierarchy query {query}"
        );
        if !sides.is_empty() {
            changed_sides += sides.len();
            let corrected_pc = std::cell::Cell::new(0);
            let mut corrected_traces = Vec::new();
            let mut corrected_trace = |start: DVec3, end: DVec3, channel: &str, complex: bool| {
                let hit = geometry.trace(start, end, channel, complex)?;
                corrected_traces.push(
                    json!({"instruction":corrected_pc.get(),"start":start,"end":end,"hit":hit}),
                );
                Ok(hit)
            };
            let mut corrected_context = RigUnitContext {
                hierarchy: &mut corrected_hierarchy,
                curves: &curves,
                game_world,
                construction: false,
                delta_seconds: delta,
                geometry: &definition.geometry,
                to_world,
                line_trace: Some(&mut corrected_trace),
            };
            let mut call_differences = Vec::new();
            let mut index = 0;
            let corrected_steps=definition.program.execute("Forwards Solve",&mut corrected_values,|pc,name,args| {
    corrected_pc.set(pc);
    let expected=&expected_calls[index];
    assert_eq!(pc,expected["instruction"].as_u64().unwrap() as usize);
    corrected_context.call(name,args)?;
    let after=json!(&*args);
    if after!=expected["arguments_after_call"] {
     call_differences.push(json!({"pc":pc,"function":name,"before":expected["arguments_after_call"],"after":after}));
    }
    index+=1;
    Ok(())
   }).unwrap();
            drop(corrected_context);
            assert_eq!(steps, corrected_steps);
            assert_eq!(index, call_index);
            assert_eq!(
                &corrected_traces, expected_traces,
                "Trace result changed at query {query}"
            );
            let mut differences = Vec::new();
            for (i, bone) in definition.bones.iter().enumerate() {
                let before = hierarchy.transform(i, RigSpace::Local, false).unwrap();
                let after = corrected_hierarchy
                    .transform(i, RigSpace::Local, false)
                    .unwrap();
                if before != after {
                    let f32_changed = before.translation.as_vec3() != after.translation.as_vec3()
                        || before.rotation.as_quat() != after.rotation.as_quat()
                        || before.scale.as_vec3() != after.scale.as_vec3();
                    changed_f32_bones += usize::from(f32_changed);
                    differences.push(json!({"bone":bone.name,"before":before,"after":after,"f32_changed":f32_changed}));
                }
            }
            changed_bones += differences.len();
            outputs.push(json!({"query":query,"sides":sides,"calls":call_differences,"local":differences,"traces":corrected_traces}));
        }
        total_calls += call_index;
        total_traces += trace_index.get();
        if query % 32 == 0 {
            println!("Replayed complete actual Control Rig state query {query}");
        }
    }
    assert_eq!((total_calls, total_traces), (13378, 636));
    let report = json!({"queries":319,"calls":total_calls,"traces":total_traces,"changed_sides":changed_sides,
                   "changed_local_rows":changed_bones,"changed_f32_local_rows":changed_f32_bones,"outputs":outputs});
    std::fs::write(
        r.join(format!("replay-{label}.json")),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    println!(
        "Replayed 319 actual frames, with {changed_sides} altered sides and {changed_f32_bones} F32 local changes"
    );
    assert_eq!(
        changed_sides, 103,
        "The D276 input substitution must be active"
    );
    assert_eq!(
        changed_f32_bones, 103,
        "The D276 output changes must reach the F32 local pose"
    );
}
