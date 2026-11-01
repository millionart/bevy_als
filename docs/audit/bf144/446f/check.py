import copy
import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
GUID='b8f30b3f-4d30-eab1-ce6b-6bb9ec868234'


def load(label):
    p=r/(label+'.native.json')
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress((r/(label+'.native.json.gz')).read_bytes()))


def main_instance(instances):return next(v for v in instances if v['main'])


def standing(frame):
    pose=next(v for v in frame['source_state_after_tick']if v['name']=='PoseState')
    return next(v['numeric_value']for v in pose['fields']if v['name']=='StandingAmount')


def validate(data):
    cases=data['character_animation_probe']['cases']
    assert len(cases)==2
    for case in cases:
        initial=main_instance(case['initialization']['instances'])
        before=main_instance(case['frames'][0]['instances_before_tick'])
        after=main_instance(case['frames'][0]['linked_instances'])
        assert before['proxy_observed_engine_frame']==initial['proxy_observed_engine_frame']+1
        assert before['proxy_native_update_engine_frame']==initial['proxy_native_update_engine_frame']
        assert before['proxy_observed_engine_frame']>before['proxy_native_update_engine_frame']
        assert after['proxy_native_update_engine_frame']==before['proxy_observed_engine_frame']
        assert after['proxy_observed_engine_frame']==after['proxy_native_update_engine_frame']+1
        assert standing(case['frames'][0])==1
        player=next(p for instance in case['frames'][0]['linked_instances']for p in instance['asset_players']if p['node_guid']==GUID)
        assert player['sync_read_buffer_tick_present']and player['cached_blend_weight']==1
    assert [len(c['frames'])for c in cases]==[318,2]
    return dict(source_history_frames=318,control_frames=2,initialization_to_first_world_engine_frame_delta=1,first_native_refresh_executed=True,first_default_pose_evaluator_ticked=True)


if __name__=='__main__':
    label=sys.argv[1];data=load(label);result=validate(data)
    for kind in range(3):
        bad=copy.deepcopy(data);case=bad['character_animation_probe']['cases'][0]
        if kind==0:
            before=main_instance(case['frames'][0]['instances_before_tick']);before['proxy_observed_engine_frame']=main_instance(case['initialization']['instances'])['proxy_observed_engine_frame']
        elif kind==1:main_instance(case['frames'][0]['linked_instances'])['proxy_native_update_engine_frame']=0
        else:
            p=next(p for i in case['frames'][0]['linked_instances']for p in i['asset_players']if p['node_guid']==GUID);p['sync_read_buffer_tick_present']=False
        try:validate(bad)
        except AssertionError:continue
        raise AssertionError(kind)
    result['negative_fixtures_rejected']=3
    (r/(label+'-check.json')).write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
