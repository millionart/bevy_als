import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def read(path):
    raw = gzip.decompress(path.read_bytes()) if path.suffix=='.gz' else path.read_bytes()
    return json.loads(raw)

def project(row,query):
    rig=row['rig_snapshot']
    h=row['history_input']
    return dict(query=query,ordinal=row['ordinal'],delta=row['frame_delta'],
        locomotion=h['locomotion'],gait=h['gait'],effective_move=h['effective_move'],
        position=row['position'],velocity=row['velocity'],provider_speed=row['provider_speed'],
        rig_input=rig['externals']['RigInput'],to_world=rig['to_world'],init=rig['initialization_count'],
        rig_source=rig['poses']['rig_source']['pose'],root=rig['poses']['root_result']['pose'],
        root_gait=rig['upstream_context_curves'].get('PoseGait'),traces=rig['traces'],
        pole_states=[rig['registers'][281],rig['registers'][331]],
        pole_init=[rig['registers'][282],rig['registers'][332]],
        pole_calls=[c for c in rig['calls'] if c['instruction'] in [89,119]],
        audited_inputs=[dict(guid=n['guid'],right=n['c503']['right']) for n in row['scale_nodes']
                        if n['pin']=='Pose' and n['c503'] is not None])

trajectories=[]
for label in ['baseline','widened']:
    frames=read(root/f'runtime-{label}/parameters.json')
    previous=read(parent/f'503/runtime-{label}/parameters.json.gz')
    assert frames[:16]==[project(row,q) for q,row in enumerate(previous,95)]
    assert [row['query'] for row in frames]==list(range(95,199))
    trajectories.append(frames)
print('Verified exact overlap with both earlier real trajectories',flush=True)

baseline,widened=trajectories
differing={key:[] for key in ['rig_input','rig_source','root','root_gait','pole_states','pole_calls']}
changed_bones=[]
for a,b in zip(baseline,widened,strict=True):
    assert all(a[key]==b[key] for key in ['position','velocity','provider_speed','locomotion','gait','effective_move','to_world','delta','traces'])
    assert a['init']==b['init']==1
    assert a['pole_init']==b['pole_init']==[{'List':[{'Bool':True}]}]*2
    assert len(a['pole_calls'])==len(b['pole_calls'])==2
    for key in differing:
        if a[key]!=b[key]:
            differing[key].append(a['query'])
    assert [x['name'] for x in a['root']['bones']]==[x['name'] for x in b['root']['bones']]
    changed_bones.append(dict(query=a['query'],bones=[x['name'] for x,y in zip(a['root']['bones'],b['root']['bones'],strict=True) if x!=y]))
motion_end=next(row['query'] for row in baseline if row['query']>101 and row['effective_move']==[0,0])
stop=next(row['query'] for row in baseline if row['query']>=motion_end and row['velocity']==[0,0,0])
assert motion_end==139 and stop==146
assert all(row['velocity']==[0,0,0] and row['effective_move']==[0,0] for row in baseline[stop-95:])
assert differing['rig_input']==[102] and differing['rig_source']==[101,102] and differing['root_gait']==[102]
assert differing['root']==list(range(101,149)) and differing['pole_states']==differing['pole_calls']==list(range(101,150))
(root/'comparison.json').write_text(json.dumps(dict(input_released=motion_end,stationary=stop,
    first_persistent_root_equality=149,first_persistent_pole_equality=150,last_observed=198,
    differing=differing,changed_bones=changed_bones),indent=2)+'\n')
print('Qualified first stationary frame and stable equality through the end of the observed tail',flush=True)

definition=read(root/'runtime-baseline/rig-definition.json')
assert definition==read(root/'runtime-widened/rig-definition.json')
initial=[frames[102-95]['pole_states'] for frames in trajectories]
calls=[]
for query in range(103,199):
    index=query-95
    for side,pc in enumerate([89,119]):
        expected=[next(x['arguments_after_call'] for x in frames[index]['pole_calls'] if x['instruction']==pc)
                  for frames in trajectories]
        assert expected[0][:2]==expected[1][:2]
        assert [x[2] for x in expected]==[frames[index]['pole_states'][side] for frames in trajectories]
        calls.append(dict(query=query,side=side,pc=pc,delta=baseline[index]['delta'],expected=expected,
                          previous=[frames[index-1]['pole_states'][side] for frames in trajectories]))
(root/'damper-input.json').write_text(json.dumps(dict(initial=initial,calls=calls),indent=2)+'\n')
print('Qualified 192 actual paired damper calls and their prior states',flush=True)

engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
config=json.loads(Path('tools/env.local.json').read_text())
als=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts=[]
for base,key,ranges in [
    (als,'Private/Nodes/AlsRigUnits.cpp',[(13,30)]),
    (als,'Public/Utility/AlsMath.h',[(84,95)]),
    (engine,'Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/AnimNode_ControlRig.cpp',[(142,155)]),
    (Path('.'),'src/control_rig/control_rig_units.rs',[(441,455)]),
    (Path('.'),'src/animation_graph_ext/root_graph_boundaries.rs',[(217,235),(295,301)]),
]:
    raw=(base/key).read_bytes()
    lines=raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=f'source:{key}' if base==als else key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
