import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
temporary = Path('tools/tmp/bf144/491')
label = sys.argv[1]
report = json.loads((root/f'{label}.native.json').read_text())['character_animation_probe']
request = json.loads((root/'switch.request.json').read_text())
def named(items,name):
    return next(item for item in items if item['name']==name)

def fields(value,name):
    return named(value['fields'],name)

def flag(value):
    text = fields(value,'bUseFootIkBones')['exported_text']
    assert text in ['True','False']
    return text=='True'

def rig(value):
    return dict(input_flag=flag(value['RigInput']),foot_root=fields(value['FootRootBone'],'Name')['exported_text'])

def transform(value):
    return {key:[value[key][axis] for axis in axes]
            for key,axes in [('translation','xyz'),('rotation','xyzw'),('scale','xyz')]}

def pose(value):
    return {bone['name']:transform(bone['local']) for bone in value['mesh_local_pose']}

results = []
sockets = []
assert len(report['cases'])==len(request['cases'])
for case, specification in zip(report['cases'],request['cases'],strict=True):
    assert case['name']==specification['name']
    if 'initial_use_foot_ik_bones' not in specification:
        continue
    current_flag = specification['initial_use_foot_ik_bones']
    init = case['initialization']
    initial = dict(getter_flag=flag(init['getter']),**rig(init['rig_variables']))
    previous = pose(init)
    rows = []
    for frame, request_frame in zip(case['frames'],specification['frames'],strict=True):
        current_flag = request_frame.get('use_foot_ik_bones',current_flag)
        idx = frame['frame']
        state = named(frame['source_state_after_tick'],'FeetState')
        valid = fields(state,'bValid')['exported_text']=='True'
        before,after = [flag(frame[f'getter_{stage}_tick']) for stage in ['before','after']]
        rows.append(dict(frame=idx,requested=current_flag,getter_before=before,getter_after=after,valid=valid,
                         **rig(frame['rig_variables_after_tick']),executions=frame['rig_executions']))
        parents = frame['pose_boundaries']['root_result']['pose']['bones']
        names = [row['name'] for row in parents]
        current = pose(frame)
        assert set(previous)==set(current)==set(names)
        for side, leg in enumerate(['Left','Right']):
            suffix = ['l','r'][side]
            expected = []
            item = fields(state,leg)
            for key in ['TargetLocationWorldSpace','TargetRotationWorldSpace']:
                expected.append([value['numeric_value'] for value in fields(item,key)['fields']])
            controls = {}
            for marker in [f'ik_foot_{suffix}',f'VB foot_{suffix}']:
                chain = []
                index = names.index(marker)
                while index>=0:
                    chain.append(names[index])
                    index = parents[index]['parent_index']
                chain.reverse()
                controls[marker] = dict(prior=[previous[name] for name in chain],current=[current[name] for name in chain])
            sockets.append(dict(case=case['name'],frame=idx,valid=valid,side=side,requested=current_flag,
                                expected=expected,world=transform(frame['mesh_to_world']),controls=controls))
        previous = current
    results.append(dict(case=case['name'],initial=initial,settings=case['settings'],frames=rows))
assert len(results)==2 and len(sockets)==32
(root/f'lifecycle-{label}.json').write_text(json.dumps(results,indent=2)+'\n')
(temporary/f'socket-{label}.json').write_text(json.dumps(sockets,separators=(',',':')))
print(f'Prepared {len(sockets)} native socket controls and two lifecycle histories',flush=True)
