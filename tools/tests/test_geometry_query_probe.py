import json
import math
import sys
from pathlib import Path


def check(path):
    data=json.loads(Path(path).read_text(encoding='utf-8-sig'))
    samples={sample['input']['name']:sample for sample in data['geometry_query_probe']['samples']}
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
