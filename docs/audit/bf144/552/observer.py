from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '551' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/551', 'tools/tmp/bf144/552')
for name in ('target_wall.inc', 'substeps.inc'):
    script = script.replace(f"(root / '{name}')", f"(root.parent / '551' / '{name}')")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/552/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('    c533_install(&mut app);',
     '''    c533_install(&mut app);
    app.add_systems(Update,c552_stop.after(c533_after_transaction).before(c458_sample).before(TnuaPipelineSystems::Motors));'''),
    ('''    let boolean=|key|player.get_global_input_data(key).and_then(|v|v.as_bool().ok());''',
     '''    let boolean=|key|player.get_global_input_data(key).and_then(|v|v.as_bool().ok());
    if probe.frame==41 && !before {
        c516_write("locomotion-consumer.json",serde_json::json!({
            "ordinal":40,"velocity":motion.world_velocity,"speed":motion.horizontal_speed,
            "has_input":motion.has_input,"moving":boolean("locomotion_state.b_moving"),
            "moving_smooth":boolean("locomotion_state.b_moving_smooth"),
            "graph_speed":scalar("locomotion_state.speed"),"raw_velocity":velocity.0,
        }));
    }'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver + (root / 'stop_probe.inc').read_text())
print('Prepared stopped-first-step handoff and live locomotion provider reads', flush=True)
