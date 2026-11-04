import gzip
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'509/matrix.native.json.gz'
d = json.loads(gzip.decompress(previous.read_bytes()))
native = next(c for c in d['character_animation_probe']['cases'] if c['name'] == 'ground-zero')
def props(rows):
    return {row['name']: row for row in rows}

rows = []
for frame in native['frames']:
    fields = props(frame['source_state_after_tick'])
    loco = props(fields['LocomotionState']['fields'])
    lean = props(fields['LeanState']['fields'])
    vector = lambda key: [part['numeric_value'] for part in loco[key]['fields']]
    rows.append(dict(frame=frame['frame'], acceleration=vector('AccelerationWorldSpace'),
                     velocity=vector('VelocityWorldSpace'), rotation=vector('RotationQuaternionWorldSpace'),
                     max_acceleration=loco['MaxAcceleration']['numeric_value'],
                     max_braking=loco['MaxBrakingDeceleration']['numeric_value'],
                     goal=[lean[key]['numeric_value'] for key in ('ForwardAmount', 'RightAmount')]))
runtime = json.loads((root/'runtime-baseline/parameters.json').read_bytes())
target = [dict(frame=row['ordinal'], **row['acceleration'], goal=row['lean']['goal'])
          for row in runtime['after'] if row['gameplay']=='Grounded' and
          row['acceleration']['standing_write_frame']==row['animation_frame']]
(root/'acceleration-input.json').write_text(json.dumps(dict(native=rows,target=target),indent=2)+'\n')
print(f'Qualified {len(rows)} native and {len(target)} target Grounds', flush=True)
