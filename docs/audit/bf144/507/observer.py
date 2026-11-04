from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '506' / 'observer.py'
text = previous.read_text()
text = text.replace('tools/tmp/bf144/506', 'tools/tmp/bf144/507')
text = text.replace("(root / 'slot_observer.rs')", "(root.parent / '506' / 'slot_observer.rs')")
exec(compile(text, previous.name, 'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/507/playground_observed.rs')
driver = path.read_text()
key = '    app.add_systems(Last,c458_late);'
assert driver.count(key) == 1
driver = driver.replace(key, key + '''
    app.add_systems(Last,c507_edge_step);
    app.add_systems(PostUpdate,c507_slot_control
        .after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
''')
path.write_text(driver + '\n' + (root / 'edge_observer.rs').read_text())
print('Prepared single-step Roll fade edge control', flush=True)
