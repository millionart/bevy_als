from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/580')
script = script.replace('deck_fixture.inc', 'corner_fixture.inc')
script = script.replace("(root/'deck.rs')", "(root.parent/'523'/'deck.rs')")
exec(compile(script, previous.name, 'exec'))
driver = path.read_text()
for before, after in (
    ('docs/audit/bf144/523/initial.json','docs/audit/bf144/580/initial.json'),
    ('let moving = (40..62).contains(&probe.frame) || (120..160).contains(&probe.frame);','let moving = false;'),
):
    assert driver.count(before)==1,before
    driver=driver.replace(before,after)
path.write_text(driver)
print('Prepared actual narrow ledge and wall fixture with existing Air and contact observations',flush=True)
