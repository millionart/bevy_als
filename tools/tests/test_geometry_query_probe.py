import json
import math
import sys
import struct
from pathlib import Path


def check(path):
    data=json.loads(Path(path).read_text(encoding='utf-8-sig'))
    rows = data['geometry_query_probe']['samples']
    yaw_rows = [row for row in rows if 'yaw_difference' in row['input']]
    if yaw_rows:
        for row in yaw_rows:
            values = row['input']['yaw_difference']
            velocity = struct.unpack('f', struct.pack('f', values['velocity_yaw_degrees']))[0]
            degrees = values['view_yaw_radians'] * (180.0 / math.pi)
            angle = struct.unpack('f', struct.pack('f', velocity - degrees))[0]
            while angle > 180.0:
                angle -= 360.0
            while angle < -180.0:
                angle += 360.0
            assert row['view_yaw_degrees'] == degrees
            assert row['yaw_difference'] == angle
        print(f'Native yaw difference self-test: PASS ({len(yaw_rows)} samples)', flush=True)
        if len(yaw_rows) == len(rows):
            return
    samples={sample['input']['name']:sample for sample in rows if 'name' in sample['input']}
    assert len(samples)==10
    for name,expected in [('identity',(0,0,0)),('yaw90',(0,90,0))]:
        actual=samples[name]['rotator_degrees']
        assert all(abs(actual[key]-value)<1e-9 for key,value in zip(('pitch','yaw','roll'),expected))
    assert samples['rotation_22']['rotator_degrees']['roll']>0
    assert samples['rotation_85']['rotator_degrees']['roll']>0
    pole=samples['rotation_40']['rotator_degrees']
    assert pole['pitch']==-90 and pole['roll']==0
    flat=samples['flat']['sphere_sweep']
    assert flat['hit'] and abs(flat['distance_cm']-70)<1e-5
    for name in ['sphere_1_slope30','sphere_3_air','sphere_4_origin','sphere_5_overlap']:
        sample=samples[name];result=sample['sphere_sweep']
        assert result['hit']==('air' not in name)
        if result['hit']:
            assert math.isfinite(result['distance_cm'])
            assert result['distance_cm']<=sample['input']['sphere_sweep']['length_cm']
            assert all(math.isfinite(value) for key in ['normal','position_cm'] for value in result[key].values())
            assert abs(sum(value*value for value in result['normal'].values())-1)<1e-5
            assert (result['distance_cm']<=0)==('overlap' in name)
    print('Geometry query probe self-test: PASS',flush=True)


if __name__=='__main__':
    check(sys.argv[1])
