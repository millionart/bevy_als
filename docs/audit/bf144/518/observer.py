from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'516'/'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/516','tools/tmp/bf144/518')
script = script.replace("(root/'ledge_observer.rs')","(root.parent/'516'/'ledge_observer.rs')")
exec(compile(script,previous.name,'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/518/playground_observed.rs')
driver = path.read_text()
key='    c516_install(&mut app);'
assert driver.count(key)==1
driver=driver.replace(key,key+'\n    c518_install(&mut app);')
path.write_text(driver+'\n'+(root/'vertical_observer.rs').read_text())
print('Prepared actual walk-off motor, solver and mode snapshots',flush=True)
