from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'510'/'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/510','tools/tmp/bf144/512')
script = script.replace("(root / 'acceleration_observer.rs')","(root.parent / '510' / 'acceleration_observer.rs')")
exec(compile(script,previous.name,'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/512/playground_observed.rs')
driver = path.read_text()

def replace(old,new):
    global driver
    assert driver.count(old)==1,old
    driver=driver.replace(old,new)

replace('gait.0 = bevy_als::als::state::Gait::Running;',
        'gait.0 = if (20..60).contains(&probe.frame) { bevy_als::als::state::Gait::Sprinting } else { bevy_als::als::state::Gait::Running };')
replace('*mode = bevy_als::als::state::RotationMode::ViewDirection;',
        '*mode = bevy_als::als::state::RotationMode::VelocityDirection;')
replace('jump_pressed: probe.frame == 34, jump_held: (34..40).contains(&probe.frame)', 'sprint_held: moving')
replace('    app.add_systems(First,c458_drive);','''    app.add_systems(First,c458_drive);
    app.add_systems(PostUpdate,c512_configure
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));''')
replace('"acceleration":c510_state(player),', '''"acceleration":c510_state(player),
        "sprint":c512_sprint(player, &graphs, &skeletons, &assets, before),''')
replace('"acceleration":row["acceleration"]', '''"acceleration":row["acceleration"],"sprint":row["sprint"],
            "gait":row["history_input"]["gait"],"desired_gait":row["history_input"]["desired_gait"],
            "sprint_leaves":row["leaf_nodes"].as_array().into_iter().flatten().filter(|item|
                item["name"].as_str().is_some_and(|name|name.starts_with("A_Als_Sprint")) &&
                item["clocks"].as_array().is_some_and(|clocks|!clocks.is_empty())).collect::<Vec<_>>(),
            "rig_poses":if row["sprint"]["mixed"].as_array().is_some_and(|values|!values.is_empty())
                && row["ordinal"].as_u64().is_some_and(|ordinal|ordinal < 50) {row["rig_snapshot"]["poses"].clone()} else {serde_json::Value::Null}''')
path.write_text(driver+'\n'+(root/'sprint_observer.rs').read_text())
print('Prepared natural Sprint entry, active blend and cutoff readouts',flush=True)
