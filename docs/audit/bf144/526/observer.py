from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '524' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/524', 'tools/tmp/bf144/526')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/526/playground_observed.rs')
driver = path.read_text()
for anchor, replacement in (
    ('        let center=Vec3::new(origin.x+wall_cm as f32*0.01,3.0,origin.z);',
     '''        let top=2.0+0.01*std::env::var("ALS_C526_WALL_TOP_CM").unwrap().parse::<f32>().unwrap();
        let height=top-1.0;
        let center=Vec3::new(origin.x+wall_cm as f32*0.01,1.0+height*0.5,origin.z);'''),
    ('Collider::cuboid(0.04,4.0,2.0)', 'Collider::cuboid(0.04,height,2.0)'),
    ('Cuboid::new(0.04,4.0,2.0)', 'Cuboid::new(0.04,height,2.0)'),
    ('"size":[0.04,4.0,2.0]', '"size":[0.04,height,2.0],"top_m":top'),
    ('''        let input:serde_json::Value=serde_json::from_slice(&std::fs::read("docs/audit/bf144/523/initial.json").unwrap()).unwrap();
        let top=input["deck_top_m"].as_f64().unwrap() as f32;''',
     '''        let top=std::env::var("ALS_C526_DECK_TOP_M").unwrap().parse::<f32>().unwrap();'''),
):
    assert driver.count(anchor) == 1, anchor
    driver = driver.replace(anchor, replacement)
path.write_text(driver)
print('Prepared real Playground wall-edge geometry and independent deck height', flush=True)
