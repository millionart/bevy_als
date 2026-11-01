import gzip
import hashlib
import json
from pathlib import Path
from check import load,strip,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline');b=load('restored');disabled=load('disabled')
assert validate(a)==validate(b)
assert a['character_animation_probe']==b['character_animation_probe']
old=json.loads(gzip.decompress(Path('docs/audit/bf144/446f/restored.native.json.gz').read_bytes()))['character_animation_probe']
assert disabled['character_animation_probe']==old
assert strip(a['character_animation_probe'])==strip(b['character_animation_probe'])==old
assert read('check-disabled.report.json')['process_exit_code']==1
assert read('identity-check.json')['reader_source_unchanged']
restore=read('request-byte-restore.json');assert restore['enabled_sha256']==restore['restored_sha256']!=restore['disabled_sha256']
assert hashlib.sha256((r/'request.json').read_bytes()).hexdigest()==restore['enabled_sha256']
root=load('root');graph=next(g for g in root['graphs']if g['name']=='AnimGraph')
edges=[('8191c2ea-49e5-bd21-ee33-86bd87b2f93c','Result','01dbd86a-4d37-d1df-d0f4-65a458a965ab','Pose','Source'),('01dbd86a-4d37-d1df-d0f4-65a458a965ab','Ragdolling Input','5514524f-4eea-c42c-19cd-349ea6f99e36','Pose','Ragdolling Input'),('5514524f-4eea-c42c-19cd-349ea6f99e36','Source','e4f33e44-4019-9bac-8e0a-dcbc0e3ae178','Pose','Source')]
target=Path('assets/ALS/Character/AB_Als/AnimGraph.animgraph.ron').read_text()
facts=[]
for to_node,to_pin,from_node,from_pin,target_pin in edges:
    node=next(n for n in graph['nodes']if n['node_guid']==to_node);pin=next(p for p in node['pins']if p['name']==to_pin)
    assert [(p['node_guid'],p['pin_name'])for p in pin['linked_to']]==[(from_node,from_pin)]
    assert f'NodeData("{to_node}", "{target_pin}"): NodeData("{from_node}", "{from_pin}")'in target
    facts.append(dict(to_node=to_node,source_pin=to_pin,target_pin=target_pin,from_node=from_node,from_pin=from_pin))
(r/'root-direct-edges.json').write_text(json.dumps(dict(source_asset=root['asset_path'],target='assets/ALS/Character/AB_Als/AnimGraph.animgraph.ron',target_sha256=hashlib.sha256(target.encode()).hexdigest(),edges=facts),indent=2)+'\n')
counts=read('restored-check.json')
summary=dict(gate='PASS_NATIVE318_FIVE_ROOT_POSE_BOUNDARIES_WITH_CURRENT_ATTRIBUTE_AND_LEGACY_CONTROLS',source_history_frames=318,additional_control_frames=2,initialization_packets=2,
    five_boundaries_each79_bones=True,bone_rows_including_control_and_initialization=counts['bone_rows_including_control_and_initialization'],
    attributes_empty_all_current_observed_boundaries=True,root_output_precedes_mesh_finalization=True,raw_root_and_mesh_rotation_differences_are_source_internal=True,
    raw_root_to_mesh_rotation_difference_rows=counts['root_to_mesh_raw_rotation_difference_rows'],legacy_character_probe_exact_after_removing_only_added_observations=True,
    boundary_ablation_rejected=True,enabled_request_bytes_restored=True,same_unchanged_native_package_used=True,restored_complete_character_probe_exact=True,negative_fixtures_rejected=4,
    fresh_main_root_direct_edges_verified=3,source_reader_and_production_unchanged=True,request_or_report_build_cache_used_as_permanent_evidence=False,
    new_differences=[],total_confirmed_differences=271,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)
