from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/564')
script = script.replace("(root/'deck_fixture.inc').read_text()", "''").replace("root/'deck.rs'", "root/'speed_probe.inc'")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/564/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('gait.0 = if (40..62).contains(&probe.frame) { bevy_als::als::state::Gait::Sprinting } else { bevy_als::als::state::Gait::Running };',
     'gait.0 = bevy_als::als::state::Gait::Running;'),
    ('stance.0 = bevy_als::als::state::Stance::Standing;',
     '''stance.0 = if std::env::var("ALS_C564_CROUCH").unwrap()=="1" {
        bevy_als::als::state::Stance::Crouching
    } else {bevy_als::als::state::Stance::Standing};'''),
    ('let moving = (40..62).contains(&probe.frame) || (120..160).contains(&probe.frame);',
     'let moving = (49..52).contains(&probe.frame);'),
    ('sprint_held: moving,', 'sprint_held: false,'),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared real posture readouts and same-input speed-cap counterfactuals', flush=True)
