import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/502')
temporary.mkdir(parents=True,exist_ok=True)
with gzip.open(parent/'499/replay-restored.json.gz','rt') as stream:
    previous = json.load(stream)
assert previous['minimum_source_field_mismatches']==0 and len(previous['best_source_models'])==96
modes = [previous['rows'][0]['native'][i]['mode'] for i in previous['best_source_models']]
(temporary/'modes.json').write_text(json.dumps(modes,separators=(',',':')))
prior=(parent/'499/replay.rs').read_text()
math=prior[prior.index('fn a32'):prior.index('fn main()')]
a=math.index('fn target_direction(')
b=math.index('fn native_lerp(',a)
math=math[:a]+math[b:]
a=math.index('fn native_local(')
b=math.index('fn native_direction(',a)
wide=math[a:b].replace('fn native_local(','fn native_local_f64(').replace('-> Vec3','-> DVec3').replace('Vec3::new(','DVec3::new(').replace(' as f32','')
assert 'DVec3::new(' in wide and ' as f32' not in wide
(temporary/'math.rs').write_text(math+wide)
axis=(parent/'500/replay.rs').read_text()
axis=axis[axis.index('fn project_axes('):axis.index('fn input(')]
(temporary/'basis.rs').write_text(axis)
print('Prepared shared native, runtime basis and first-motion model references',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config=json.loads(Path('tools/env.local.json').read_text())
als=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
glam=next((Path.home()/'.cargo/registry/src').glob('*/glam-0.32.1'))
facts=[]
for base,key,ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(824,827),(856,866)]),
    (engine,'Engine/Source/Runtime/Core/Public/Math/Quat.h',[(1252,1258)]),
    (engine,'Engine/Source/Runtime/Core/Public/Math/Vector.h',[(324,329),(1764,1774)]),
    (glam,'src/f32/neon/quat.rs',[(534,538),(773,783),(872,884)]),
    (Path('.'),'src/als/utility/animation.rs',[(1,12)]),
    (Path('.'),'src/als/character/animation_instances/source_native_refresh.rs',[(141,146)]),
    (Path('.'),'tools/import/coordinate_basis.py',[(5,29),(40,46)]),
]:
    raw=(base/key).read_bytes()
    lines=raw.decode('utf-8-sig').splitlines()
    kind=f'source:{key}' if base==als else f'dependency:glam-0.32.1/{key}' if base==glam else key
    facts.append(dict(key=kind,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
