from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'513'/'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/513','tools/tmp/bf144/514')
script = script.replace("(root/'profile.rs')","(root.parent/'513'/'profile.rs')")
exec(compile(script,previous.name,'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/514/playground_observed.rs')
driver = path.read_text()
key = '    if let AppExit::Error(code) = app.run() {'
assert driver.count(key)==1
driver = driver.replace(key, '    c514_install(&mut app);\n'+key)
path.write_text(driver+'\n'+(root/'cache_control.rs').read_text())
print('Prepared single-frame cached acceleration plan control',flush=True)
