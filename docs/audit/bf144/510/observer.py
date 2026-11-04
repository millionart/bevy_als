from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '509' / 'observer.py'
text = previous.read_text().replace('tools/tmp/bf144/509', 'tools/tmp/bf144/510')
text = text.replace("(root / 'lean_observer.rs')", "(root.parent / '509' / 'lean_observer.rs')")
exec(compile(text, previous.name, 'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/510/playground_observed.rs')
driver = path.read_text()

def replace(old, new):
    global driver
    assert driver.count(old) == 1, old
    driver = driver.replace(old, new)

replace('"lean":c509_state(player, *locomotion),',
        '"lean":c509_state(player, *locomotion),\n        "acceleration":c510_state(player),')
replace('"lean":row["lean"]', '"lean":row["lean"],"acceleration":row["acceleration"]')
capture = '|ordinal|ordinal>=24)).map(sample)'
assert driver.count(capture) == 2
driver = driver.replace(capture, '|ordinal|ordinal>=17)).map(sample)')
replace('    app.add_systems(First,c458_drive);','''    app.add_systems(First,c458_drive);
    app.add_systems(PostUpdate,c510_edge_input
        .after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));''')
path.write_text(driver + '\n' + (root / 'acceleration_observer.rs').read_text())
print('Prepared actual Grounded acceleration operands and Standing consumer', flush=True)
