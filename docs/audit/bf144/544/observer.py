from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '537' / 'observer.py'
script = previous.read_text().replace('tools/tmp/bf144/537', 'tools/tmp/bf144/544')
script = script.replace("(root/'target_fin.inc').read_text()",
                        "(root.parent/'537'/'target_fin.inc').read_text()+(root/'target_corner.inc').read_text()")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/544/playground_observed.rs')
driver = path.read_text()
for before, after in (
    ('    skip_floor_guard: bool, events:', '    second_slide: bool, events:'),
    ('if !skip_floor_guard && landing_floor.is_none()', 'if landing_floor.is_none()'),
    ('if skip {"no-floor-guard"} else {"actual"}', 'if skip {"with-second-slide"} else {"actual"}'),
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
                "advance":advance,"hit":c529_hit(&hit,names)}));
            let tail = remaining_delta - advance;'''),
    ('''                progressed += *slide_direction * slide_hit.map_or(slide_distance,
                    |hit| bevy_als::als::motion::movement_sweep_travel(slide_distance, hit.distance));''',
     '''                let slide_travel = slide_hit.as_ref().map_or(slide_distance,
                    |hit| bevy_als::als::motion::movement_sweep_travel(slide_distance, hit.distance));
                events.push(serde_json::json!({"kind":"step_slide","start":raised+progressed,"requested":slide,
                    "advance":*slide_direction*slide_travel,
                    "hit":slide_hit.as_ref().map(|hit|c529_hit(hit,names))}));
                progressed += *slide_direction * slide_travel;
                if second_slide && let Some(hit) = slide_hit {
                    assert!(normal.dot(hit.normal1) > 0.0 && hit.normal1.y.abs() < 1e-3);
                    let projected = (slide - hit.normal1 * slide.dot(hit.normal1)) * (1.0-slide_travel/slide_distance);
                    if projected.dot(remaining_delta) > 0.0
                        && let Ok((direction,distance)) = Dir3::new_and_length(projected) {
                        let config = ShapeCastConfig {max_distance:distance,..forward_config};
                        let hit = spatial_query.cast_shape_predicate(capsule, raised+progressed, Quat::IDENTITY,
                            direction, &config, &filter, &accepts);
                        let travel = hit.as_ref().map_or(distance,
                            |hit| bevy_als::als::motion::movement_sweep_travel(distance, hit.distance));
                        events.push(serde_json::json!({"kind":"step_second_slide","start":raised+progressed,
                            "requested":projected,"advance":*direction*travel,
                            "hit":hit.as_ref().map(|hit|c529_hit(hit,names))}));
                        progressed += *direction * travel;
                    }
                }'''),
):
    assert driver.count(before) == 1, before
    driver = driver.replace(before, after)
path.write_text(driver)
print('Prepared StepUp forward and slide tracing with a read-only second-sweep control', flush=True)
