import gzip
import hashlib
import json
import os
from pathlib import Path

r=Path(__file__).resolve().parent;e=Path(os.environ['ALS_UE_SOURCE_ROOT']);config=json.loads(Path('tools/env.local.json').read_text());plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored'
p=r/'ragdoll.native.json';x=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()));g=next(g for g in x['graphs']if g['name']=='AnimGraph');n=next(n for n in g['nodes']if n['node_guid']=='a88d5696-4292-b2ae-ca2e-ac8d13e7dd2f');fields={f['name']:f for p in n['properties']if p['name']=='Node'for f in p['fields']}
assert fields['BlendProfile']['exported_text']=='None'and fields['BlendType']['exported_text']=='Linear'
pin=next(p for p in n['pins']if p['name']=='BlendPose_0');assert [(p['node_guid'],p['pin_name'])for p in pin['linked_to']]==[('277fbd6d-4f8a-8a3e-1d35-f3bf5df88eb0','Pose')]
units=[('source:Public/Nodes/AlsAnimNode_GameplayTagsBlend.h',plugin/'Source/ALS/Public/Nodes/AlsAnimNode_GameplayTagsBlend.h',[(1,40)]),('source:Private/Nodes/AlsAnimNode_GameplayTagsBlend.cpp',plugin/'Source/ALS/Private/Nodes/AlsAnimNode_GameplayTagsBlend.cpp',[(1,18)]),('Engine/Source/Runtime/AnimGraphRuntime/Private/AnimNodes/AnimNode_BlendListBase.cpp',e/'Engine/Source/Runtime/AnimGraphRuntime/Private/AnimNodes/AnimNode_BlendListBase.cpp',[(291,321)]),('src/animation_graph_ext/source_pose_nodes.rs',Path('src/animation_graph_ext/source_pose_nodes.rs'),[(48,54),(956,965),(990,1037)]),('src/animation_graph_ext/gameplay_tag_blend.rs',Path('src/animation_graph_ext/gameplay_tag_blend.rs'),[(40,49)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
(r/'source-node-contract.json').write_text(json.dumps(dict(source_asset=x['asset_path'],node_guid=n['node_guid'],no_blend_profile=True,blend_mode='Linear',source_single_full_weight_evaluate_passthrough=True,base_source_node=pin['linked_to'][0]['node_guid'],target_same_base_source=True,actual_source_selector_not_inferred_from_capture_time_tag=True),indent=2)+'\n')
print('Captured fresh Ragdolling Base edge, no-profile single-full-weight source branch and current unconditional normalizer',flush=True)
