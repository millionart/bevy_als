import copy
import gzip
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('f',struct.pack('f',v))[0]
def load(label):
    p=r/('runtime-'+label)/'parameters.json'
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
def camera_roles(rows):
    out=copy.deepcopy(rows);assert len({v['camera_at_update']['entity']for v in out})==1
    for v in out:v['camera_at_update']['entity']='camera-role'
    return out
def leaf_roles(nodes):
    out=copy.deepcopy(nodes)
    for n in out:
        if n['node_guid']=='5ced9e76-5148-58a8-a591-ffda3ad6d593':
            assert n['clip']in[None,'ALS/Animations/View/A_Als_Look_Forward.anim.ron'];n['clip']='Forward-data-role'
    return out
def without_x(pose):
    out=copy.deepcopy(pose);bone=next(v for v in out['bones']if v['name']=='pelvis');bone['translation'][0]='pelvis-x-role';return out
def validate(data,routed):
    assert len(data['animation_after'])==len(data['animation_before'])==319
    old=json.loads(gzip.decompress(Path('docs/audit/bf144/450a/runtime-restored/parameters.json.gz').read_bytes()))
    target=3.921628012903966e-5 if routed else 1.7007637193685825e-30
    assert camera_roles(data['rows'])==camera_roles(old['rows'])
    for key in ['after_motors','camera_after_post_update']:assert data[key]==old[key],key
    half_records=0
    for phase in ['animation_before','animation_after']:
        for row,prior in zip(data[phase],old[phase],strict=True):
            assert {k:v for k,v in row.items()if k not in ['data_factor','pelvis_markers','forward_difference','leaf_nodes']}=={k:v for k,v in prior.items()if k not in ['forward_difference','leaf_nodes']}
            assert leaf_roles(row['leaf_nodes'])==leaf_roles(prior['leaf_nodes'])
            assert row['data_factor']['applications']==1 and row['data_factor']['info']['routed_to_clone']is routed
            assert row['data_factor']['info']['translation_key_count']==31 and row['data_factor']['info']['changed_key']==15 and row['data_factor']['info']['only_one_key_pelvis_x_changed']
            assert len(row['forward_difference'])==len(prior['forward_difference'])
            for actual,previous in zip(row['forward_difference'],prior['forward_difference'],strict=True):
                assert actual['pose_exact_public']and actual['actual']==actual['public']
                assert actual['inputs'][0]==previous['inputs'][0]
                assert without_x(actual['inputs'][1]['pose'])==without_x(previous['inputs'][1]['pose'])
                assert without_x(actual['actual'])==without_x(previous['actual'])
                if actual['inputs'][1]['pose']['timestamp']==.5:
                    half_records+=1
                    for pose in [actual['actual'],actual['inputs'][1]['pose']]:assert next(b for b in pose['bones']if b['name']=='pelvis')['translation'][0]==target
                    raw_ref=json.loads((r/'key-data.json').read_text())['raw_reference_x']
                    assert actual['raw_reference_x_only_delta']==f32(target-raw_ref)
    first=data['animation_after'][1]
    for name in ['rig_source','root_result']:
        marker=next(v for v in first['pelvis_markers']if v['boundary']==name);assert marker['translation'][0]==target
    assert next(v for v in first['pelvis_markers']if v['boundary']=='base_provider_input')['translation'][0]==0
    return dict(first_actual_delta_x=target,first_actual_rig_and_root_x=target,half_phase_records=half_records,raw_reference_only_first_delta=first['forward_difference'][0]['raw_reference_x_only_delta'])
if __name__=='__main__':
    label=sys.argv[1];routed=label!='no-route';data=load(label);counts=validate(data,routed);negative=[]
    for kind in ['wrong-route','wrong-first-x','repeated-application','changed-input']:
        if kind=='wrong-route':old=data['animation_after'][0]['data_factor']['info']['routed_to_clone'];data['animation_after'][0]['data_factor']['info']['routed_to_clone']=not old
        elif kind=='wrong-first-x':marker=next(v for v in data['animation_after'][1]['pelvis_markers']if v['boundary']=='root_result');old=marker['translation'][0];marker['translation'][0]=999
        elif kind=='repeated-application':old=data['animation_after'][0]['data_factor']['applications'];data['animation_after'][0]['data_factor']['applications']=2
        else:old=data['animation_after'][0]['provider_speed'];data['animation_after'][0]['provider_speed']=999
        try:validate(data,routed)
        except AssertionError:negative.append(kind)
        else:raise AssertionError('Invalid Head data factor consumer passed: '+kind)
        finally:
            if kind=='wrong-route':data['animation_after'][0]['data_factor']['info']['routed_to_clone']=old
            elif kind=='wrong-first-x':marker['translation'][0]=old
            elif kind=='repeated-application':data['animation_after'][0]['data_factor']['applications']=old
            else:data['animation_after'][0]['provider_speed']=old
    result=dict(gate='PASS_HEAD_SINGLE_KEY_DATA_AND_RAW_REFERENCE_FACTOR_CONSUMERS',routed=routed,counts=counts,other_motion_input_and_clock_fields_exact=True,other_pose_channels_exact=True,
        camera_entity_compared_by_unique_role=True,negative_fixtures_rejected=negative,new_differences=[],full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
