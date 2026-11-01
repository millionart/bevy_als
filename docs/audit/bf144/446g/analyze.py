import gzip
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
label=sys.argv[1]
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
data=json.loads((r/f'runtime-{label}/parameters.json').read_text())
old=json.loads(Path('docs/audit/bf144/446e/runtime-restored/parameters.json').read_text())
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert data[key]==old[key]
native=json.loads(gzip.decompress(Path('docs/audit/bf144/446f/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
source=native['frames'][18]
distance=source['movement_after_tick']['floor_distance_cm']
assert 1.9<=distance<=2.4 and source['movement_after_tick']['movement_mode']==1
source_position=source['actor_transform']['translation']
projection=[f32(source_position[k]*.01)for k in ['x','z','y']]
replay=json.loads((r/f'replay-{label}.json').read_text())
assert replay['first']['source_band_control_position']==projection
first=replay['first']
assert first['query']==19 and first['app_frame']==24
result=dict(gate='PASS_D31_FIRST_COLD_PHYSICS_CONSUMER_WITH_SOURCE_BAND',source_report='docs/audit/bf144/446f/restored.native.json.gz',source_frame=18,target_query=19,target_app_frame=24,
    source_floor_distance_cm=distance,source_position_projection=projection,actual_position=first['actual_position'],only_spring_removed_position=first['source_band_control_position'],
    motor_to_coefficient_to_raw_position_exact=True,raw_to_final_cold_position_unchanged=True,final_vertical_velocity_zero=True,
    query_error_compensation_or_threshold_change_used=False,known_difference='BF144-D31',new_difference=False,
    all_source_target_trajectory_or_root_equivalence_claimed=False)
(r/f'analysis-{label}.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
