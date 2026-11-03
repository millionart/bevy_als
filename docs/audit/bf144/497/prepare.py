import gzip
import hashlib
import json
import os
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
sys.path.insert(0,'tools/import')
from audit_animation_transform_conversion import load_gltf, accessor_values

def read(path):
    raw = gzip.decompress(path.read_bytes()) if path.suffix=='.gz' else path.read_bytes()
    return json.loads(raw)

def node(row,guid,pin='Pose',required=True):
    result = [value for value in row['scale_nodes'] if value['guid'].startswith(guid) and value['pin']==pin]
    assert len(result)==1 or (not required and not result),(guid,len(result))
    return result[0] if result else None

def cleaned(row):
    nodes = []
    for node in row['scale_nodes']:
        item = {key:value for key,value in node.items() if key!='lean'}
        item['inputs'] = sorted(item['inputs'],key=lambda item:item['pin'])
        nodes.append(item)
    return {**row,'scale_nodes':nodes}

actual = read(root/'runtime-observed/parameters.json')
assert len(actual)==16
prior = read(parent/'496/runtime-observed/parameters.json.gz')[4]
assert cleaned(actual[10])==cleaned(prior)
print('Verified the added Lean observation preserves the previous actual query105 data',flush=True)

native_case = read(parent/'493/virtual-motion.native.json.gz')['character_animation_probe']['cases'][0]
native_frame = native_case['frames'][104]
standing = next(row for row in native_frame['linked_instances'] if '/AB_Als_Standing.' in row['class'])
native_player = next(row for row in standing['asset_players'] if row['node_guid'].startswith('34f57700'))
assert native_player['sync_read_buffer_tick_present'] and native_player['tick_evaluator']
assert native_player['tick_delta']==native_player['current_asset_time']==0
assert [s['index'] for s in native_player['blend_samples']]==[0,1,4]
assert all(s['time']==0 and s['pose_mode_at_observation']=='compressed' for s in native_player['blend_samples'])
previous_native = native_case['frames'][103]
previous_standing = next(row for row in previous_native['linked_instances'] if '/AB_Als_Standing.' in row['class'])
previous_player = next(row for row in previous_standing['asset_players'] if row['node_guid'].startswith('34f57700'))
pose_state = next(row for row in native_frame['source_state_after_tick'] if row['name']=='PoseState')
raw_gait = next(field['numeric_value'] for field in pose_state['fields'] if field['name']=='UnweightedGaitRunningAmount')
native_alpha = dict(raw=raw_gait,previous_tick_weight=previous_player['tick_weight'],
                    tick_weight=native_player['tick_weight'],delta=native_frame['world_delta_seconds'])
assert native_alpha==dict(raw=0.6779059171676636,previous_tick_weight=0.5,
                          tick_weight=0.6186039447784424,delta=0.03333333507180214)


names = {'center':'A_Als_Lean','forward':'A_Als_Lean_Forward','right':'A_Als_Lean_Right','reference':'A_Als_Lean_Pose'}
assets = {name:Path('assets/ALS/Animations/Grounded/Lean')/stem for name,stem in names.items()}
tracks = {}
for label,path in assets.items():
    doc,blob = load_gltf(path.with_suffix('.gltf'))
    bone = next(i for i,row in enumerate(doc['nodes']) if row.get('name')=='VB foot_l')
    channel, = [(anim,c) for anim in doc['animations'] for c in anim['channels'] if c['target']=={'node':bone,'path':'scale'}]
    anim,c = channel
    values = accessor_values(doc,blob,anim['samplers'][c['sampler']]['output'])
    assert len(values)==2 and values[0]==values[1]
    tracks[label]=values[0]
assert tracks['reference']==(1.,0.9999998807907104,1.)
assert all(tracks[label]==(1.,1.,1.) for label in ['center','forward','right'])
def native_scale(file,mode):
    probe = read(file)['sequence_pose_probe']
    assert probe['required_bones_mesh'].endswith('SKM_Als.SKM_Als')
    frame, = probe['frames']
    assert frame['time']==0 and frame['pose_evaluation_mode']==mode
    bone = next(value for value in frame['bones'] if value['name']=='VB foot_l')
    return [bone['local_transform']['scale'][axis] for axis in 'xzy']
