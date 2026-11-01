import copy
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent


def validate(data):
    before=data['animation_before']
    after=data['animation_after']
    assert len(before)==len(after)==319
    for index,(a,b)in enumerate(zip(before,after,strict=True)):
        x=a['curve_frame_input'];y=b['curve_frame_input']
        assert x['app_frame']==y['app_frame']
        if index>0:assert x['app_frame']==before[index-1]['curve_frame_input']['app_frame']+1
        assert y['projected_context_present']
        for key in ['global_standing','global_grounded']:
            assert math.isfinite(x[key])and x[key]==y[key]
        for key in ['projected_standing','projected_grounded']:
            assert math.isfinite(y[key])
            if index>0:assert x[key]==after[index-1]['curve_frame_input'][key]
    return dict(rows_per_phase=319,consecutive_application_frames=319)


if __name__=='__main__':
    label=sys.argv[1]
    if label=='self-test':
        a=[];b=[]
        for i in range(319):
            x=dict(app_frame=i,projected_context_present=True,global_standing=1,global_grounded=1,projected_standing=1,projected_grounded=1)
            a.append(dict(curve_frame_input=copy.deepcopy(x)));b.append(dict(curve_frame_input=copy.deepcopy(x)))
        data=dict(animation_before=a,animation_after=b);validate(data)
        for kind in range(4):
            bad=copy.deepcopy(data)
            if kind==0:del bad['animation_before'][0]['curve_frame_input']
            elif kind==1:bad['animation_after'][0]['curve_frame_input']['app_frame']=7
            elif kind==2:bad['animation_before'][0]['curve_frame_input']['global_standing']=float('nan')
            else:bad['animation_before'][1]['curve_frame_input']['projected_standing']=0
            try:validate(bad)
            except(AssertionError,KeyError):continue
            raise AssertionError(kind)
        result=dict(rejected=4)
    else:result=validate(json.loads((r/f'runtime-{label}/parameters.json').read_text()))
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
