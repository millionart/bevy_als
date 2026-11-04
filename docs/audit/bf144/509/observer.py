from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '505' / 'observer.py'
text = previous.read_text().replace('tools/tmp/bf144/505', 'tools/tmp/bf144/509')
text = text.replace("(root / 'velocity_observer.rs')", "(root.parent / '505' / 'velocity_observer.rs')")
exec(compile(text, previous.name, 'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/509/playground_observed.rs')
driver = path.read_text()

def replace(old, new):
    global driver
    assert driver.count(old) == 1, old
    driver = driver.replace(old, new)

replace('    app.add_systems(First,c458_drive);','''    app.add_systems(First,c458_drive);
    app.add_systems(PostUpdate,c509_config_input
        .after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
    app.add_systems(PostUpdate,c509_install_ground_control
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));''')
replace('"velocity_blend":c505_state(player, &graphs, before),',
        '"velocity_blend":c505_state(player, &graphs, before),\n        "lean":c509_state(player, *locomotion),')
replace('"velocity_blend":row["velocity_blend"]', '"velocity_blend":row["velocity_blend"],"lean":row["lean"]')
path.write_text(driver + '\n' + (root / 'lean_observer.rs').read_text())
print('Prepared both Lean consumers in the existing visible jump scenario', flush=True)
