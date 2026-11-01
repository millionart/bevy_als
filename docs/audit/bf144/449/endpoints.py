import hashlib
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0,'tools/import')
from audit_animation_transform_conversion import load_gltf,animation_channel_values

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('f',struct.pack('f',v))[0]
native={label:json.loads((r/(label+'.native.json')).read_bytes())['sequence_pose_probe']for label in ['stand-raw','stand-compressed-fixed']}
for label,probe in native.items():
    assert probe['required_bones_mesh']=='/ALS/ALS/Character/SKM_Als.SKM_Als'
    assert len(probe['frames'])==1 and probe['frames'][0]['time']==0 and len(probe['frames'][0]['bones'])==79
    assert probe['frames'][0]['pose_evaluation_mode']==('raw_model'if label=='stand-raw'else 'compressed')
pelvis={label:next(b for b in p['frames'][0]['bones']if b['name']=='pelvis')['local_transform']for label,p in native.items()}
source_z=pelvis['stand-compressed-fixed']['translation']['z'];assert source_z==pelvis['stand-raw']['translation']['z']==96.75060272216797
gltf=Path('assets/ALS/Animations/Base/A_Als_Stand_Pose.gltf');document,blob=load_gltf(gltf);channel=animation_channel_values(document,blob,'pelvis')['translation'];times,values=channel
buffer_path=gltf.parent/document['buffers'][0]['uri']
assert times[0]==0 and values[0][1]==0.9675059914588928
exact=f32(source_z*.01);exported=f32(f32(source_z)*f32(.01));assert exact==.9675060510635376 and exported==values[0][1]and exact!=exported
assert json.loads((r/'stand-compressed.native.json.reader.report.json').read_text())['process_exit_code']==7
result=dict(gate='PASS_CURRENT_STAND_RAW_COMPRESSED_AND_GLTF_UNIT_ENDPOINTS',correct_mesh=True,time=0,source_pelvis=pelvis,source_pelvis_height_cm=source_z,target_pelvis_translation=values[0],physical_unit_projection=exact,exporter_float_unit_projection=exported,exporter_float_scale=f32(.01),first_height_difference_is_not_raw_versus_compressed=True,
    inputs={p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in [r/'stand-raw.native.json',r/'stand-compressed-fixed.native.json',gltf,buffer_path]},failed_combined_source_stages_request_excluded=True,new_difference_confirmed=False)
(r/'endpoints.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
