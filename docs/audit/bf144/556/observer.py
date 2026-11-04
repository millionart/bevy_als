from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/556')
script = script.replace("(root/'deck_fixture.inc').read_text()", "''").replace("root/'deck.rs'", "root/'residual.inc'")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/556/playground_observed.rs')
driver = path.read_text()
anchor = 'let moving = (40..62).contains(&probe.frame) || (120..160).contains(&probe.frame);'
assert driver.count(anchor) == 1
path.write_text(driver.replace(anchor, 'let moving = ((40..62).contains(&probe.frame) && probe.frame!=49) || (120..160).contains(&probe.frame);'))
print('Prepared read-only no-input air residual observations', flush=True)
