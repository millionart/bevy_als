from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'531'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/531','tools/tmp/bf144/532')
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/532/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('docs/audit/bf144/531/native-origins.json','docs/audit/bf144/532/native-origins.json'),
    ('origin.x+2.39,top-0.1',
     'origin.x+std::env::var("ALS_C532_DECK_X_M").unwrap().parse::<f32>().unwrap(),top-0.1'),
    ('''        let (perch, perch_distance, perch_valid, _, _) = compute(at, perch_radius,
            (max_perch - above_base).max(0.0), max_perch + radius, events)?;''',
     '''        let narrow = compute(at, perch_radius, (max_perch - above_base).max(0.0), max_perch + radius, events);
        events.push(serde_json::json!({"kind":"perch_outcome","max_distance_m":max_perch,
            "above_base_m":above_base,"hit":narrow.as_ref().map(|(hit,distance,valid,line,_)|
                serde_json::json!({"entity":c529_name(hit.entity,names),"distance_m":distance,
                    "valid":valid,"line":line,"within_distance":above_base+*distance<=max_perch}))}));
        let (perch, perch_distance, perch_valid, _, _) = narrow?;'''),
    ('''    let is_line = phases.get("post").unwrap()["variants"]["none"]["events"]
        .as_array().unwrap().iter().any(|event|
            (event["kind"]=="selected" && event["line"]==true) ||
            (event["kind"]=="perch_replacement" && std::env::var_os("ALS_C531_REJECT_PERCH").is_some()));''',
     '''    let stop_lacks_floor = phases.get("target-stop").unwrap()["public"].is_null() &&
        !phases.get("post").unwrap()["public"].is_null();'''),
    ('    is_line\n}', '    stop_lacks_floor\n}'),
    ('std::env::var_os("ALS_C530_REQUIRE_CAPSULE").is_some();',
     'std::env::var_os("ALS_C532_REQUIRE_STOP_FLOOR").is_some();'),
    ('let reject_line = frame==control.frame', 'let reject_late_floor = frame==control.frame'),
    ('C522_CLEAR_CONTACT || reject_line','C522_CLEAR_CONTACT || reject_late_floor'),
):
    assert driver.count(anchor)==1,anchor
    driver=driver.replace(anchor,replacement)
path.write_text(driver)
print('Prepared narrow failure tracing and an impact-stop qualification control',flush=True)
