import hashlib
import json
import os
import sys
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parent
label = sys.argv[1]
count = 158 if label=='virtual-motion' else 80
case = json.loads((root/f'{label}.native.json').read_bytes())['character_animation_probe']['cases'][0]
life = json.loads((root/f'lifecycle-{label}.json').read_bytes())[0]
assert life['initial']=={'getter_flag':False,'input_flag':False,'foot_root':'VB foot_root'}
assert len(life['frames'])==count and sum(row['valid'] for row in life['frames'])==count-1
for frame in life['frames']:
    assert not any(frame[key] for key in ['requested','getter_before','getter_after','input_flag'])
    assert frame['foot_root']=='VB foot_root'
    assert frame['executions']==[{'entry':'Forwards Solve','succeeded':True}]
sockets = json.loads((root/f'socket-{label}.json').read_bytes())
assert len(sockets)==count*2 and sum(row['valid'] for row in sockets)==(count-1)*2
wrong_marker = 0
for row in sockets:
    marker = 'VB foot_'+['l','r'][row['side']]
    phase = 'prior' if row['valid'] else 'current'
    selected = next(item for item in row['controls'] if item['bone']==marker and item['phase']==phase)
    alternate = next(item for item in row['controls'] if item['bone']!=marker and item['phase']==phase)
    assert selected['t_exact'] and selected['q_exact']
    wrong_marker += not (alternate['t_exact'] and alternate['q_exact'])
def named(items,name):
    return next(item for item in items if item['name']==name)

def components(property):
    return [v['numeric_value'] for v in property['fields']]

counts = Counter()
for frame in case['frames']:
    state = named(frame['source_state_after_tick'],'FeetState')['fields']
    getter = frame['getter_after_tick']['fields']
    rig = frame['rig_variables_after_tick']['RigInput']['fields']
    for side in ['Left','Right']:
        foot = named(state,side)['fields']
        for field in ['Location','Rotation']:
            name = f'Foot{side}{field}'
            assert components(named(foot,f'Final{field}'))==components(named(getter,name))==components(named(rig,name))
    boundary = frame['pose_boundaries']
    poses = [{row['name']:row['local_transform'] for row in boundary[key]['pose']['bones']}
             for key in ['rig_source','rig_output','root_result']]
    for name in ['VB foot_root','VB foot_l','VB foot_r']:
        counts[f'rig_source_to_rig_output/{name}']+=poses[0][name]!=poses[1][name]
        counts[f'rig_output_to_root_result/{name}']+=poses[1][name]!=poses[2][name]
(root/'source-check.json').write_text(json.dumps(dict(native_frames=count,exact_sockets=count*2,exact_final_getter_rig_fields=count*4,
    wrong_marker_rejections=wrong_marker,local_pose_changes=counts),indent=2)+'\n')
print('Qualified exact source Socket, final foot pose and native RigInput chain',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts = []
for base,key,ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(235,257),(1277,1295),(1300,1327)]),
    (engine,'Engine/Source/Runtime/Engine/Private/Animation/ReferenceSkeleton.cpp',[(510,531)]),
    (engine,'Engine/Source/Runtime/Engine/Private/Animation/AnimDataModel.cpp',[(671,695)]),
    (engine,'Engine/Source/Runtime/Engine/Private/SkinnedAsset.cpp',[(520,547)]),
    (Path('.'),'src/als/character/foot_ik_apply.rs',[(633,681)]),
    (Path('.'),'src/als/character/control_rig_apply.rs',[(272,290),(410,427)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=f'source:{key}' if base==als else key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
