from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '553' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/553', 'tools/tmp/bf144/554')
script = script.replace("(root / 'adjust_probe.inc')", "(root.parent / '553' / 'adjust_probe.inc')")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/554/playground_observed.rs')
driver = path.read_text()
for before, after, count in (
    ('        schedule.add_systems(c536_trace.after(c533_first_step).before(transaction));',
     '''        schedule.add_systems(c536_trace.after(c533_first_step).before(transaction));
        schedule.add_systems(c554_before.after(c516_before).before(transaction));
        schedule.add_systems(c554_after.after(c533_after_transaction).before(c458_sample).before(TnuaPipelineSystems::Motors));''', 1),
    ('if !(control.frame..=control.frame + 1).contains(&frame)',
     'if !(control.frame..=48).contains(&frame)', 2),
):
    assert driver.count(before) == count, before
    driver = driver.replace(before, after)
path.write_text(driver + (root / 'edge_probe.inc').read_text())
print('Prepared low-deck base and motor observations through the departure frame', flush=True)
