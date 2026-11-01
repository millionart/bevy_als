import gzip
import hashlib
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('f',struct.pack('f',v))[0]
labels=['forward-compressed','forward-raw-additive','forward-raw-absolute']
probes={n:json.loads((r/(n+'.native.json')).read_bytes())['sequence_pose_probe']for n in labels}
for name,p in probes.items():
    assert p['required_bones_mesh']=='/ALS/ALS/Character/SKM_Als.SKM_Als'and len(p['frames'][0]['bones'])==79 and p['frames'][0]['time']==.5
    assert p['frames'][0]['pose_evaluation_mode']==('compressed'if name=='forward-compressed'else 'raw_model')
pelvis={n:next(b for b in p['frames'][0]['bones']if b['name']=='pelvis')['local_transform']for n,p in probes.items()}
ref_path=Path('docs/audit/bf144/449/stand-raw.native.json');ref=json.loads(ref_path.read_bytes())['sequence_pose_probe']
ref_x=next(b for b in ref['frames'][0]['bones']if b['name']=='pelvis')['local_transform']['translation']['x']
compressed=pelvis['forward-compressed']['translation']['x'];assert compressed==.003921627998352051 and pelvis['forward-raw-additive']['translation']['x']==0
assert pelvis['forward-raw-absolute']['translation']['x']==ref_x==1.7007638171697125e-28
physical=f32(compressed*.01);same_float=f32(f32(compressed)*f32(.01));assert physical==same_float==3.921628012903966e-5
source=json.loads(gzip.decompress(Path('docs/audit/bf144/448a/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]['frames'][0]
head=next(v for v in source['linked_instances']if '/AB_Als_Head.'in v['class']);player=next(v for v in head['asset_players']if v['node_guid']=='09bab580-438d-a996-49ba-5893d20844f1')
assert len(player['blend_samples'])==1 and player['blend_samples'][0]['index']==0 and player['blend_samples'][0]['time']==.5 and player['blend_samples'][0]['weight']==1 and player['blend_samples'][0]['pose_mode_at_observation']=='compressed'
head_x=next(v for v in source['pose_boundaries']['head_additive']['pose']['bones']if v['name']=='pelvis')['local_transform']['translation']['x'];assert head_x==compressed
result=dict(gate='PASS_FORWARD_SAME_PHASE_ADDITIVE_DATA_MODE_ENDPOINTS',time=.5,correct_mesh=True,pelvis=pelvis,raw_reference_pelvis_x_cm=ref_x,
    compressed_delta_x_cm=compressed,raw_delta_x_cm=0,compressed_physical_x=physical,compressed_same_float_export_x=same_float,unit_factor_has_no_separate_output_signal_for_this_first_delta=True,
    first_actual_native_head_x_equals_compressed=True,actual_native_sample_weight=1,raw_absolute_equals_raw_reference_x=True,
    input_identities={p.relative_to(Path.cwd()).as_posix()if p.is_absolute()else p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest()for p in [*(r/(n+'.native.json')for n in labels),ref_path]},new_differences=[],full_root_or_als_equivalence_claimed=False)
(r/'endpoints.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
