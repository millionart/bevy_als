from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'539'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/539','tools/tmp/bf144/540')
script=script.replace("(root/'gap_probe.rs')","(root.parent/'539'/'gap_probe.rs')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/540/playground_observed.rs')
driver=path.read_text()
anchor='    app.add_systems(PostUpdate,c539_control.after(PhysicsSystems::Last).before(c522_before));'
assert driver.count(anchor)==1
driver=driver.replace(anchor,anchor+'\n    app.add_systems(PostUpdate,c540_probe.after(c539_control).before(c522_before));')
path.write_text(driver+'\n'+(root/'fresh_probe.rs').read_text())
print('Prepared the raw fresh floor capsule trace at the real sync input',flush=True)
