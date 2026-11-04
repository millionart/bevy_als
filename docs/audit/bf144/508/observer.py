from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '505' / 'observer.py'
text = previous.read_text().replace('tools/tmp/bf144/505', 'tools/tmp/bf144/508')
text = text.replace("(root / 'velocity_observer.rs')", "(root.parent / '505' / 'velocity_observer.rs')")
exec(compile(text, previous.name, 'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/508/playground_observed.rs')
driver = path.read_text()
key = '    app.add_systems(First,c458_drive);'
assert driver.count(key) == 1
driver = driver.replace(key, key + '''
    app.add_systems(PostUpdate,c508_configure
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
''')
path.write_text(driver + '\n' + (root / 'snap_observer.rs').read_text())
print('Prepared one-frame native zero half-life branch control', flush=True)
