from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'536'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/536','tools/tmp/bf144/537')
script=script.replace("(root/'step_probe.rs')","(root.parent/'536'/'step_probe.rs')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/537/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('    c516_write("fixture.json"', (root/'target_fin.inc').read_text()+'    c516_write("fixture.json"'),
    ('position.0 = Vec3::new(-14.01, 2.9215, -15.0);','position.0 = Vec3::new(-13.98, 2.9215, -15.0);'),
    ('velocity.0 = Vec3::new(6.0, 0.0, 0.0);','velocity.0 = Vec3::new(6.5, 0.0, 0.0);'),
):
    assert driver.count(anchor)==1,anchor
    driver=driver.replace(anchor,replacement)
start=driver.index('fn c516_delta(')
stop=driver.index('fn c516_cache(',start)
driver=driver[:start]+driver[start:stop].replace('from_secs_f32(0.03)','from_secs_f32(0.05)')+driver[stop:]
start=driver.index('fn c536_step(')
step=driver[start:]
anchor='    let destination = raised + remaining_delta;'
assert step.count(anchor)==1
step=step.replace(anchor,anchor+'''
    events.push(serde_json::json!({"kind":"down_request","start":destination,
        "end":destination-Vec3::Y*(C529_MAX_STEP_HEIGHT+2.0*C529_MAX_FLOOR_DISTANCE)}));''')
path.write_text(driver[:start]+step)
print('Prepared the real off-axis fin and StepDown miss trace',flush=True)
