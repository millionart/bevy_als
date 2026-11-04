from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '505' / 'observer.py'
scratch = Path('tools/tmp/bf144/506')
script = previous.read_text().replace('tools/tmp/bf144/505', str(scratch))
script = script.replace("(root / 'velocity_observer.rs').read_text()", "''")
exec(compile(script, previous.name, 'exec'))
root = Path(__file__).resolve().parent
path = scratch / 'playground_observed.rs'
driver = path.read_text()
def replace(old, new):
    global driver
    assert driver.count(old) == 1, old
    driver = driver.replace(old, new)

driver = driver.replace('probe.frame >= 80', 'probe.frame >= 120').replace('probe.frame==80','probe.frame==120')
replace('mut actors: Query<(&mut bevy_als::als::gait::AlsDesiredGait,',
        'mut actors: Query<(&mut Transform, &mut Position, &mut LinearVelocity, &mut bevy_als::als::gait::AlsDesiredGait,')
replace('let Ok((mut gait, mut stance, mut view, mut mode))',
        'let Ok((mut transform, mut position, mut velocity, mut gait, mut stance, mut view, mut mode))')
replace('    let moving = (20..60).contains(&probe.frame) || (120..160).contains(&probe.frame);',
        '''    if probe.frame == 34 {
        transform.translation.y += 3.48;
        position.0 = transform.translation;
        velocity.0 = Vec3::ZERO;
    }
    let moving = (34..110).contains(&probe.frame);''')
replace(', jump_pressed: probe.frame == 34, jump_held: (34..40).contains(&probe.frame)', '')
replace('characters: Query<(&bevy_als::actual_motion::AlsActualMotion,&Position,&LinearVelocity)>',
        'characters: Query<(&bevy_als::actual_motion::AlsActualMotion,&Position,&LinearVelocity,Option<&bevy_als::als::roll::AlsRollState>)>')
replace('let Ok((motion,position,velocity))=characters.get(parent.parent())',
        'let Ok((motion,position,velocity,roll))=characters.get(parent.parent())')
replace('"velocity_blend":c505_state(player, &graphs, before),',
        '''"velocity_blend":c505_state(player, &graphs, before),
        "post_locomotion":c506_slot(player),
        "roll":roll.map(|roll|serde_json::json!({"active":roll.active,"time":roll.time})),''')
replace('"velocity_blend":row["velocity_blend"]', '''"velocity_blend":row["velocity_blend"],
            "post_locomotion":row["post_locomotion"],"roll":row["roll"],"action":row["history_input"]["root_tags"]["locomotion_action"]''')
replace('PostUpdate,c505_configure','PostUpdate,c506_configure')
velocity = (root.parent / '505' / 'velocity_observer.rs').read_text()
velocity = velocity[velocity.index('fn c505_state('):]
path.write_text(driver + '\n' + velocity + '\n' + (root / 'slot_observer.rs').read_text())
print('Prepared real Playground drop and PostLocomotion observation', flush=True)
