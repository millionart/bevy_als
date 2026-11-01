import copy
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent


def validate(data):
    frames=[v for v in data['animation_after']if v['ordinal']is not None]
    assert [v['ordinal']for v in frames]==list(range(240))
    active=temporary=clip_keys=0
    touched=set()
    for frame in frames:
        assert frame['leaf_nodes']
        for node in frame['leaf_nodes']:
            assert isinstance(node['clocks'],list)
            if node['kind']=='clip':assert node['clip']
            for clock in node['clocks']:
                active+=1
                temporary+=clock['temporary']
                clip_keys+=node['kind']=='clip'
                touched.add(node['node_guid'])
                assert math.isfinite(clock['node_time'])
                if node['kind']=='clip':assert clock['pose_timestamp']==clock['node_time']
                if node['kind']=='blendspace':
                    assert math.isfinite(clock['sample_time'])
                    assert len(clock['point_times'])==len(node['points'])
                assert isinstance(clock['temporary'],bool)
                if clock['relevant']is not None:
                    for key in ['position_seconds','previous_seconds','duration_seconds','delta_seconds']:
                        assert math.isfinite(clock['relevant'][key])
                for tick in clock['sync']:
                    assert math.isfinite(tick['phase'])and tick['phase_duration']>0
    assert active>240 and clip_keys>0
    return dict(ready_frames=240,completed_leaf_keys=active,temporary_leaf_keys=temporary,completed_builtin_clip_keys=clip_keys,touched_leaf_guids=len(touched))


if __name__=='__main__':
    label=sys.argv[1]
    if label=='self-test':
        node=dict(node_guid='fixture',kind='clip',clip='fixture.anim.ron',clocks=[dict(temporary=False,node_time=0,pose_timestamp=0,relevant=None,sync=[])])
        data=dict(animation_after=[dict(ordinal=i,leaf_nodes=[copy.deepcopy(node),copy.deepcopy(node)])for i in range(240)])
        validate(data)
        for kind in range(3):
            bad=copy.deepcopy(data)
            if kind==0:bad['animation_after'][0]['leaf_nodes']=[]
            elif kind==1:bad['animation_after'][0]['leaf_nodes'][0]['clocks'][0]['node_time']=float('nan')
            else:del bad['animation_after'][0]['leaf_nodes'][0]['clocks'][0]['temporary']
            try:validate(bad)
            except(AssertionError,KeyError):continue
            raise AssertionError(kind)
        result=dict(rejected=3)
    else:
        result=validate(json.loads((r/f'runtime-{label}/parameters.json').read_text()))
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
