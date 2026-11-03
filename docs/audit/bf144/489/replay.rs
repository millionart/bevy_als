include!("../../../../tools/tmp/bf144/489/foot.rs");

fn match_sign(candidate: Quat, actual: Quat) -> Quat {
    let dot = candidate.dot(actual);
    assert!(dot.is_finite() && dot != 0.0);
    if dot < 0.0 { -candidate } else { candidate }
}

fn main() {
    let root = Path::new("docs/audit/bf144/489");
    let temporary = Path::new("tools/tmp/bf144/489");
    let label = std::env::args().nth(1).unwrap();
    let inputs = read(&temporary.join("foot-input.json"));
    let grid = read(&temporary.join("grid.json"));
    let profile = read(Path::new("assets/ALS/Character/CR_Als.runtime.json"));
    let axes = [
        "foot_left_thigh_axis_pelvis_space",
        "foot_right_thigh_axis_pelvis_space",
    ]
    .map(|name| vector(&profile["target"][name]));
    let mut output = Vec::new();
    let mut replacements = [Vec::new(), Vec::new()];
    let mut changed_targets = 0;
    let mut changed_inputs = 0;
    for (index, row) in inputs["rows"].as_array().unwrap().iter().enumerate() {
        let observed = &row["observation"];
        let query = row["query"].as_u64().unwrap();
        assert_eq!(query, index as u64 + 1);
        assert_eq!(observed["valid"], true);
        assert_eq!(observed["use_ik"], true);
        assert_eq!(observed["teleport_active"], false);
        let actual = [0, 1].map(|side| quaternion(&observed["states"][side]["target_rotation"]));
        let baseline = foot_case(row, axes, actual);
        assert_eq!(
            baseline["states"], observed["states"],
            "Actual foot state query {query}"
        );
        assert_eq!(
            baseline["rig"], observed["rig"],
            "Actual RigInput query {query}"
        );
        let uniform = [0, 1].map(|side| {
            let input = &grid["target"][index * 2 + side];
            assert_eq!(input["query"], query);
            assert_eq!(input["side"], side);
            assert_eq!(quaternion(&input["actual_q"]), actual[side]);
            assert_eq!(
                input["actual_t"],
                observed["states"][side]["target_location"]
            );
            match_sign(quaternion(&input["uniform_q"]), actual[side])
        });
        let unit_only = [0, 1].map(|side| {
            if actual[side] == uniform[side] {
                actual[side]
            } else {
                actual[side].normalize()
            }
        });
        let unit = foot_case(row, axes, unit_only);
        let control = foot_case(row, axes, uniform);
        for (mode, value) in [&unit, &control].into_iter().enumerate() {
            assert_eq!(value["rig"]["location"], baseline["rig"]["location"]);
            replacements[mode]
                .push(json!({"query":query,"baseline":baseline["rig"],"corrected":value["rig"]}));
        }
        changed_targets += (0..2).filter(|&side| actual[side] != uniform[side]).count();
        changed_inputs += (0..2)
            .filter(|&side| baseline["rig"]["rotation"][side] != control["rig"]["rotation"][side])
            .count();
        output.push(json!({"query":query,"baseline":baseline,"unit":unit,"uniform":control}));
        if index % 32 == 0 {
            println!("Replayed actual and D278 rotation controls at query {query}");
        }
    }
    assert_eq!(output.len(), 318);
    write(
        &root.join(format!("foot-{label}.json")),
        json!({"rows":output,"changed_targets":changed_targets,"changed_inputs":changed_inputs}),
    );
    assert_eq!(
        changed_targets, 190,
        "The D278 world rotation control must be active"
    );
    assert!(changed_inputs > 0);
    for (mode, inputs) in ["unit", "uniform"].into_iter().zip(replacements) {
        write(
            &temporary.join(format!("vm-input-{label}-{mode}.json")),
            json!(inputs),
        );
        vm::run(&temporary, &label, mode);
        let name = format!("vm-{label}-{mode}.json");
        std::fs::rename(temporary.join(&name), root.join(name)).unwrap();
    }
}
