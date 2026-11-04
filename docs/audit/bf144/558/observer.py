from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/558')
script = script.replace('deck_fixture.inc', 'corner_fixture.inc').replace("root/'deck.rs'", "root/'corner_probe.inc'")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/558/playground_observed.rs')
driver = path.read_text()
anchor = 'let moving = (40..62).contains(&probe.frame) || (120..160).contains(&probe.frame);'
assert driver.count(anchor) == 1
driver = driver.replace(anchor, '''let corner=probe.frame==49;
    let moving = if corner {true}
        else {(40..62).contains(&probe.frame) || (120..160).contains(&probe.frame)};''')
anchor = 'move_dir: if moving { Vec2::Y } else { Vec2::ZERO }'
assert driver.count(anchor) == 1
path.write_text(driver.replace(anchor, 'move_dir: if !moving {Vec2::ZERO} else if corner {if std::env::var("ALS_C558_AWAY").unwrap()=="1" {Vec2::new(0.8660254,-0.5)} else {Vec2::X}} else {Vec2::Y}'))
print('Prepared second-impact air-control counterfactual in the same real collision geometry', flush=True)
