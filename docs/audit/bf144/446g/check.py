import copy
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent


def validate(data):
    maps=[]
    for key in ['physics_motors','physics_coefficients','physics_raw']:
        rows=data[key]
        by_frame={v['app_frame']:v for v in rows}
        assert len(rows)==len(by_frame)
        maps.append(by_frame)
    motor,coeff,raw=maps
    count=0
    for row in data['animation_before']:
        frame=row['curve_frame_input']['app_frame']
        m=motor[frame];c=coeff[frame];p=raw[frame]
        assert c['substeps']>0 and c['frame_delta_ns']>0
        for v in [m['position'],m['velocity'],m['motor_acceleration'],p['position'],p['velocity']]:
            assert len(v)==3 and all(math.isfinite(x)for x in v)
        if c['integration']is not None:
            assert all(math.isfinite(x)for x in c['integration']['linear_increment'])
        assert isinstance(m['disabled'],bool)and isinstance(c['sleeping'],bool)
        count+=1
    assert count==319
    return dict(complete_animation_queries=319,motor_rows=len(motor),coefficient_rows=len(coeff),raw_physics_rows=len(raw))


if __name__=='__main__':
    label=sys.argv[1]
    if label=='self-test':
        frames=list(range(319))
        d=dict(animation_before=[dict(curve_frame_input=dict(app_frame=i))for i in frames],
            physics_motors=[dict(app_frame=i,position=[0,1,0],velocity=[0,0,0],motor_acceleration=[0,9.81,0],disabled=False)for i in frames],
            physics_coefficients=[dict(app_frame=i,substeps=6,frame_delta_ns=33333335,integration=dict(linear_increment=[0,0,0]),sleeping=False)for i in frames],
            physics_raw=[dict(app_frame=i,position=[0,1,0],velocity=[0,0,0])for i in frames])
        validate(d)
        for kind in range(4):
            bad=copy.deepcopy(d)
            if kind==0:bad['physics_motors'].pop()
            elif kind==1:bad['physics_coefficients'][0]['integration']['linear_increment'][0]=float('nan')
            elif kind==2:bad['physics_raw'][1]['app_frame']=0
            else:bad['physics_coefficients'][0]['substeps']=0
            try:validate(bad)
            except(AssertionError,KeyError):continue
            raise AssertionError(kind)
        result=dict(rejected=4)
    else:result=validate(json.loads((r/f'runtime-{label}/parameters.json').read_text()))
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
