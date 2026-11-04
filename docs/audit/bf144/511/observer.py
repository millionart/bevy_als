from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'510/observer.py'
script = previous.read_text().replace('tools/tmp/bf144/510','tools/tmp/bf144/511')
script = script.replace("(root / 'acceleration_observer.rs')","(root.parent / '510' / 'acceleration_observer.rs')")
exec(compile(script,previous.name,'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/511/playground_observed.rs')
driver = path.read_text()

def replace(old,new):
    global driver
    assert driver.count(old)==1,old
    driver=driver.replace(old,new)

replace('    app.add_systems(First,c458_drive);','''    app.add_systems(First,c458_drive);
    app.add_systems(PostUpdate,c511_configure
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));''')
replace('"acceleration":c510_state(player),', '''"acceleration":c510_state(player),
        "standing_consumer":if !before && matches!(probe.frame,20..=24|60..=65) {c511_consumer(player,&graphs,&skeletons,&assets)} else {serde_json::Value::Null},''')
replace('"acceleration":row["acceleration"]', '''"acceleration":row["acceleration"],
            "standing_consumer":row["standing_consumer"],
            "rig_poses":if row["standing_consumer"].is_null() {serde_json::Value::Null} else {row["rig_snapshot"]["poses"].clone()}''')
path.write_text(driver+'\n'+(root/'consumer.rs').read_text())
print('Prepared actual Standing Lean and final pose readouts',flush=True)
