import gzip
import hashlib
import json
import os
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
sys.path.insert(0,'tools/import')
from audit_animation_transform_conversion import load_gltf, accessor_values

def read(path):
    return json.loads(path.read_bytes())

with gzip.open(parent/'495/runtime-moving/parameters.json.gz','rt') as stream:
    prior = json.load(stream)
actual = read(root/'runtime-observed/parameters.json')
def without_weights(row):
    nodes = []
    for node in row['scale_nodes']:
        value = {key:item for key,item in node.items() if key not in ['blend_alpha','multi']}
        value['inputs'] = sorted(value['inputs'],key=lambda item:item['pin'])
        nodes.append(value)
    return {**row,'scale_nodes':nodes}
assert list(map(without_weights,actual))==list(map(without_weights,prior))
print('Verified actual weight observation is transparent to all previously recorded fields',flush=True)

asset_paths = {'lean':'Grounded/Lean/A_Als_Lean_Pose',
               'forward':'Grounded/Acceleration/A_Als_Acceleration_Forward',
               'right':'Grounded/Acceleration/A_Als_Acceleration_Right'}
tracks = {}
for name,path in asset_paths.items():
    d,b = load_gltf(Path('assets/ALS/Animations')/(path+'.gltf'))
    index = next(i for i,node in enumerate(d['nodes']) if node.get('name')=='VB foot_l')
    channel = [(anim,c) for anim in d['animations'] for c in anim['channels']
               if c['target']=={'node':index,'path':'scale'}]
    assert len(channel)==1
    anim,c = channel[0]
    values = accessor_values(d,b,anim['samplers'][c['sampler']]['output'])
    assert all(value==values[0] for value in values)
    tracks[name] = dict(scale=values[0],key_count=len(values))
assert tracks=={'lean':dict(scale=(1.,0.9999998807907104,1.),key_count=2),
                'forward':dict(scale=(1.,1.,1.),key_count=31),'right':dict(scale=(1.,1.,1.),key_count=31)}
native = {}
for name in tracks:
    probe = read(root/f'{name}.native.json')['sequence_pose_probe']
    assert probe['required_bones_mesh'].endswith('SKM_Als.SKM_Als')
    frame, = probe['frames']
    assert frame['pose_evaluation_mode']==('raw_model' if name=='lean' else 'compressed')
    bone = next(item for item in frame['bones'] if item['name']=='VB foot_l')
    scale = [bone['local_transform']['scale'][k] for k in 'xzy']
    assert scale==([1,1,1] if name=='lean' else [0,0,0])
    native[name] = dict(mode=frame['pose_evaluation_mode'],time=frame['time'],scale=scale)
assert native['forward']['time']==native['right']['time']==0.3083333373069763

def node(row,guid,pin='Pose'):
    result = [x for x in row['scale_nodes'] if x['guid'].startswith(guid) and x['pin']==pin]
    assert len(result)==1,(guid,len(result))
    return result[0]
def scale(row,guid,pin='Pose'):
    return node(row,guid,pin)['scale']
records = []
for query,row in zip([105,158],actual[4:],strict=True):
    locomotion = scale(row,'5b8f6ccb')
    assert locomotion==scale(row,'4b1303c8')==scale(row,'d1a88d82')
    if query==105:
        assert locomotion==scale(row,'640a5817')
    else:
        assert not any(item['guid'].startswith('640a5817') for item in row['scale_nodes'])
    reference = scale(row,'8ad6ab55')
    assert reference==scale(row,'ee9eacbc')==[1,0.9999998807907104,1]
    legs = node(row,'a793b64d')
    assert legs['scale']==scale(row,'cacf7512')==scale(row,'47121ad2')==scale(row,'316d5caa')
    dynamic = scale(row,'345fa28a')
    assert dynamic==scale(row,'f55462ec')==scale(row,'c1a7cde0')
    head_add = next(x['scale'] for x in row['head_apply'][0]['inputs'][1]['pose']['bones'] if x['name']=='VB foot_l')
    record = dict(query=query,locomotion=locomotion,ordinary_reference=reference,
                  dynamic_delta=dynamic,legs_overlay_base=scale(row,'a3a13f89'),
                  legs_overlay_applied=scale(row,'64a1f95c'),legs_alpha=legs['blend_alpha'],
                  legs_output=legs['scale'],head_delta=head_add,head_output=scale(row,'d17ea3bf'))
    if query==105:
        multi = node(row,'69a33590')
        assert multi['multi']['normalize'] and multi['multi']['scale']==1
        assert all(w is not None for w in multi['multi']['desired'])
        assert multi['multi']['desired'][1:3]==[0,0]
        children = [node(row,name) for name in ['e0d5b3bd','6f95aa16']]
        assert all(child['time']==0.3083333373069763 for child in children)
        assert scale(row,'2e848f83')==locomotion
        record['movement_before_run_start'] = dict(
            state_machine=scale(row,'4c6fa15f'),lean_delta=scale(row,'34f57700','pose'),
            lean_output=scale(row,'7c742ec2'),cached_movement=scale(row,'25575e9e'))
        assert record['movement_before_run_start']['lean_output']==record['movement_before_run_start']['cached_movement']
        record['run_start'] = dict(base=scale(row,'25575e9e'),delta=multi['scale'],
                                   child_scales=[child['scale'] for child in children],
                                   desired=multi['multi']['desired'])
    else:
        assert not any(x['guid'].startswith('2e848f83') for x in row['scale_nodes'])
    records.append(record)

run_graph = next(Path('assets').rglob('S4_Run_Start_From_Walk.animgraph.ron'))
layer_graph = Path('assets/ALS/Character/AnimationInstances/AB_Als_Layering/AnimGraph.animgraph.ron')
run = run_graph.read_text()
assert 'constant_alpha: 1.0' in next(line for line in run.splitlines() if 'id: "2e848f83' in line)
assert all('additive_reference: Some("ALS/Animations/Grounded/Lean/A_Als_Lean_Pose.anim.ron")' in
           next(line for line in run.splitlines() if f'id: "{key}' in line) for key in ['e0d5b3bd','6f95aa16'])
assert not any('NodeData("69a33590-' in line and 'DesiredAlpha' in line for line in run.splitlines())
assert 'constant_alpha: 1.0' in next(line for line in layer_graph.read_text().splitlines() if 'id: "64a1f95c' in line)

(root/'evidence-input.json').write_text(json.dumps(dict(rows=records,native=native,tracks=tracks,assets=asset_paths,
    graphs=[run_graph.as_posix(),layer_graph.as_posix()]),indent=2)+'\n')
print('Qualified actual Run Start leaves, cached input scales and Legs weights',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
facts = []
for base,key,ranges in [
    (engine,'Engine/Source/Runtime/Engine/Private/Animation/AnimSequence.cpp',[(1895,1926),(1938,1973)]),
    (engine,'Engine/Source/Runtime/Engine/Private/Animation/AnimationRuntime.cpp',[(509,525),(1109,1119)]),
    (engine,'Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h',[(1091,1109)]),
    (Path('.'),'src/animation_graph_ext/source_pose_nodes.rs',[(382,429),(717,754),(956,965)]),
    (Path('.'),'src/als/nodes/source_movement_nodes.rs',[(62,104)]),
    (Path('.'),'src/animation_graph_ext/reference_local_difference.rs',[(46,51)]),
    (Path('.'),'src/animation_graph_ext/mesh_space_additive.rs',[(51,58),(162,181)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
