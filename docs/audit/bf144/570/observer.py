from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/570')
script = script.replace("(root/'deck.rs')", "(root.parent/'523'/'deck.rs')")
exec(compile(script, previous.name, 'exec'))

driver = path.read_text()
for before, after in (
    ('docs/audit/bf144/523/initial.json', 'docs/audit/bf144/570/initial.json'),
    ('    c523_install(&mut app);', '    c523_install(&mut app);\n    c570_install(&mut app);'),
    ('let moving = (40..62).contains(&probe.frame) ||', 'let moving = (43..62).contains(&probe.frame) ||'),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver + (root / 'body_probe.inc').read_text())
print('Prepared live dynamic and static deck landing body observations', flush=True)
