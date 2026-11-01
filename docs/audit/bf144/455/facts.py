import gzip
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent;e=Path(os.environ['ALS_UE_SOURCE_ROOT'])
p=r/'main.native.json';x=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()));assert x['asset_path']=='/ALS/ALS/Character/AB_Als.AB_Als'
g=next(g for g in x['graphs']if g['name']=='AnimGraph');nodes={n['node_guid']:n for n in g['nodes']}
expected=[('5514524f-4eea-c42c-19cd-349ea6f99e36','Source','e4f33e44-4019-9bac-8e0a-dcbc0e3ae178','Pose'),('01dbd86a-4d37-d1df-d0f4-65a458a965ab','Ragdolling Input','5514524f-4eea-c42c-19cd-349ea6f99e36','Pose'),('8191c2ea-49e5-bd21-ee33-86bd87b2f93c','Result','01dbd86a-4d37-d1df-d0f4-65a458a965ab','Pose')]
target_path=Path('assets/ALS/Character/AB_Als/AnimGraph.animgraph.ron');target=target_path.read_text()
for owner,pin,producer,output in expected:
    p=next(p for p in nodes[owner]['pins']if p['name']==pin and p['direction']=='Input');assert len(p['linked_to'])==1
    link=p['linked_to'][0];assert (link['node_guid'],link['pin_name'])==(producer,output)
    owner_pin='Source'if pin=='Result'else pin
    assert f'NodeData("{owner}", "{owner_pin}"): NodeData("{producer}", "{output}")'in target
rig=nodes[expected[0][0]];fields={p['name']:p for p in next(p for p in rig['properties']if p['name']=='Node')['fields']}
source_flags={k:fields[k]['exported_text']for k in ['Alpha','bTransferInputPose','bTransferInputCurves','bTransferPoseInGlobalSpace','bExecute']}
assert float(source_flags['Alpha'])==1 and source_flags['bTransferInputPose']==source_flags['bTransferInputCurves']==source_flags['bExecute']=='True'and source_flags['bTransferPoseInGlobalSpace']=='False'
for text in ['alpha_enabled: true','transfer_input_pose: true','transfer_input_curves: true','transfer_pose_in_global_space: false','pose_input: "Source"']:assert text in target
original=subprocess.check_output(['git','show','2cb9a50b:'+str(target_path)]);assert original==target_path.read_bytes()
units=[('src/animation_graph_ext/root_graph_boundaries.rs',Path('src/animation_graph_ext/root_graph_boundaries.rs'),[(208,245),(299,378)]),('src/control_rig/control_rig_hierarchy.rs',Path('src/control_rig/control_rig_hierarchy.rs'),[(304,348)]),('Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/AnimNode_ControlRigBase.cpp',e/'Engine/Plugins/Animation/ControlRig/Source/ControlRig/Private/AnimNode_ControlRigBase.cpp',[(130,171)])]
facts=[]
for key,p,ranges in units:
    raw=p.read_bytes();lines=raw.decode().splitlines();facts.append(dict(key=key,sha256=hashlib.sha256(raw).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
(r/'consumer-contract.json').write_text(json.dumps(dict(gate='PASS_FRESH_MAIN_HEAD_RIG_RAGDOLLING_ROOT_DIRECT_EDGES_AND_TRANSFER_FLAGS',direct_edges=expected,source_control_rig_flags=source_flags,target_main_graph_sha256=hashlib.sha256(target_path.read_bytes()).hexdigest(),target_main_graph_unchanged_since_actual_corpus=True,independent_rig_output_absent_from_existing_corpus=True,current_actual_vm_snapshot_absent=True,source_locomotion_action_not_captured=True,next_required_observation='Actual ControlRig output cache, runtime hierarchy/external and working values and actual input bindings; retain downstream Ragdolling boundary separately'),indent=2)+'\n')
print('Verified fresh native three-edge main closure, full-alpha transfer settings and unchanged target graph; recorded actual VM observation gap',flush=True)
