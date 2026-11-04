import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def read(path):
    return json.loads(gzip.decompress(path.read_bytes()))

def named(items,name):
    return next(item for item in items if item['name']==name)

def numeric(values,name):
    return named(values,name)['numeric_value']

def components(values,name):
    return [value['numeric_value'] for value in named(values,name)['fields']]

def native_blend(frame):
    fields = named(frame['source_state_after_tick'],'GroundedState')['fields']
    value = named(fields,'VelocityBlend')['fields']
    return dict(amounts=[numeric(value,f'{name}Amount') for name in ['Forward','Backward','Left','Right']],
                init=named(value,'bInitializationRequired')['exported_text']=='True')

source = read(parent/'493/virtual-motion.native.json.gz')['character_animation_probe']['cases'][0]['frames']
target = read(parent/'498/runtime-observed/parameters.json.gz')
def target_blend(frame):
    node, = [node for node in frame['scale_nodes'] if node['guid'].startswith('41fd7093') and node['pin']=='Pose']
    return node['multi']['desired']

rows = []
for query in range(100,111):
    source_frame = source[query-1]
    before = source[query-2]
    locomotion = named(source_frame['source_state_after_tick'],'LocomotionState')['fields']
    native = dict(velocity=components(locomotion,'VelocityWorldSpace'),
                  rotation=components(locomotion,'RotationQuaternionWorldSpace'),
                  delta=source_frame['world_delta_seconds'],previous=native_blend(before),
                  actual=native_blend(source_frame))
    current = target[query-95]
    target_input = dict(velocity=current['provider_velocity'],rotation=current['history_input']['actor_world_rotation'],
                        delta=current['frame_delta'],previous=target_blend(target[query-96]),actual=target_blend(current))
    assert native['rotation'][:2]==[0,0] and native['velocity'][2]==0
    assert target_input['rotation'][0]==target_input['rotation'][2]==target_input['velocity'][1]==0
    assert not native['previous']['init'] and not native['actual']['init']
    rows.append(dict(query=query,source=native,target=target_input))
assert rows[4]['source']['actual']['amounts']==[0.4904688596725464,0,0,0.259621798992157]
assert rows[4]['target']['actual']==[0.49046874046325684,0,0,0.2596219480037689]
(root/'evidence-input.json').write_text(json.dumps(rows,indent=2)+'\n')
print('Qualified 11 observed native and target VelocityBlend input and previous-state rows',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
bevy_math = next((Path.home()/'.cargo/registry/src').glob('*/bevy_math-0.19.1'))
glam = next((Path.home()/'.cargo/registry/src').glob('*/glam-0.32.1'))
facts = []
for base,key,ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(824,827),(850,901)]),
    (als,'Public/Utility/AlsMath.h',[(84,95)]),
    (als,'Public/Settings/AlsGroundedSettings.h',[(21,27)]),
    (engine,'Engine/Source/Runtime/Core/Public/Math/Quat.h',[(1252,1258)]),
    (engine,'Engine/Source/Runtime/Core/Public/Math/Vector.h',[(320,332),(1764,1775)]),
    (engine,'Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h',[(1118,1129),(1553,1560)]),
    (engine,'Engine/Source/Runtime/Core/Public/GenericPlatform/GenericPlatformMath.h',[(558,563)]),
    (Path('.'),'src/als/character/animation_instances/ab_als_provider.rs',[(250,255),(313,322)]),
    (Path('.'),'src/als/character/animation_instances/source_native_refresh.rs',[(135,184)]),
    (Path('.'),'src/als/utility/animation.rs',[(1,12),(28,33)]),
    (bevy_math,'src/common_traits.rs',[(49,57)]),
    (bevy_math,'src/lib.rs',[(78,86)]),
    (glam,'src/f32/float.rs',[(1,11)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    name = f'source:{key}' if base==als else f'dependency:bevy_math-0.19.1/{key}' if base==bevy_math else key
    if base==glam:
        name = f'dependency:glam-0.32.1/{key}'
    facts.append(dict(key=name,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
