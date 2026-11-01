import copy
import gzip
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent


def load(label):
    p=r/(label+'.native.json')
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress((r/(label+'.native.json.gz')).read_bytes()))


def validate(data):
    snapshots=nonempty=zeros=0
    for case in data['character_animation_probe']['cases']:
        phases=[case['initialization']['instances']]+[f[k]for f in case['frames']for k in ['instances_before_tick','linked_instances']]
        for instances in phases:
            for instance in instances:
                snapshots+=1
                curves=instance['proxy_attribute_curves']
                assert isinstance(curves,dict)and all(isinstance(v,(float,int))and math.isfinite(v)for v in curves.values())
                observed=instance['proxy_observed_engine_frame']
                updated=instance['proxy_native_update_engine_frame']
                assert isinstance(observed,int)and isinstance(updated,int)and 0<=updated<=observed
                nonempty+=bool(curves)
                zeros+=sum(value==0 for value in curves.values())
        for frame in case['frames']:
            main=next(v for v in frame['linked_instances']if v['main'])
            assert main['proxy_native_update_engine_frame']+1==main['proxy_observed_engine_frame']
    assert snapshots>0 and nonempty>0 and zeros>0
    return dict(snapshot_instances=snapshots,nonempty_maps=nonempty,explicit_zero_values=zeros)


if __name__=='__main__':
    label=sys.argv[1]
    data=load(label)
    result=validate(data)
    for kind in range(4):
        bad=copy.deepcopy(data)
        first=bad['character_animation_probe']['cases'][0]
        instance=next(v for v in first['frames'][0]['linked_instances']if v['main'])
        if kind==0:del instance['proxy_attribute_curves']
        elif kind==1:instance['proxy_attribute_curves']['bad']=float('nan')
        elif kind==2:instance['proxy_native_update_engine_frame']=0
        else:instance['proxy_native_update_engine_frame']=instance['proxy_observed_engine_frame']+1
        try:validate(bad)
        except(AssertionError,KeyError):continue
        raise AssertionError(kind)
    result['negative_fixtures_rejected']=4
    (r/(label+'-check.json')).write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
