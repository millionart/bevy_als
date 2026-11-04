from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'538'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/538','tools/tmp/bf144/539')
script=script.replace("(root/'solver_probe.rs')","(root.parent/'538'/'solver_probe.rs')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/539/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('origin.x+1.145,2.08,origin.z+0.27','origin.x+1.145,2.073,origin.z+0.27'),
    ('    c538_install(&mut app);','''    c538_install(&mut app);
    app.add_systems(PostUpdate,c539_control.after(PhysicsSystems::Last).before(c522_before));'''),
):
    assert driver.count(anchor)==1,anchor
    driver=driver.replace(anchor,replacement)
path.write_text(driver+'\n'+(root/'gap_probe.rs').read_text())
print('Prepared natural grazing step and isolated post-physics gap control',flush=True)
