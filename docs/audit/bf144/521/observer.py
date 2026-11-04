from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'520'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/520','tools/tmp/bf144/521')
script=script.replace("(root/'align_walking.rs')","(root.parent/'520'/'align_walking.rs')")
exec(compile(script,previous.name,'exec'))
root=Path(__file__).resolve().parent
path=Path('tools/tmp/bf144/521/playground_observed.rs')
driver=path.read_text()
key='    c520_install(&mut app);'
assert driver.count(key)==1
driver=driver.replace(key,key+'\n    app.add_systems(Update,c521_trace.after(c516_after).before(c458_sample));')
path.write_text(driver+'\n'+(root/'trace.rs').read_text())
print('Prepared exact live wall sweep and slide replay',flush=True)
