import json
import math
from pathlib import Path

root=Path(__file__).resolve().parent
runtime=root/'runtime-wall-162-aligned-air'
read=lambda name:json.loads((runtime/name).read_bytes())
trace=read('wall-trace.json')
fixture=read('fixture.json')
wall=read('barrier.json')
origin=[fixture['origin'][0],fixture['floor_y'],fixture['origin'][2]]
half_yaw=math.radians(wall['yaw_source_degrees'])*0.5

def axes(v):
    return [v[0],v[2],v[1]]

def position(v):
    return axes([(x-y)*100 for x,y in zip(v,origin,strict=True)])

def sample(name,start,movement):
    length=math.sqrt(sum(value*value for value in movement))
    return dict(name=name,rotation_xyzw=[0,0,math.sin(half_yaw),math.cos(half_yaw)],
                capsule_sweep=dict(box_translation_cm=position(wall['center']),
                    box_half_extents_cm=axes([value*50 for value in wall['size']]),start_cm=position(start),
                    direction=axes([value/length for value in movement]),
                    radius_cm=min(trace['capsule_half_extents'][0],trace['capsule_half_extents'][2])*100,
                    half_height_cm=trace['capsule_half_extents'][1]*100,length_cm=length*100))
samples=[sample('walking',trace['ground_start'],trace['ground_request'])]
samples.extend(sample(f'falling-{i}',row['origin'],row['movement']) for i,row in enumerate(trace['hits']))
assert len(samples)==2
(root/'geometry.request.json').write_text(json.dumps(dict(samples=samples),indent=2)+'\n')
print('Prepared native sweep with the same observed geometry and query operands',flush=True)
