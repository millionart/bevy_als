from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '537' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/537', 'tools/tmp/bf144/545')
script = script.replace("(root/'target_fin.inc').read_text()",
                        "(root.parent/'537'/'target_fin.inc').read_text()+(root/'target_overhang.inc').read_text()")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/545/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('    position.0 = Vec3::new(-13.98, 2.9215, -15.0);',
     '    let center_y=std::env::var("ALS_C545_CENTER_Y").unwrap().parse::<f32>().unwrap();\n    position.0 = Vec3::new(-13.98, center_y, -15.0);'),
    ('        initial_floor_y = Some(impact_y);',
     '        initial_floor_y = Some(impact_y);\n        let initial_floor_gap = center.y - floor_center;'),
    ('''            let normal = if hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos() {''',
     '''            let normal = if (hit.normal1.y > 0.0 && hit.normal1.y < c529_deg_to_rad(C529_MAX_SLOPE_DEGREES).cos())
                || (near_floor_guard && hit.normal1.y < -1e-4 && initial_floor_gap < 0.019) {'''),

    ('    skip_floor_guard: bool, events:', '    near_floor_guard: bool, events:'),
    ('if !skip_floor_guard && landing_floor.is_none()', 'if landing_floor.is_none()'),
    ('if skip {"no-floor-guard"} else {"actual"}', 'if skip {"with-near-floor-guard"} else {"actual"}'),
    ('''        let lift = spatial_query.cast_shape_predicate(capsule, raised, Quat::IDENTITY, Dir3::Y, &up_config, &filter, &accepts)
            .map_or(up_config.max_distance, |hit| bevy_als::als::motion::movement_sweep_travel(up_config.max_distance, hit.distance));
        raised.y += lift;''',
     '''        let up_hit = spatial_query.cast_shape_predicate(capsule, raised, Quat::IDENTITY, Dir3::Y, &up_config, &filter, &accepts);
        let lift = up_hit.as_ref().map_or(up_config.max_distance,
            |hit| bevy_als::als::motion::movement_sweep_travel(up_config.max_distance, hit.distance));
        events.push(serde_json::json!({"kind":"lift","start":raised,"advance_m":lift,
            "hit":up_hit.as_ref().map(|hit|c529_hit(hit,names))}));
        raised.y += lift;'''),
    ('''            let tail = remaining_delta - advance;''',
     '''            events.push(serde_json::json!({"kind":"step_forward","start":raised,"requested":remaining_delta,
                "advance":advance,"floor_gap_m":initial_floor_gap,"hit":c529_hit(&hit,names)}));
            let tail = remaining_delta - advance;'''),
    ('''                progressed += *slide_direction * slide_hit.map_or(slide_distance,
                    |hit| bevy_als::als::motion::movement_sweep_travel(slide_distance, hit.distance));''',
     '''                let slide_travel = slide_hit.as_ref().map_or(slide_distance,
                    |hit| bevy_als::als::motion::movement_sweep_travel(slide_distance, hit.distance));
                events.push(serde_json::json!({"kind":"step_slide","start":raised+progressed,"requested":slide,
                    "advance":*slide_direction*slide_travel,"normal_used":normal,
                    "hit":slide_hit.as_ref().map(|hit|c529_hit(hit,names))}));
                progressed += *slide_direction * slide_travel;'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared actual StepUp slide and its negative-normal floor guard control', flush=True)
