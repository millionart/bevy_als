import json
import struct
from pathlib import Path

root=Path(__file__).resolve().parent

def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]

def ue_axes(vector):
    return [vector[i]*100 for i in (0,2,1)]

samples=[]
for label in ('grazing-verified','source-gap'):
    row=json.loads((root/f'runtime-{label}'/'fresh-floor.json').read_bytes())
    cast,=[event for event in row['events'] if event['kind']=='capsule']
    qualified,=[event for event in row['events'] if event['kind']=='capsule_qualification']
    assert cast['hit']['entity']=='AuditWalkOffPlatform' and qualified['valid']
    origin=row['origin']
    box=row['box_center']
    relative=[value-center for value,center in zip(origin,box,strict=True)]
    world_origin=[origin[0],origin[1]-2,origin[2]]
    world_box=[box[0],box[1]-2,box[2]]
    half=f32(row['half_m']-qualified['shrink_m'])
    for kind,center,start in (
        ('world',world_box,world_origin),('relative',[0,0,0],relative)
    ):
        samples.append(dict(name=f'{label}-{kind}',rotation_xyzw=[0,0,0,1],capsule_sweep=dict(
            box_translation_cm=ue_axes(center),box_half_extents_cm=ue_axes(row['box_half_extents']),
            start_cm=ue_axes(start),direction=[0,0,-1],
            radius_cm=cast['radius_m']*100,half_height_cm=half*100,length_cm=cast['cast_distance_m']*100)))
(root/'geometry.request.json').write_text(json.dumps({'samples':samples},indent=2)+'\n')
print('Prepared shared world and box-relative inputs for both live floor casts',flush=True)
