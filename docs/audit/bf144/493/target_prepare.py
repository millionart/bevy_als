import gzip
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/493')
label = sys.argv[1]
data = json.loads((root/f'runtime-{label}/parameters.json').read_bytes())
before,after = [data[f'animation_{stage}'] for stage in ['before','after']]
assert data['motion_frames']==80 and data['screenshots']==0
assert len(before)==len(after)==159
virtual = label=='virtual'
markers = ['VB foot_l','VB foot_r'] if virtual else ['ik_foot_l','ik_foot_r']
root_name = 'VB foot_root' if virtual else 'ik_foot_root'
if not virtual:
    with gzip.open(parent/'484/runtime-observed/parameters.json.gz','rt') as stream:
        old = json.load(stream)
    def compact(row):
        return {**{key:row[key] for key in ['ordinal','foot_feedback','history_input','frame_delta']},
                'rig_snapshot':{key:(row['rig_snapshot'] or {}).get(key) for key in ['poses','externals','initialization_count']}}
    assert before==[compact(row) for row in old['animation_before'][:len(before)]]
    assert after==[compact(row) for row in old['animation_after'][:len(after)]]
    del old
    print('Default 159-frame prefix is exactly the prior real scene observation',flush=True)
definition = json.loads((parent/'459/inputs.json').read_bytes())['definition']
names = [bone['name'] for bone in definition['bones']]
parents = [bone['parent'] for bone in definition['bones']]
fields = ['translation','rotation','scale']
inputs = []
chain_nodes = 0
changes = dict(rig_source_to_rig_output={name:0 for name in [root_name,*markers]},
               rig_output_to_root_result={name:0 for name in [root_name,*markers]})
for query,(pre,post) in enumerate(zip(before,after,strict=True)):
    observation = pre['foot_feedback']
    assert observation['use_ik']==(not virtual)
    assert post['foot_feedback']['states']==observation['states']
    assert post['foot_feedback']['valid']==observation['valid']==(query>0)
    assert post['rig_snapshot']['initialization_count']==1
    externals = post['rig_snapshot']['externals']
    assert externals['FootRootBone']['value']['List'][1]=={'Name':root_name}
    consumed = externals['RigInput']['value']['List']
    assert consumed[1:3]==[{'Bool':not virtual},{'Bool':query>0}]
    poses = [{bone['name']:{key:bone[key] for key in fields}
              for bone in post['rig_snapshot']['poses'][stage]['pose']['bones']}
             for stage in ['rig_source','rig_output','root_result']]
    for name in [root_name,*markers]:
        changes['rig_source_to_rig_output'][name] += poses[0][name]!=poses[1][name]
        changes['rig_output_to_root_result'][name] += poses[1][name]!=poses[2][name]
    if query==0:
        continue
    prior_pose = after[query-1]['rig_snapshot']['poses']['root_result']['pose']['bones']
    locals_ = {bone['name']:{field:bone[field] for field in fields} for bone in prior_pose}
    assert set(locals_)==set(names)|{'SKM_Als'}
    rig = observation['rig']
    armature = observation['armature']['chain']
    assert len(armature)==2 and not observation['teleport_active']
    assert not any(state['base_present'] for state in observation['states'])
    for side,index in [(0,7),(1,9)]:
        assert consumed[index:index+2]==[{'Vector':rig['location'][side]},{'Quaternion':rig['rotation'][side]}]
    scene = None
    for name,target in zip([*markers,'pelvis'],[*observation['targets'],observation['pelvis']],strict=True):
        chain = target['chain']
        assert chain[0]['bone']==name and chain[-2:]==armature
        i = names.index(name)
        prefix = 0
        while i is not None:
            node = chain[prefix]
            assert node['bone']==names[i]
            assert {field:node[field] for field in fields}==locals_[names[i]]
            chain_nodes+=1
            prefix+=1
            i=parents[i]
        tail = chain[prefix:-2]
        assert all(node['bone'] is None for node in tail)
        if scene is not None:
            assert scene==tail
        scene = tail
    assert pre['history_input']['locomotion']=='Grounded'
    inputs.append(dict(query=query,prior_root=query-1,observation=observation,
                       prior=before[query-1]['foot_feedback'],scene=scene,
                       locals=[locals_[name] for name in names]))
assert len(inputs)==158
(temporary/f'target-{label}.json').write_text(json.dumps(dict(names=names,parents=parents,rows=inputs),separators=(',',':')))
(root/f'target-{label}-check.json').write_text(json.dumps(dict(gate='PASS_ACTUAL_PRIOR_POSE_AND_RIG_BINDING',
    observed_frames=159,valid_frames=158,markers=markers,foot_root=root_name,exact_chain_nodes=chain_nodes,
    rig_binding_fields=158*4,local_pose_changes=changes),indent=2)+'\n')
print(f'Qualified {chain_nodes} actual prior pose nodes and {158*4} RigInput fields for {label}',flush=True)
