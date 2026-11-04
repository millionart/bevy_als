from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '484'
scratch = Path('tools/tmp/bf144/505')
scratch.mkdir(parents=True, exist_ok=True)
driver = (previous / 'playground_observed.rs').read_text()

def replace(old, new):
    global driver
    assert driver.count(old) == 1, old
    driver = driver.replace(old, new)

replace('probe.frame >= 240 { return; }\n    let Some(admission)', 'probe.frame >= 80 { return; }\n    let Some(admission)')
replace('if probe.frame >= 240 { return; }','if probe.frame >= 80 { return; }')
replace('if probe.frame==240 {','if probe.frame==80 {')
replace('move_dir: if moving { Vec2::Y } else { Vec2::ZERO }, ..default()',
        'move_dir: if moving { Vec2::Y } else { Vec2::ZERO }, jump_pressed: probe.frame == 34, jump_held: (34..40).contains(&probe.frame), ..default()')
replace('    app.add_systems(First,c458_drive);','''    app.add_systems(First,c458_drive);
    app.add_systems(PostUpdate,c505_configure
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));''')
replace('        "ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),',
        '        "ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),\n        "velocity_blend":c505_state(player, &graphs, before),')
begin = driver.index('        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240"')
end = driver.index('\n', begin)
driver = driver[:begin] + '''        let sample=|row:&serde_json::Value|serde_json::json!({
            "ordinal":row["ordinal"],"animation_frame":row["arena_frame"],"gameplay":row["history_input"]["locomotion"],
            "graph_tag":row["history_input"]["root_tags"]["locomotion_mode"],"input":row["history_input"]["effective_move"],
            "position":row["position"],"velocity":row["velocity"],"graph_error":row["graph_error"],
            "velocity_blend":row["velocity_blend"]
        });
        let before=probe.animation_before.iter().filter(|row|row["ordinal"].as_u64().is_some_and(|ordinal|ordinal>=24)).map(sample).collect::<Vec<_>>();
        let after=probe.animation_after.iter().filter(|row|row["ordinal"].as_u64().is_some_and(|ordinal|ordinal>=24)).map(sample).collect::<Vec<_>>();
        std::fs::write(output,serde_json::to_vec(&serde_json::json!({"before":before,"after":after})).unwrap()).unwrap();''' + driver[end:]
(scratch / 'playground_observed.rs').write_text(driver + '\n' + (root / 'velocity_observer.rs').read_text())
(scratch / 'foot_observer.rs').write_bytes((previous / 'foot_observer.rs').read_bytes())
print('Prepared Grounded leave and reentry observation', flush=True)
