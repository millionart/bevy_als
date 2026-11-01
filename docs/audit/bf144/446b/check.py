import copy
import gzip
import json
import math
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]


def load(label):
    p=r/(label+'.native.json')
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress((r/(label+'.native.json.gz')).read_bytes()))


def validate(data):
    cases=data['character_animation_probe']['cases']
    expected=json.loads((r/'request.json').read_text())['cases']
    assert len(cases)==len(expected)==2
    counts=[]
    for case,request in zip(cases,expected,strict=True):
        assert len(case['frames'])==len(request['frames'])
        initial=case['initialization']
        t=initial['actor_transform']['translation']
        assert [t['x'],t['y'],t['z']]==[*request['spawn_xy_cm'],request['spawn_height_cm']]
        control=initial['controller_input']['control_rotation_degrees']
        assert f32(control['yaw'])==f32(request['spawn_yaw_degrees'])
        q=initial['actor_transform']['rotation']
        assert abs(q['z'])>0 and all(math.isfinite(v)for v in q.values())
        for frame in case['frames']:
            assert frame['movement_after_tick']['movement_mode']==1
            assert frame['movement_after_tick']['walkable_floor']
        counts.append(len(case['frames']))
    assert counts==[318,2]
    return dict(actual_history_frames=318,shifted_floor_control_frames=2,nonzero_initial_positions_and_yaws_verified=True,all_actual_frames_walking=True)


if __name__=='__main__':
    label=sys.argv[1]
    data=load(label)
    result=validate(data)
    rejected=0
    for kind in range(3):
        bad=copy.deepcopy(data)
        c=bad['character_animation_probe']['cases'][0]
        if kind==0:c['frames'].pop()
        elif kind==1:c['initialization']['actor_transform']['translation']['x']=0
        else:c['initialization']['controller_input']['control_rotation_degrees']['yaw']=0
        try:validate(bad)
        except AssertionError:rejected+=1;continue
        raise AssertionError(kind)
    result['negative_output_fixtures_rejected']=rejected
    (r/(label+'-check.json')).write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
