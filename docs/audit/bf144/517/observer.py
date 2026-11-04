from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'516'/'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/516','tools/tmp/bf144/517')
script = script.replace("(root/'ledge_observer.rs')","(root.parent/'516'/'ledge_observer.rs')")
exec(compile(script,previous.name,'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/517/playground_observed.rs')
driver = path.read_text()
key = '        schedule.add_systems(c516_before.after(grounded_dynamics_motor_system).before(transaction));'
assert driver.count(key)==1
driver = driver.replace(key,key+'\n        schedule.add_systems(c517_align.after(c516_before).before(transaction));')
path.write_text(driver+'\n'+(root/'alignment.rs').read_text())
print('Prepared source first-step longitudinal alignment control',flush=True)
