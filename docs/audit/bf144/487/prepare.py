import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
temporary = Path('tools/tmp/bf144/487')
temporary.mkdir(parents=True,exist_ok=True)
for name,path in [('foot-input','484/inputs.json.gz'),('grid','486/replay-restored.json.gz')]:
    (temporary/f'{name}.json').write_bytes(gzip.decompress((root.parent/path).read_bytes()))
    print(f'Restored {name} from tracked evidence',flush=True)

production = Path('src/als/character/foot_ik_apply.rs')
text = production.read_text()
def section(value, start, end):
    assert value.count(start)==1, start
    start_at = value.index(start)
    return value[start_at:value.index(end,start_at)]

helpers = section(text,'fn refresh_previous_foot_lock_transform(','fn refresh_foot_lock_inputs(')
lock = section(text,'            if became_valid {\n                reset_foot_lock_to_target',
               '            let chain = bones.chain(side);')
lock = lock.replace('target.translation()', 'target_location').replace('target.rotation()', 'target_rotation')
publish = section(text,'            input.foot_transforms_valid = lock_state.valid;',
                  '        }\n        dynamic_transition.update(')
(temporary/'production.rs').write_text(helpers+'''
fn consume_target(
    foot_state: &mut AlsFootLockFootState,
    target_location: Vec3,
    target_rotation: Quat,
    ik_amount: f32,
    lock_amount: f32,
    settings: &AlsFootLockSettings,
    lock_input: &AlsFootLockInput,
    locomotion: &LocomotionState,
    became_valid: bool,
    dt: f32,
    current_base: Option<AlsFootLockMovementBase>,
) {
'''+lock+'''}
fn publish_inputs(input: &mut AlsControlRigInput, armature: GlobalTransform, lock_state: &AlsFootLockState) {
'''+publish+'}\n')
fixture = (root.parent/'484/replay.rs').read_text()
(temporary/'foot_types.rs').write_text(section(fixture,'fn number(','fn world_value('))

vm = (root.parent/'485/replay.rs').read_text()
a = vm.index('mod native;')
b = vm.index('    let data: Value',a)
vm = vm[:a]+'''pub fn run(r: &std::path::Path, label: &str, mode: &str) {
'''+vm[b:]
vm = vm.replace('r.join("vm-input.json")','r.join(format!("vm-input-{label}-{mode}.json"))')
vm = vm.replace('r.join(format!("replay-{label}.json"))','r.join(format!("vm-{label}-{mode}.json"))')
vm = vm[:vm.index('    assert_eq!(\n        changed_sides, 103,')]+'}\n'
(temporary/'vm.rs').write_text(vm)
print('Extracted current target lock and publish statements and reused calibrated VM replay',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts = []
for base, key, ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(1277,1295),(1300,1327),(1382,1440)]),
    (engine,'Engine/Source/Runtime/Engine/Private/SkinnedAsset.cpp',[(520,547)]),
    (Path('.'),production.as_posix(),[(477,540),(607,681)]),
    (Path('.'),'src/als/state/foot_ik.rs',[(225,242),(298,380)]),
    (Path('.'),'src/als/character/control_rig_apply.rs',[(24,65)]),
    (Path('.'),'assets/ALS/Character/CR_Als.runtime.json',[(101,120)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    identity = f'source:{key}' if base==als else key
    facts.append(dict(key=identity,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
