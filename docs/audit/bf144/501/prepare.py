import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/501')
def read(path):
    raw = gzip.decompress(path.read_bytes()) if path.suffix=='.gz' else path.read_bytes()
    return json.loads(raw)
def named(items,name):
    return next(item for item in items if item['name']==name)
def components(values,name):
    return [value['numeric_value'] for value in named(values,name)['fields']]
def source_blend(frame):
    fields=named(frame['source_state_after_tick'],'GroundedState')['fields']
    blend=named(fields,'VelocityBlend')['fields']
    return dict(amounts=[named(blend,f'{name}Amount')['numeric_value'] for name in ['Forward','Backward','Left','Right']],
                init=named(blend,'bInitializationRequired')['exported_text']=='True')

actual=read(root/'runtime-observed/parameters.json')
old=read(parent/'484/runtime-observed/parameters.json.gz')
for key in ['before','after']:
    assert [{k:v for k,v in row.items() if k!='velocity_blend_state'} for row in actual[key]]==old['animation_'+key][96:104]
assert actual['physics_before']==old['rows'][17:25]
assert actual['physics_after']==old['camera_after_post_update'][17:25]
motor=old['after_motors'][20]
del old
print('Verified the real observation preserves the original first-motion frame and physics rows',flush=True)

case=read(parent/'493/virtual-motion.native.json.gz')['character_animation_probe']['cases'][0]
source,source_prior=case['frames'][98],case['frames'][97]
locomotion=named(source['source_state_after_tick'],'LocomotionState')['fields']
source_info=dict(velocity=components(locomotion,'VelocityWorldSpace'),
                 rotation=components(locomotion,'RotationQuaternionWorldSpace'),delta=source['world_delta_seconds'],
                 previous=source_blend(source_prior),actual=source_blend(source),
                 start=[source_prior['actor_transform']['translation'][axis] for axis in 'xyz'],
                 end=[source['actor_transform']['translation'][axis] for axis in 'xyz'])
assert source_info['previous']=={'amounts':[0,0,0,0],'init':False}
prior=actual['after'][2]['velocity_blend_state']
before=actual['before'][3]['velocity_blend_state']
after=actual['after'][3]['velocity_blend_state']
assert prior['amounts']==before['amounts']==[0,0,0,0]
assert prior['initialization_required']==before['initialization_required']==after['initialization_required']==False
assert before['velocity']==after['velocity']==actual['after'][3]['provider_velocity']
assert before['rotation']==after['rotation']==actual['after'][3]['history_input']['actor_world_rotation']
assert before['delta']==after['delta']==source_info['delta']
physics=actual['physics_before'][3]
final=actual['physics_after'][3]
assert physics['ordinal']==20 and physics['input_move']==[0,1]
assert physics['has_base'] and physics['support_velocity']==[0,0,0]
assert physics['grounded_move_start']==physics['position']
assert physics['planned_velocity']==motor['velocity']
assert after['velocity']==final['velocity']
assert physics['simulation_delta']==before['delta']
target_info=dict(velocity=before['velocity'],rotation=before['rotation'],delta=before['delta'],
                 previous=before['amounts'],actual=after['amounts'],plan=physics['planned_velocity'],
                 simulation_delta=physics['simulation_delta'],
                 start=physics['grounded_move_start'],end=final['position'],
                 plan_delta=physics['planned_delta'],move_input=physics['expected_direction'])
assert source['movement_before_tick']['pending_input_world']['x']==target_info['move_input'][0]
assert source['movement_before_tick']['pending_input_world']['y']==target_info['move_input'][2]
assert source['movement_before_tick']['velocity_cm_s']=={'x':0,'y':0,'z':0}
assert physics['physical_velocity']==[0,0,0]
(root/'evidence-input.json').write_text(json.dumps(dict(query=99,source=source_info,target=target_info),indent=2)+'\n')
print('Qualified actual zero-weight prestate, common motion input and plan/position boundaries',flush=True)

previous=read(parent/'499/replay-restored.json.gz')
modes=[previous['rows'][0]['native'][i]['mode'] for i in previous['best_source_models']]
(temporary/'modes.json').write_text(json.dumps(modes,separators=(',',':')))
math=(parent/'499/replay.rs').read_text()
math=math[math.index('fn a32'):math.index('fn main()')]
a=math.index('fn target_direction(')
b=math.index('fn native_lerp(',a)
direction = math[math.index('fn native_direction('):a]
direct = direction.replace('fn native_direction(','fn native_direct_direction(').replace(
    'v * (v.x.abs() + v.y.abs() + v.z.abs()).recip()', 'v / (v.x.abs() + v.y.abs() + v.z.abs())')
assert direct!=direction
(temporary/'math.rs').write_text(math[:a]+direct+math[b:])
axis=(parent/'500/replay.rs').read_text()
axis=axis[axis.index('#[derive(Clone, Copy)]'):axis.index('fn main()')]
(temporary/'basis.rs').write_text(axis)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
glam = next((Path.home()/'.cargo/registry/src').glob('*/glam-0.32.1'))
facts = []
for base,key,ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(694,705),(824,827),(850,901)]),
    (engine,'Engine/Source/Runtime/Core/Public/Math/Vector.h',[(320,330)]),
    (glam,'src/f32/vec3.rs',[(1419,1428)]),
    (engine,'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(5863,5876)]),
    (Path('.'),'src/als/character/movement/motion.rs',[(2675,2698),(3814,3822)]),
    (Path('.'),'src/als/character/movement/dynamics.rs',[(78,85)]),
    (Path('.'),'src/als/character/animation_instances/ab_als_provider.rs',[(313,322)]),
    (Path('.'),'src/als/character/animation_instances/source_native_refresh.rs',[(135,184)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=f'source:{key}' if base==als else f'dependency:glam-0.32.1/{key}' if base==glam else key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
