import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
specs = {
    'Engine/Source/Runtime/Engine/Private/Components/SkeletalMeshComponent.cpp':[(902,933),(2560,2581)],
    'Engine/Source/Runtime/Engine/Public/BonePose.h':[(312,324)],
    'Engine/Source/Runtime/Core/Public/Math/TransformVectorized.h':[(1134,1138)],
    'Engine/Source/Runtime/Core/Public/Math/UnrealMathVectorCommon.h.inl':[(375,385),(433,442),(605,608)],
    'Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/AnimNode_ControlRigBase.cpp':[(153,170),(188,201),(304,329)],
    'Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/Tools/ControlRigHierarchyMappings.cpp':[(326,400)],
    'src/animation_graph_ext/root_graph_boundaries.rs':[(308,374)],
    'src/animation_graph_ext/post_physics_animation.rs':[(60,86),(213,228)],
    'src/animation_graph_ext/source_pose_nodes.rs':[(956,991),(1010,1032)],
    'src/als/character/animation_instances/animation_context_projection.rs':[(21,43),(68,103)],
    'src/als/character/animation_instances/ab_als_provider.rs':[(139,161)],
}
facts = []
for key, ranges in specs.items():
    path = engine/key if key.startswith('Engine/') else Path(key)
    raw = path.read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
native = json.loads(gzip.decompress(Path('docs/audit/bf144/455/main.native.json.gz').read_bytes()))
graph = next(graph for graph in native['graphs'] if graph['name']=='AnimGraph')
node = next(node for node in graph['nodes'] if node['node_guid']=='5514524f-4eea-c42c-19cd-349ea6f99e36')
props = {field['name']:field['exported_text'] for field in next(p for p in node['properties'] if p['name']=='Node')['fields']}
selected = {key:props[key] for key in ['OutputSettings','OutputBonesToTransfer','Alpha','bTransferPoseInGlobalSpace']}
assert selected == {'OutputSettings':'(bUpdatePose=True,bUpdateCurves=True)','OutputBonesToTransfer':'',
                    'Alpha':'1.000000','bTransferPoseInGlobalSpace':'False'}
definition = json.loads(Path('docs/audit/bf144/459/inputs.json').read_bytes())['definition']
functions = sorted({row['Call']['function'] for row in definition['program']['instructions'] if isinstance(row,dict) and 'Call' in row})
assert not any('SetCurve' in function for function in functions)
(root/'node-contract.json').write_text(json.dumps(dict(node=node['node_guid'],settings=selected,rig_functions=functions),indent=2)+'\n')
print('Captured source transfer, final normalization, actual target writer registration and prior curve route',flush=True)
