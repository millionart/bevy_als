import copy
import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
def read(p):return json.loads(p.read_bytes()if p.exists()and p.suffix!='.gz'else gzip.decompress((p if p.suffix=='.gz'else p.with_suffix(p.suffix+'.gz')).read_bytes()))
def load(label):return read(Path('docs/audit/bf144/448a/restored.native.json.gz')if label=='old'else r/(label+'.native.json'))
def validate_case(case):
    assert len(case['frames'])in [318,2]
    names=None;actions={};counter=[]
    for index,row in enumerate([case['initialization'],*case['frames']]):
        packet=row['pose_boundaries'];rig=packet['rig_output'];action=packet['locomotion_action']
        assert len(rig['pose']['bones'])==79 and len({b['name']for b in rig['pose']['bones']})==79
        if names is None:names=[b['name']for b in rig['pose']['bones']]
        assert [b['name']for b in rig['pose']['bones']]==names
        assert rig['required_bones_asset']==packet['rig_source']['required_bones_asset']==packet['root_result']['required_bones_asset']
        assert rig['engine_frame']==packet['rig_source']['engine_frame']==packet['root_result']['engine_frame']
        assert rig['delta_seconds']==packet['rig_source']['delta_seconds']
        assert rig['attribute_count']==0 and rig['updates']>=0 and rig['evaluations']>0
        assert rig['engine_frame']==case['initial_engine_frame']+max(index-1,0)+(1 if index else 0)
        assert action['stage']=='after_pose_evaluation_capture'and isinstance(action['tag_name'],str)and isinstance(action['is_none'],bool)
        assert action['is_none']==(action['tag_name']=='None')and action['engine_frame']>=rig['engine_frame']
        actions[action['tag_name']]=actions.get(action['tag_name'],0)+1
        counter.append(dict(index=index,updates=rig['updates'],evaluations=rig['evaluations'],pose_engine_frame=rig['engine_frame'],action_capture_engine_frame=action['engine_frame']))
    controls=case['initialization']['locomotion_action_reader_controls']
    assert controls==[dict(tag_name='None',is_none=True),dict(tag_name='Als.LocomotionAction.Ragdolling',is_none=False)]
    return dict(initialization_packets=1,ordinary_frames=len(case['frames']),bone_rows=(len(case['frames'])+1)*79,action_names=actions,counters=counter)
def validate(probe):
    assert len(probe['cases'])==2
    counts=[validate_case(case)for case in probe['cases']]
    assert [x['ordinary_frames']for x in counts]==[318,2]
    return dict(initialization_packets=2,ordinary_frames=320,bone_rows=sum(x['bone_rows']for x in counts),cases=counts)

def old_fields(probe):
    result=copy.deepcopy(probe)
    for case in result['cases']:
        case['initialization'].pop('locomotion_action_reader_controls',None)
        for row in [case['initialization'],*case['frames']]:
            row['pose_boundaries'].pop('rig_output',None);row['pose_boundaries'].pop('locomotion_action',None)
    return result
if __name__=='__main__':
    label=sys.argv[1];data=load(label)['character_animation_probe'];counts=validate(data)
    old=load('old')['character_animation_probe'];assert old_fields(data)==old
    bad=[];case=data['cases'][0]
    for kind in ['missing-rig','wrong-owner-asset','wrong-engine-frame','missing-action','constant-none-control']:
        value=dict(data);c=dict(case);value['cases']=[c,*data['cases'][1:]];c['initialization']=copy.deepcopy(case['initialization'])
        if kind=='missing-rig':del c['initialization']['pose_boundaries']['rig_output']
        elif kind=='wrong-owner-asset':c['initialization']['pose_boundaries']['rig_output']['required_bones_asset']='invalid'
        elif kind=='wrong-engine-frame':c['initialization']['pose_boundaries']['rig_output']['engine_frame']=-1
        elif kind=='missing-action':del c['initialization']['pose_boundaries']['locomotion_action']
        else:c['initialization']['locomotion_action_reader_controls'][1]=dict(tag_name='None',is_none=True)
        try:validate(value)
        except (AssertionError,KeyError):bad.append(kind)
        else:raise AssertionError('Invalid source Rig output observation passed: '+kind)
    result=dict(gate='PASS_NATIVE318_INDEPENDENT_RIG_OUTPUT_ACTION_AND_COMPLETE_OLD_PROBE_COMPATIBILITY',counts=counts,complete_old_character_probe_exact_after_removing_only_new_observations=True,negative_fixtures_rejected=bad,action_is_capture_time_not_ragdoll_update_state=True,full_rig_or_root_equivalence_claimed=False,new_differences=[],screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items()if k!='counts'}),flush=True);print(json.dumps({k:v for k,v in counts.items()if k!='cases'}),flush=True)
