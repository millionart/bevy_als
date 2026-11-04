use bevy_als::extensions::{
    control_rig_hierarchy::{RigHierarchy, RigTransform},
    control_rig_math::RigValue,
    control_rig_program::RigDefinition,
    control_rig_units::RigUnitContext,
};
use serde_json::{Value, json};
use std::path::Path;

const RETAIN_WIDENED_SEED: bool = true;

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn main() {
    let label = std::env::args().nth(1).unwrap();
    let root = Path::new("docs/audit/bf144/504");
    let definition: RigDefinition =
        serde_json::from_value(read(&root.join("runtime-baseline/rig-definition.json"))).unwrap();
    let input = read(&root.join("damper-input.json"));
    let mut hierarchy =
        RigHierarchy::new(definition.bones.clone(), definition.translation_tolerance).unwrap();
    let seed = |role: usize| {
        std::array::from_fn::<RigValue, 2, _>(|side| {
            serde_json::from_value(input["initial"][role][side].clone()).unwrap()
        })
    };
    let mut state = [seed(0), seed(usize::from(RETAIN_WIDENED_SEED))];
    let mut mismatches = 0;
    let mut previous_mismatches = 0;
    let mut rows = Vec::new();
    for row in input["calls"].as_array().unwrap() {
        let side = row["side"].as_u64().unwrap() as usize;
        let query = row["query"].as_u64().unwrap();
        let mut outputs = Vec::new();
        let mut resets = Vec::new();
        for (role, side_states) in state.iter_mut().enumerate() {
            previous_mismatches += usize::from(json!(side_states[side]) != row["previous"][role]);
            let mut args: Vec<RigValue> =
                serde_json::from_value(row["expected"][role].clone()).unwrap();
            args[2] = side_states[side].clone();
            let mut ctx = RigUnitContext {
                hierarchy: &mut hierarchy,
                curves: &Default::default(),
                game_world: true,
                construction: false,
                delta_seconds: row["delta"].as_f64().unwrap(),
                geometry: &definition.geometry,
                to_world: RigTransform::default(),
                line_trace: None,
            };
            let unit = "FAlsRigVMFunction_DamperExactVector::Execute";
            let reset = (query == 146).then(|| {
                let mut init = args.clone();
                init[3] = RigValue::Bool(false);
                ctx.call(unit, &mut init).unwrap();
                assert_eq!(init[2], init[0]);
                assert_eq!(init[3], RigValue::Bool(true));
                init[2].clone()
            });
            resets.push(reset);
            ctx.call(unit, &mut args).unwrap();
            mismatches += usize::from(json!(&args) != row["expected"][role]);
            side_states[side] = args[2].clone();
            outputs.push(args[2].clone());
        }
        if query == 146 {
            assert!(resets[0].is_some() && resets[0] == resets[1]);
        }
        rows.push(json!({"query":query,"side":side,"outputs":outputs,"forced_reset":resets}));
        if side == 0 && query % 16 == 0 {
            println!("Replayed continuous pole histories at {query}");
        }
    }
    std::fs::write(
        root.join(format!("replay-{label}.json")),
        serde_json::to_vec(
            &json!({"rows":rows,"mismatches":mismatches,"previous_mismatches":previous_mismatches}),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        mismatches + previous_mismatches,
        0,
        "Both continuous histories must reproduce actual pole states"
    );
}
