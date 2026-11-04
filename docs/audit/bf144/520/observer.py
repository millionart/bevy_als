from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'519'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/519','tools/tmp/bf144/520')
script=script.replace("(root/'wall_fixture.inc')","(root.parent/'519'/'wall_fixture.inc')")
exec(compile(script,previous.name,'exec'))
root=Path(__file__).resolve().parent
path=Path('tools/tmp/bf144/520/playground_observed.rs')
driver=path.read_text()
key='if probe.frame==control.frame {\n        *time=TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.06));'
assert driver.count(key)==1
driver=driver.replace(key, 'if probe.frame==control.frame || probe.frame+1==control.frame {\n        *time=TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(0.06));')
key='    c518_install(&mut app);'
assert driver.count(key)==1
driver=driver.replace(key,key+'\n    c520_install(&mut app);')
path.write_text(driver+'\n'+(root/'align_walking.rs').read_text())
print('Prepared consecutive long frames and actual multi-step barrier observation',flush=True)
