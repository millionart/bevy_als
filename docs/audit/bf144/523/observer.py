from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'522'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/522','tools/tmp/bf144/523')
script=script.replace("(root/'contact.rs')","(root.parent/'522'/'contact.rs')")
exec(compile(script,previous.name,'exec'))
root=Path(__file__).resolve().parent
path=Path('tools/tmp/bf144/523/playground_observed.rs')
driver=path.read_text()
for hook in ('    c518_install(&mut app);\n', '    c520_install(&mut app);\n',
             '    app.add_systems(Update,c521_trace.after(c516_after).before(c458_sample));\n'):
    assert driver.count(hook)==1
    driver=driver.replace(hook,'')
for anchor,replacement in (
    ('    c522_install(&mut app);','    c522_install(&mut app);\n    c523_install(&mut app);'),
    ('    app.add_systems(Last,c458_late);', '''    app.edit_schedule(Last, |schedule| {
        let matches: Vec<_> = schedule.graph().systems.iter().filter(|(_, system, _)|
            system.name().to_string().ends_with("::flush_animation_visual_report")).collect();
        assert_eq!(matches.len(), 1);
        let flush = matches[0].1.default_system_sets()[0];
        schedule.add_systems(c458_late.before(flush));
    });'''),
    ('    c516_write("fixture.json"', (root/'deck_fixture.inc').read_text()+'    c516_write("fixture.json"'),
):
    assert driver.count(anchor)==1
    driver=driver.replace(anchor,replacement)
driver=driver.replace('const C522_CLEAR_CONTACT: bool = false;', (root/'contact_control.inc').read_text().strip())
path.write_text(driver+'\n'+(root/'deck.rs').read_text())
print('Prepared live Air initialization and a low static landing deck',flush=True)
