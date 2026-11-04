import json
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
run=root/'runtime-depth-47403'
def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]

step=json.loads((run/'walking-step.json').read_bytes())
deck=json.loads((run/'deck.json').read_bytes())
events=step['first_floor']['variants']['none']['events']
cast,=[row for row in events if row['kind']=='capsule' and row['radius_m']<.16]
qualified,=[row for row in events if row['kind']=='capsule_qualification' and row['hit']['entity']=='AuditLandingDeck']
assert cast['hit']['entity']=='AuditLandingDeck'
relative=[x-y for x,y in zip(cast['origin'],deck['center'],strict=True)]
half=f32(f32(.9)-qualified['shrink_m'])
sample=dict(name='narrow-flat-deck',rotation_xyzw=[0,0,0,1],capsule_sweep=dict(
    box_translation_cm=[0,0,0],
    box_half_extents_cm=[deck['size'][axis]*50 for axis in (0,2,1)],
    start_cm=[relative[axis]*100 for axis in (0,2,1)],
    direction=[0,0,-1],radius_cm=cast['radius_m']*100,half_height_cm=half*100,
    length_cm=cast['cast_distance_m']*100))
(root/'geometry.request.json').write_text(json.dumps({'samples':[sample]},indent=2)+'\n')
print('Prepared the exact target narrow capsule input for the native flat deck sweep',flush=True)
