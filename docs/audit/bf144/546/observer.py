from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '545' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/545', 'tools/tmp/bf144/546')
script = script.replace("(root/'target_overhang.inc')", "(root.parent/'545'/'target_overhang.inc')")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/546/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('''    commands.spawn((
        Name::new("AuditWalkOffPlatform"),
        Transform::from_xyz(origin.x,1.9,origin.z),''',
     '''    let floor_pitch=std::env::var("ALS_C546_FLOOR_PITCH").unwrap().parse::<f32>().unwrap();
    let floor_rotation=Quat::from_rotation_y(25.0_f32.to_radians())*Quat::from_rotation_z(floor_pitch.to_radians());
    let floor_top=origin.with_y(2.0)+Vec3::X*1.02;
    let floor_center=floor_top-floor_rotation*Vec3::Y*0.10;
    c516_write("old-floor.json",serde_json::json!({"pitch":floor_pitch,"center":floor_center,
        "top":floor_top,"normal":floor_rotation*Vec3::Y}));
    commands.spawn((
        Name::new("AuditWalkOffPlatform"),
        Transform::from_translation(floor_center).with_rotation(floor_rotation),'''),
    ('''    let center_y=std::env::var("ALS_C545_CENTER_Y").unwrap().parse::<f32>().unwrap();''',
     '''    let pitch=std::env::var("ALS_C546_FLOOR_PITCH").unwrap().parse::<f32>().unwrap();
    let center_y=2.618+0.3/pitch.to_radians().cos();'''),
    ('near_floor_guard: bool, events:', 'floor_control: u8, events:'),
    ('let Some((floor_center, impact_y, _)) = bevy_als::als::motion::find_ground_floor',
     'let Some((floor_center, impact_y, floor_entity)) = bevy_als::als::motion::find_ground_floor'),
    ('''        let initial_floor_gap = center.y - floor_center;''',
     '''        let initial_floor_gap = center.y - floor_center;
        let old_floor = spatial_query.cast_shape_predicate(capsule, center, Quat::IDENTITY, Dir3::NEG_Y,
            &ShapeCastConfig::from_max_distance(0.05), &filter, &accepts).unwrap();
        assert_eq!(old_floor.entity,floor_entity);'''),
    ('''            let normal = if (hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos())
                || (near_floor_guard && hit.normal1.y < -1e-4 && initial_floor_gap < 0.019) {
                hit.normal1.with_y(0.0).normalize_or_zero()
            } else { hit.normal1 };''',
     '''            let near_floor = floor_control > 0 && hit.normal1.y < -1e-4 && initial_floor_gap < 0.019;
            let floor_selected = near_floor && floor_control == 2
                && remaining_delta.dot(old_floor.normal1) < 0.0 && old_floor.normal1.y < 1.0 - 1e-5;
            let normal = if floor_selected {
                old_floor.normal1.with_y(0.0).normalize_or_zero()
            } else if near_floor || (hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos()) {
                hit.normal1.with_y(0.0).normalize_or_zero()
            } else { hit.normal1 };'''),
    ('"advance":advance,"floor_gap_m":initial_floor_gap,"hit":c529_hit(&hit,names)',
     '"advance":advance,"floor_gap_m":initial_floor_gap,"old_floor":c529_hit(&old_floor,names),"hit":c529_hit(&hit,names)'),
    ('"normal_used":normal,', '"normal_used":normal,"floor_selected":floor_selected,'),
    ('    for skip in [false,true] {', '    for (label, floor_control) in [("actual",0),("wall-only",1),("floor-aware",2)] {'),
    ('&candidates,&permissions,skip,&mut events,&names);', '&candidates,&permissions,floor_control,&mut events,&names);'),
    ('if skip {"with-near-floor-guard"} else {"actual"}.to_string()', 'label.to_string()'),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared both floor signs and live floor versus wall normal counterfactuals', flush=True)
