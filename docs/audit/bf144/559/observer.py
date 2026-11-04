from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/559')
script = script.replace('deck_fixture.inc', 'corner_fixture.inc').replace("root/'deck.rs'", "root/'corner_probe.inc'")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/559/playground_observed.rs')
driver = path.read_text()
anchor = 'let moving = (40..62).contains(&probe.frame) || (120..160).contains(&probe.frame);'
assert driver.count(anchor) == 1
path.write_text(driver.replace(anchor, 'let moving = false;'))
print('Prepared real third-contact and lateral probe observations', flush=True)
