from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'529'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/529','tools/tmp/bf144/530')
script=script.replace("(root/'floor_trace.rs')","(root.parent/'529'/'floor_trace.rs')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/530/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('docs/audit/bf144/529/native-origins.json','docs/audit/bf144/530/native-origins.json'),
    ('origin.x+2.3,top-0.1','origin.x+2.24,top-0.1'),
    ('    c516_write("fixture.json"', (root/'target_blocker.inc').read_text()+'    c516_write("fixture.json"'),
    ('''    if frame==control.frame {
        c529_trace(&spatial,position.0,half,radius,&params.cmc,*mode,actor,&candidates,&names);
    }''', '''    let reject_line = frame==control.frame &&
        c529_trace(&spatial,position.0,half,radius,&params.cmc,*mode,actor,&candidates,&names) &&
        std::env::var_os("ALS_C530_REQUIRE_CAPSULE").is_some();'''),
    ('if C522_CLEAR_CONTACT && frame == control.frame {',
     'if (C522_CLEAR_CONTACT || reject_line) && frame == control.frame {'),
    ('''    names: &Query<&Name>,
) {
    let output =''', '''    names: &Query<&Name>,
) -> bool {
    let output ='''),
    ('''    c516_write(
        "floor-provenance.json",''', '''    let is_line = phases.get("post").unwrap()["variants"]["none"]["events"]
        .as_array().unwrap().iter().any(|event| event["kind"]=="selected" && event["line"]==true);
    c516_write(
        "floor-provenance.json",'''),
    ('''            "phases":phases,"half_m":half,"radius_m":radius,"mode":mode,"perch_threshold_m":cmc.perch_radius_threshold,
        }),
    );
}''', '''            "phases":phases,"half_m":half,"radius_m":radius,"mode":mode,"perch_threshold_m":cmc.perch_radius_threshold,
        }),
    );
    is_line
}'''),
):
    assert driver.count(anchor)==1
    driver=driver.replace(anchor,replacement)
path.write_text(driver)
print('Prepared real slanted floor blocker and center-ray geometry in Playground',flush=True)
