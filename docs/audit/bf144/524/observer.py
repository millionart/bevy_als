from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'523'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/523','tools/tmp/bf144/524')
for name in ('deck_fixture.inc', 'deck.rs'):
    script=script.replace(f"(root/'{name}')",f"(root.parent/'523'/'{name}')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/524/playground_observed.rs')
driver=path.read_text()
anchor='        let center=Vec3::new(origin.x+wall_cm as f32*0.01,3.0,origin.z);'
assert driver.count(anchor)==1
driver=driver.replace(anchor,anchor+'''
        let pitch=std::env::var("ALS_C524_PITCH_DEG").unwrap().parse::<f32>().unwrap();
        let rotation=Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4)*Quat::from_rotation_z(pitch.to_radians());''')
anchor='Transform::from_translation(center).with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4)),'
assert driver.count(anchor)==1
driver=driver.replace(anchor,'Transform::from_translation(center).with_rotation(rotation),')
anchor='"size":[0.04,4.0,2.0],"yaw_source_degrees":45.0}'
assert driver.count(anchor)==1
driver=driver.replace(anchor,'"size":[0.04,4.0,2.0],"yaw_source_degrees":45.0,"pitch_source_degrees":pitch,"rotation":rotation}')
path.write_text(driver)
print('Prepared positive and negative wall pitch in the existing Playground fixture',flush=True)