native = {label:native_scale(root/f'{label}.native.json','compressed') for label in ['center','forward','right']}
assert all(scale==[0,0,0] for scale in native.values())
raw_reference = native_scale(parent/'496/lean.native.json.gz','raw_model')
assert raw_reference==[1,1,1]
print('Verified three native time-zero additive scales and the common raw Lean reference',flush=True)

graph = Path('assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron')
ron = graph.read_text()
mapping_line = next(line for line in ron.splitlines() if 'id: "7c742ec2-' in line)
assert not any('NodeData("7c742ec2-' in line and '"Alpha"):' in line for line in ron.splitlines())
tokens = dict(re.findall(r'(\w+): ([^,()]+)',mapping_line.split('mapping: (',1)[1].split(')',1)[0]))
mapping = {key:json.loads(value) for key,value in tokens.items()}
assert mapping['source_semantics'] and mapping['clamp_result'] and mapping['interpolate_result']
assert 'alpha_scale: 1.0, alpha_bias: 0.0' in mapping_line

timeline = []
for query,row in zip(range(95,111),actual,strict=True):
    apply = node(row,'7c742ec2',required=False)
    if apply is None:
        timeline.append(dict(query=query,active=False))
        continue
    blend = node(row,'34f57700','pose')
    base = node(row,'4c6fa15f')
    assert apply['lean']['raw'] is not None and apply['lean']['mapped'] is not None
    assert blend['lean']['teleport']
    timeline.append(dict(query=query,active=True,alpha=apply['lean'],base=base['scale'],
                         delta=blend['scale'],output=apply['scale'],position=blend['lean']['position']))
witness = actual[10]
leaf_ids = ['cf4c43fd','ac6fff66','7accd076']
lean_blend = node(witness,'34f57700','pose')
assert lean_blend['scale']==[0,1.1920928955078125e-07,0]
for prefix in leaf_ids:
    leaf, = [x for x in witness['leaf_nodes'] if x['node_guid'].startswith(prefix)]
    assert leaf['clocks'][0]['node_time']==leaf['clocks'][0]['pose_timestamp']==0
    assert node(witness,prefix,'pose')['scale']==[1,1,1]
clock, = [x for x in witness['leaf_nodes'] if x['node_guid'].startswith('34f57700')]
point_times = clock['clocks'][0]['point_times']
assert [x['id'] for x in point_times if x['requested_time'] is not None]==['sample_0','sample_1','sample_4']
assert all(x['requested_time'] in [None,'Absolute(0.0)'] for x in point_times)
(root/'evidence-input.json').write_text(json.dumps(dict(timeline=timeline,configuration=lean_blend['lean'],
    native_player=native_player,native_alpha=native_alpha,native_leaf_scale=native,tracks=tracks,raw_reference=raw_reference,
    mapping=mapping,point_times=point_times,assets={k:str(v) for k,v in assets.items()},graph=graph.as_posix()),indent=2)+'\n')
print('Qualified real target clocks, alpha snapshots and native sample selection',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
facts = []
for base,key,ranges in [
    (engine,'Engine/Source/Runtime/Engine/Private/Animation/AnimSequence.cpp',[(1938,1973)]),
    (engine,'Engine/Source/Runtime/AnimGraphRuntime/Private/AnimNodes/AnimNode_ApplyAdditive.cpp',[(37,101)]),
    (Path('.'),'src/animation_graph_ext/source_pose_nodes.rs',[(154,193),(521,549)]),
    (Path('.'),'src/animation_graph_ext/normalized_blendspace.rs',[(455,469),(513,527),(830,844)]),
    (Path('.'),graph.as_posix(),[(11,11),(88,88),(129,133),(228,236)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
