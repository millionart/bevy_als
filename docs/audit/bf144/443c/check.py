import copy
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]


def validate(data):
    rows=data['before_floor_sync'];assert len(rows)==240
    changed=0
    for row in rows:
        s=row['scale_inputs'];assert s['parented'] is False
        assert len(s['local_scale'])==3 and len(s['local_rotation'])==4
        assert s['actual_radius']==f32(s['base_radius']*min(s['actual_scale'][0],s['actual_scale'][2]))
        assert s['source_scale_radius']==f32(s['base_radius']*min(s['local_scale'][0],s['local_scale'][2]))
        assert s['actual_half']==f32(s['base_half']*s['actual_scale'][1])
        assert not s['source_scale_floor']['adjustment_blocked']
        changed+=s['actual_radius']!=s['source_scale_radius']
    return dict(rows=240,actual_radius_differs_from_stored_scale=changed)


if __name__=='__main__':
    label=sys.argv[1]
    if label=='self-test':
        s=dict(parented=False,local_scale=[1,1,1],local_rotation=[0,0,0,1],base_radius=.3,base_half=.9,actual_scale=[1,1,1],actual_radius=f32(.3),source_scale_radius=f32(.3),actual_half=f32(.9),source_scale_floor=dict(adjustment_blocked=False))
        d=dict(before_floor_sync=[dict(scale_inputs=copy.deepcopy(s))for _ in range(240)]);validate(d)
        for kind in range(4):
            bad=copy.deepcopy(d);x=bad['before_floor_sync'][0]['scale_inputs']
            if kind==0:del x['local_scale']
            elif kind==1:x['parented']=True
            elif kind==2:x['actual_radius']=1
            else:x['source_scale_floor']['adjustment_blocked']=True
            try:validate(bad)
            except(AssertionError,KeyError):continue
            raise AssertionError(kind)
        result=dict(rejected=4)
    else:result=validate(json.loads((r/f'runtime-{label}/parameters.json').read_text()))
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
