from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'513'/'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/513','tools/tmp/bf144/516')
script = script.replace("(root/'profile.rs')","(root.parent/'513'/'profile.rs')")
exec(compile(script,previous.name,'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/516/playground_observed.rs')
driver = path.read_text()
assert driver.count('(20..60)')==2
driver = driver.replace('(20..60)','(40..62)')
key = 'app.add_systems(First,(c513_profile,c458_drive).chain());'
assert driver.count(key)==1
driver=driver.replace(key,'app.add_systems(First,(c516_setup,c513_profile,c458_drive).chain());')
key='    if let AppExit::Error(code) = app.run() {'
assert driver.count(key)==1
driver=driver.replace(key,'    c516_install(&mut app);\n'+key)
path.write_text(driver+'\n'+(root/'ledge_observer.rs').read_text())
print('Prepared a visible raised static ledge and same-frame transition observation',flush=True)
