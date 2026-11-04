from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'530'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/530','tools/tmp/bf144/531')
script=script.replace("(root/'target_blocker.inc')","(root.parent/'530'/'target_blocker.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/531/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('docs/audit/bf144/530/native-origins.json','docs/audit/bf144/531/native-origins.json'),
    ('docs/audit/bf144/530/fixture-input.json','docs/audit/bf144/531/fixture-input.json'),
    ('origin.x+2.24,top-0.1','origin.x+2.39,top-0.1'),
    ('        if !valid { hit = perch; }',
     '''        if !valid {
            events.push(serde_json::json!({"kind":"perch_replacement","broad":c529_hit(&hit,names),
                "perch":c529_hit(&perch,names),"broad_distance_m":floor_distance,"perch_distance_m":perch_distance}));
            hit = perch;
        }'''),
    ('.as_array().unwrap().iter().any(|event| event["kind"]=="selected" && event["line"]==true);',
     '''.as_array().unwrap().iter().any(|event|
            (event["kind"]=="selected" && event["line"]==true) ||
            (event["kind"]=="perch_replacement" && std::env::var_os("ALS_C531_REJECT_PERCH").is_some()));'''),
):
    assert driver.count(anchor)==1
    driver=driver.replace(anchor,replacement)
path.write_text(driver)
print('Prepared real center-miss geometry and selective perch replacement consumption control',flush=True)
