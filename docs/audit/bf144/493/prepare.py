import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/493')
temporary.mkdir(parents=True,exist_ok=True)
request = json.loads((parent/'457/request.json').read_text())
case = request['cases'][0]
case['name'] = 'virtual-foot-natural-80'
case['initial_use_foot_ik_bones'] = False
case['frames'] = case['frames'][:158]
request['cases'] = [case]
(root/'virtual-motion.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
case['frames'] = case['frames'][:80]
request['cases'] = [case]
(root/'virtual.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
print('Prepared native virtual-foot motion prefix and initial warm-up control',flush=True)

driver = (parent/'484/playground_observed.rs').read_text()
key = '    app.add_systems(First,c458_drive);'
assert driver.count(key)==1
driver = driver.replace(key,key+'''
    if std::env::var_os("ALS_C493_VIRTUAL").is_some() {
        app.add_observer(c493_virtual_settings);
    }''')
driver = driver.replace('probe.frame >= 240','probe.frame >= 80').replace('probe.frame==240','probe.frame==80')
key = 'std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240","rows":probe.rows,"after_motors":probe.motor_rows,"animation_before":probe.animation_before,"animation_after":probe.animation_after,"camera_after_post_update":probe.camera_rows,"screenshots":0,"performance_acceptance":"paused"})).unwrap()).unwrap();'
assert driver.count(key)==1
driver = driver.replace(key,'c493_write(output,&probe);')
driver += '''
fn c493_virtual_settings(
    added: On<Add,AlsCharacter>,
    mut commands: Commands,
    settings: Query<&bevy_als::control_rig_apply::AlsControlRigSettings>,
) {
    let mut value=settings.get(added.entity).copied().unwrap_or_default();
    value.use_foot_ik_bones=false;
    commands.entity(added.entity).insert(value);
}
fn c493_write(output: String,probe:&C458Probe) {
    let compact=|row:&serde_json::Value| {
        let rig=&row["rig_snapshot"];
        serde_json::json!({"ordinal":row["ordinal"],"foot_feedback":row["foot_feedback"],
            "history_input":row["history_input"],"frame_delta":row["frame_delta"],
            "rig_snapshot":{"poses":rig["poses"],"externals":rig["externals"],
                "initialization_count":rig["initialization_count"]}})
    };
    let before=probe.animation_before.iter().map(compact).collect::<Vec<_>>();
    let after=probe.animation_after.iter().map(compact).collect::<Vec<_>>();
    std::fs::write(output,serde_json::to_vec(&serde_json::json!({"animation_before":before,
        "animation_after":after,"motion_frames":probe.frame,"screenshots":0,"performance_acceptance":"paused"})).unwrap()).unwrap();
}
'''
(temporary/'playground_observed.rs').write_text(driver)
(temporary/'foot_observer.rs').write_bytes((parent/'484/foot_observer.rs').read_bytes())
print('Prepared focused Playground observation and optional initial selector',flush=True)

target = (parent/'484/replay.rs').read_text()
target = target.replace('fn run_case(row: &Value, names: &[&str], parents: &[Option<usize>], corrected: bool)', 'fn run_case(row: &Value, names: &[&str], parents: &[Option<usize>])')
target = target.replace('    let prior = &row["prior"];','    let prior = &row["prior"];\n    let use_ik = observed["use_ik"].as_bool().unwrap();')
target = target.replace('AlsFootTargetSelection { use_ik_bones: true }','AlsFootTargetSelection { use_ik_bones: use_ik }')
a = target.index('        let mut local = transform(&row["locals"][i]);')
b = target.index('        let name = Name::new',a)
target = target[:a]+'        let local = transform(&row["locals"][i]);\n'+target[b:]
target = target.replace('bones.target(0,true)','bones.target(0,use_ik)').replace('bones.target(1,true)','bones.target(1,use_ik)')
target = target.replace('*name=="ik_foot_l"','*name==if use_ik {"ik_foot_l"} else {"VB foot_l"}').replace('*name=="ik_foot_r"','*name==if use_ik {"ik_foot_r"} else {"VB foot_r"}')
a = target.index('fn main()')
b = target.index('    let names',a)
target = target[:a]+'''pub fn run(label: &str) {
    let root = std::path::Path::new("docs/audit/bf144/493");
    let file = format!("tools/tmp/bf144/493/{label}.json");
    let inputs: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
'''+target[b:]
target = target.replace('run_case(row,&names,&parents,false)','run_case(row,&names,&parents)')
target = target.replace('        let corrected = run_case(row,&names,&parents,true);\n','')
target = target.replace(',"corrected":corrected','')
a = target.index('    assert!(output.iter().any(')
target = target[:a]+'    println!("Qualified {} actual foot lock frames",output.len());\n}\n'
(temporary/'target.rs').write_text(target)
print('Prepared qualified production FootIk replay for either marker selector',flush=True)
