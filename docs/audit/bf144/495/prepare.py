import gzip
import hashlib
import importlib.util
import json
import os
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/495')
def read(path):
    return json.loads(path.read_bytes())

def gltf_scales(path):
    document, blob = converter.load_gltf(path)
    node = next(i for i,value in enumerate(document['nodes']) if value.get('name')=='VB foot_l')
    channels = [(animation,sample) for animation in document['animations'] for sample in animation['channels']
                if sample['target']=={'node':node,'path':'scale'}]
    assert len(channels)==1
    animation,sample = channels[0]
    values = converter.accessor_values(document,blob,animation['samplers'][sample['sampler']]['output'])
    assert all(value==values[0] for value in values)
    return dict(value=values[0],key_count=len(values))

tool = Path('tools/import/audit_animation_transform_conversion.py')
sys.path.insert(0,str(tool.parent))
spec = importlib.util.spec_from_file_location('gltf_evidence',tool)
converter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(converter)
paths = {'stand':'Base/A_Als_Stand_Pose','idle':'Base/A_Als_Idle','look':'View/A_Als_Look_Forward'}
tracks = {name:gltf_scales(Path('assets/ALS/Animations')/(path+'.gltf')) for name,path in paths.items()}
assert tracks['stand']==dict(value=(1.,0.9999998807907104,1.),key_count=2)
assert tracks['idle']==dict(value=(1.,1.,1.),key_count=81)
assert tracks['look']==dict(value=(1.,1.,1.),key_count=31)

native = {}
for name in paths:
    result = read(root/f'{name}.native.json')['sequence_pose_probe']
    assert result['required_bones_mesh'].endswith('SKM_Als.SKM_Als')
    samples = []
    for row in result['frames']:
        bone = next(b for b in row['bones'] if b['name']=='VB foot_l')
        scale = bone['local_transform']['scale']
        sample = dict(time=row['time'],mode=row['pose_evaluation_mode'],scale=[scale[k] for k in 'xzy'])
        assert sample['mode']==('raw_model' if name=='stand' else 'compressed')
        assert sample['scale']==([1,1,1] if name=='stand' else [0,0,0])
        samples.append(sample)
    native[name]=samples
assert [len(native[k]) for k in ['stand','idle','look']]==[1,3,1]
old = read(parent/'449/stand-raw.native.json')['sequence_pose_probe']
assert read(root/'stand.native.json')['sequence_pose_probe']==old
print('Verified native modes and immutable glTF scale accessors',flush=True)

with gzip.open(parent/'484/runtime-observed/parameters.json.gz','rt') as stream:
    baseline = json.load(stream)['animation_after']
observed = read(root/'runtime-observed/parameters.json')
moving = read(root/'runtime-moving/parameters.json')
def old_fields(row):
    return {key:value for key,value in row.items() if key!='scale_nodes'}
assert len(observed)==4 and len(moving)==6
for i,row in zip([0,1,2,3,105,158],moving,strict=True):
    assert old_fields(row)==baseline[i]
for a,b in zip(observed,moving[:4],strict=True):
    assert old_fields(a)==old_fields(b)
del baseline
print('Verified both real Playground observations against the original cached output',flush=True)

overlay = Path('assets/ALS/Character/AnimationInstances/Overlays/AB_Als_Default/Overlay.animgraph.ron')
graph = overlay.read_text()
line = next(line for line in graph.splitlines() if 'id: "67ab4097-' in line)
assert 'use_constant_alpha: true, constant_alpha: 0.75' in line
assert 'NodeData("67ab4097-46de-1ab4-e8e9-7eb9f79d6505", "Alpha"):' not in graph
line = next(line for line in graph.splitlines() if 'id: "95e7e1d3-' in line)
assert 'additive_reference: Some("ALS/Animations/Base/A_Als_Stand_Pose.anim.ron")' in line

def node(row,guid,pin='Pose'):
    matches = [item for item in row['scale_nodes'] if item['guid'].startswith(guid) and item['pin']==pin]
    assert len(matches)==1, (guid,len(matches))
    return matches[0]

inputs = []
motion = []
for query,row in zip([0,1,2,3,105,158],moving,strict=True):
    base,add = [next(b['scale'] for b in item['pose']['bones'] if b['name']=='VB foot_l')
                for item in row['head_apply'][0]['inputs']]
    head = next(b['scale'] for b in row['head_apply'][0]['actual']['bones'] if b['name']=='VB foot_l')
    idle,overlay_out,overlay_base = [node(row,guid) for guid in ['95e7e1d3','67ab4097','bc1e6b63']]
    look,reference,look_absolute = [node(row,guid,'pose') for guid in ['73bc3f9e','d88815d9','5ced9e76']]
    assert reference['scale']==list(tracks['stand']['value'])
    assert look_absolute['scale']==list(tracks['look']['value'])
    assert add==look['scale']
    if query in [2,3]:
        assert base==overlay_out['scale']
        assert overlay_base['scale']==[1,1,1]
    item = dict(query=query,head_base=base,head_add=add,head_output=head,
                idle_scale=idle['scale'],overlay_base=overlay_base['scale'],overlay_output=overlay_out['scale'],
                use_overlay_as_head_base=query in [2,3],overlay_alpha=0.75,head_alpha=1.0)
    inputs.append(item)
    if query>=105:
        motion.append({**item,'locomotion':node(row,'5b8f6ccb')['scale'],
                       'layering':node(row,'47121ad2')['scale'],
                       'lean_additive':node(row,'34f57700','pose')['scale'] if query==105 else None})
(root/'evidence-input.json').write_text(json.dumps(dict(gltf=tracks,native=native,rows=inputs),indent=2)+'\n')
(root/'motion-remaining.json').write_text(json.dumps(motion,indent=2)+'\n')
print('Qualified actual Idle and Head additive input boundaries',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
facts = []
for base,key,ranges in [
    (engine,'Engine/Source/Runtime/Engine/Private/Animation/AnimSequence.cpp',[(1895,1926),(1935,1973)]),
    (engine,'Engine/Plugins/Enterprise/GLTFExporter/Source/GLTFExporter/Private/Tasks/GLTFDelayedAnimationTasks.cpp',[(114,124)]),
    (Path('.'),'src/animation_graph_ext/source_sequence_player.rs',[(117,135)]),
    (Path('.'),'src/animation_graph_ext/reference_local_difference.rs',[(46,51),(80,90)]),
    (Path('.'),'src/animation_graph_ext/mesh_space_additive.rs',[(51,59),(162,181)]),
    (Path('.'),overlay.as_posix(),[(4,4),(13,13),(25,27)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
