import copy
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent


def validate(data):
    for name in ['animation_before','animation_after']:
        rows=data[name]
        assert len(rows)==319
        assert [v['ordinal']for v in rows if v['ordinal']is not None]==list(range(240))
        for row in rows:
            h=row['history_input']
            for key in ['locomotion','gait','stance','rotation_mode']:
                tag='locomotion_mode'if key=='locomotion'else key
                expected={'Grounded':'Als.LocomotionMode.Grounded','Air':'Als.LocomotionMode.InAir','Custom':'','None':''}.get(h[key],h[key])if key=='locomotion'else h[key]
                assert h['root_tags'][tag]==expected
            assert h['actor_parented']is False
            assert h['actor_world_translation']==h['local_translation']
            for key,size in [('local_rotation',4),('local_scale',3),('actor_world_rotation',4),('avatar_world_translation',3),('avatar_world_rotation',4),('desired_world_motion',3),('effective_move',2)]:
                assert len(h[key])==size and all(math.isfinite(v)for v in h[key])
            if h['input_blocked']:assert h['effective_move']==[0,0]
    return dict(rows_per_phase=319,ready_rows=240,startup_rows=79)


if __name__=='__main__':
    label=sys.argv[1]
    if label=='self-test':
        h=dict(root_tags=dict(locomotion_mode='Als.LocomotionMode.Grounded',gait='Walking',stance='Standing',rotation_mode='ViewDirection'),locomotion='Grounded',gait='Walking',stance='Standing',rotation_mode='ViewDirection',actor_parented=False,actor_world_translation=[0,1,0],local_translation=[0,1,0],local_rotation=[0,0,0,1],local_scale=[1,1,1],actor_world_rotation=[0,0,0,1],avatar_world_translation=[0,0,0],avatar_world_rotation=[0,0,0,1],desired_world_motion=[0,0,0],effective_move=[0,0],input_blocked=False)
        rows=[dict(ordinal=None if i<79 else i-79,position=[0,1,0],history_input=copy.deepcopy(h))for i in range(319)]
        data=dict(animation_before=copy.deepcopy(rows),animation_after=copy.deepcopy(rows))
        validate(data)
        for kind in range(4):
            bad=copy.deepcopy(data)
            v=bad['animation_before'][0]['history_input']
            if kind==0:del v['root_tags']
            elif kind==1:v['root_tags']['gait']='Running'
            elif kind==2:v['local_rotation'][0]=float('nan')
            else:v['input_blocked']=True;v['effective_move']=[0,1]
            try:validate(bad)
            except(AssertionError,KeyError):continue
            raise AssertionError(kind)
        result=dict(rejected=4)
    else:result=validate(json.loads((r/f'runtime-{label}/parameters.json').read_text()))
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
