from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'532'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/532','tools/tmp/bf144/533')
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/533/playground_observed.rs')
driver=path.read_text()
for anchor,replacement in (
    ('    c523_install(&mut app);','    c533_install(&mut app);'),
    ('''    let reject_late_floor = frame==control.frame &&
        c529_trace(&spatial,position.0,half,radius,&params.cmc,*mode,actor,&candidates,&names) &&
        std::env::var_os("ALS_C532_REQUIRE_STOP_FLOOR").is_some();''',''),
    ('if (C522_CLEAR_CONTACT || reject_late_floor) && frame == control.frame {',
     'if C522_CLEAR_CONTACT && frame == control.frame {'),
):
    assert driver.count(anchor)==1,anchor
    driver=driver.replace(anchor,replacement)
path.write_text(driver+'\n'+(root/'walking_probe.rs').read_text())
print('Prepared live Walking first-step and transaction observations',flush=True)
