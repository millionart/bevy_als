from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'512'/'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/512','tools/tmp/bf144/513')
script = script.replace("(root/'sprint_observer.rs')","(root.parent/'512'/'sprint_observer.rs')")
exec(compile(script,previous.name,'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/513/playground_observed.rs')
driver = path.read_text()

def replace(old,new):
    global driver
    assert driver.count(old)==1,old
    driver=driver.replace(old,new)

replace('app.add_systems(First,c458_drive);',
        'app.add_systems(First,(c513_profile,c458_drive).chain());')
replace('"before":before,"after":after', '''"before":before,"after":after,
            "dynamics":probe.rows.iter().map(|row|serde_json::json!({
                "ordinal":row["ordinal"],"rotation_mode":row["rotation_mode"],"stance":row["stance"],
                "motion":row["desired_motion"],"gait_amount":row["gait_amount"],"thresholds":row["thresholds"],
                "dynamics":row["dynamics"],"public":row["public_sample"],"delta":row["delta"],
                "planned_velocity":row["planned_velocity"],"world_velocity_at_dynamics":row["world_velocity_at_dynamics"],
                "max_speed":row["max_speed"],"input":row["input_move"],"has_base":row["has_base"]
            })).collect::<Vec<_>>()''')
path.write_text(driver+'\n'+(root/'profile.rs').read_text())
print('Prepared single-character dynamics profile control',flush=True)
