from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/557')
script = script.replace('deck_fixture.inc', 'corner_fixture.inc').replace("root/'deck.rs'", "root/'corner_probe.inc'")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/557/playground_observed.rs')
driver = path.read_text()
anchor = 'let moving = (40..62).contains(&probe.frame) || (120..160).contains(&probe.frame);'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, '''let corner=probe.frame==49;
    let moving = if corner {std::env::var("ALS_C557_INPUT").unwrap()=="1"}
        else {(40..62).contains(&probe.frame) || (120..160).contains(&probe.frame)};''')
anchor = 'move_dir: if moving { Vec2::Y } else { Vec2::ZERO }'
assert driver.count(anchor) == 1
path.write_text(driver.replace(anchor, 'move_dir: if !moving {Vec2::ZERO} else if corner {Vec2::X} else {Vec2::Y}'))
print('Prepared second-impact air-control counterfactual in the same real collision geometry', flush=True)
