from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'518'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/518','tools/tmp/bf144/519')
script=script.replace("(root/'vertical_observer.rs')","(root.parent/'518'/'vertical_observer.rs')")
exec(compile(script,previous.name,'exec'))
root=Path(__file__).resolve().parent
path=Path('tools/tmp/bf144/519/playground_observed.rs')
driver=path.read_text()
key='    pos.0.y+=2.0;'
assert driver.count(key)==1
driver=driver.replace(key,(root/'wall_fixture.inc').read_text()+key)
path.write_text(driver)
print('Prepared the static diagonal barrier in the existing ledge fixture',flush=True)
