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

old=read(parent/'498/runtime-observed/parameters.json.gz')
baseline,widened=[read(root/f'runtime-{label}/parameters.json') for label in ['baseline','widened']]
direct_ids=['41fd7093','69a33590']
def clean(row):
    nodes=[]
    for node in row['scale_nodes']:
        item={key:value for key,value in node.items() if key!='c503'}
        item['inputs']=sorted([edge for edge in item['inputs'] if not (
            node['guid'][:8] in direct_ids and edge['pin']=='DesiredAlphas_3')],key=lambda x:x['pin'])
        nodes.append(item)
    return {**row,'scale_nodes':nodes}
assert len(old)==len(baseline)==len(widened)==16
assert list(map(clean,baseline))==list(map(clean,old))
print('Verified same-binary unmodified input route preserves prior actual graph evidence',flush=True)
numeric=read(root/'replay-baseline.json')
numeric={row['query']:row for row in numeric}
def node(row,guid):
    selected=[value for value in row['scale_nodes'] if value['guid'].startswith(guid) and value['pin']=='Pose']
    assert len(selected)<=1
    return selected[0] if selected else None

def bones(a,b):
    a,b = a['bones'],b['bones']
    assert [x['name'] for x in a]==[x['name'] for x in b]
    return [x['name'] for x,y in zip(a,b,strict=True) if x!=y]

results=[]
for query,(a,b) in enumerate(zip(baseline,widened,strict=True),95):
    assert a['history_input']==b['history_input']
    assert all(a[field]==b[field] for field in ['frame_delta','position','velocity','motion_world_velocity','provider_velocity'])
    a_rig,b_rig=[row['rig_snapshot'] for row in [a,b]]
    assert a_rig['initialization_count']==b_rig['initialization_count']==1
    assert a_rig['traces']==b_rig['traces']
    assert a_rig['executed_steps']==b_rig['executed_steps']
    direct={}
    for guid in direct_ids:
        x,y=[node(row,guid) for row in [a,b]]
        assert (x is None)==(y is None)
        if x is None: continue
        assert x['multi']==y['multi']
        assert x['c503']['right']==x['multi']['desired'][3]
        if query in numeric:
            assert y['c503']['right']==numeric[query]['widened'][3]
        else:
            assert y['c503']['right']==x['c503']['right']
        direct[guid]=dict(baseline_right=x['c503']['right'],widened_right=y['c503']['right'],
            gait=[x['gait'],y['gait']],changed_bones=bones(x['c503']['pose'],y['c503']['pose']))
    poses={key:bones(a_rig['poses'][key]['pose'],b_rig['poses'][key]['pose'])
           for key in ['rig_source','rig_output','root_result']}
    contexts=[rig['upstream_context_curves'] for rig in [a_rig,b_rig]]
    changed_curves={name:[contexts[0].get(name),contexts[1].get(name)]
                    for name in sorted(set(contexts[0])|set(contexts[1]))
                    if contexts[0].get(name)!=contexts[1].get(name)}
    lean=[node(row,'7c742ec2') for row in [a,b]]
    lean_values=[{'raw':row['lean']['raw'],'effective':row['lean']['mapped']['value']} if row else None for row in lean]
    inputs=[rig['externals']['RigInput']['value']['List'] for rig in [a_rig,b_rig]]
    changes=[dict(index=i,baseline=x,widened=y) for i,(x,y) in enumerate(zip(*inputs,strict=True)) if x!=y]
    results.append(dict(query=query,direct=direct,changed_pose_bones=poses,changed_curves=changed_curves,
                        root_gait=[ctx.get('PoseGait') for ctx in contexts],lean=lean_values,changed_rig_input=changes))
(root/'consumer-comparison.json').write_text(json.dumps(results,indent=2)+'\n')
print('Qualified one overridden frame and the observed pose, curve and Lean consumers',flush=True)

definition=read(root/'runtime-baseline/rig-definition.json')
assert definition==read(root/'runtime-widened/rig-definition.json')
damper=[]
for query in range(103,111):
    for pc in [89,119]:
        call=definition['program']['instructions'][pc]['Call']
        assert call['function']=='FAlsRigVMFunction_DamperExactVector::Execute'
        output=call['arguments'][2]['register']
        initialized=call['arguments'][3]['register']
        observed=[]
        for trajectory in [baseline,widened]:
            previous=trajectory[query-96]['rig_snapshot']
            current=trajectory[query-95]['rig_snapshot']
            expected=next(x['arguments_after_call'] for x in current['calls'] if x['instruction']==pc)
            assert previous['registers'][initialized]=={'List':[{'Bool':True}]}
            assert current['registers'][output]==expected[2]
            observed.append(dict(previous=previous['registers'][output],expected=expected))
        assert observed[0]['expected'][:2]==observed[1]['expected'][:2]
        assert observed[0]['previous']!=observed[1]['previous']
        assert observed[0]['expected'][2]!=observed[1]['expected'][2]
        damper.append(dict(query=query,pc=pc,function=call['function'],delta=baseline[query-95]['frame_delta'],observed=observed))
(root/'damper-input.json').write_text(json.dumps(damper,indent=2)+'\n')
print('Qualified 16 pairs of actual previous Rig pole damper states and common inputs',flush=True)

engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
config=json.loads(Path('tools/env.local.json').read_text())
als=Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts=[]
for base,key,ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(365,374),(850,901)]),
    (Path('.'),'src/als/nodes/source_movement_nodes.rs',[(63,102),(112,136)]),
    (Path('.'),'src/animation_graph_ext/source_pose_nodes.rs',[(154,193),(956,965),(967,985)]),
    (Path('.'),'src/control_rig/control_rig_units.rs',[(435,460)]),
]:
    raw=(base/key).read_bytes()
    lines=raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=f'source:{key}' if base==als else key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
