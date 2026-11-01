import copy
import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
def load(label):
    p=r/('runtime-'+label)/'parameters.json'
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
def canonical_rows(rows):
    out=copy.deepcopy(rows);ids={v['camera_at_update']['entity']for v in out};assert len(ids)==1
    for row in out:row['camera_at_update']['entity']='camera-role'
    return out
def canonical_leaf_nodes(rows):
    out=copy.deepcopy(rows)
    for node in out:
        if node['node_guid']=='a189ffa4-46d2-a4ea-01e7-728ccfb6d729':
            assert node['clip']in[None,'ALS/Animations/Base/A_Als_Stand_Pose.anim.ron'];node['clip']='idle-stand-data-role'
    return out
def validate(data,routed):
    assert len(data['animation_after'])==len(data['animation_before'])==319
    old=json.loads(gzip.decompress(Path('docs/audit/bf144/449a/runtime-restored/parameters.json.gz').read_bytes()))
    endpoints=json.loads(Path('docs/audit/bf144/449/endpoints.json').read_text());height=endpoints['physical_unit_projection']if routed else endpoints['target_pelvis_translation'][1]
    for row in data['animation_after']:
        factor=row['unit_factor'];assert factor['applications']==1 and factor['info']['routed_to_clone']is routed and factor['info']['public_sample_only_pelvis_y_changed']
    assert canonical_rows(data['rows'])==canonical_rows(old['rows'])
    for key in ['after_motors','camera_after_post_update']:assert data[key]==old[key],key
    leaves=0
    for phase in ['animation_before','animation_after']:
        for row,prior in zip(data[phase],old[phase],strict=True):
            assert {k:v for k,v in row.items()if k not in ['unit_factor','pelvis_markers','idle_leaf','leaf_nodes']}=={k:v for k,v in prior.items()if k not in ['idle_leaf','leaf_nodes']}
            assert canonical_leaf_nodes(row['leaf_nodes'])==canonical_leaf_nodes(prior['leaf_nodes'])
            assert len(row['idle_leaf'])==len(prior['idle_leaf'])
            for actual,previous in zip(row['idle_leaf'],prior['idle_leaf'],strict=True):
                assert actual['pose_exact_public_sample']and actual['actual']==actual['sampled']
                for key in ['actual','sampled']:
                    expected=copy.deepcopy(previous[key]);bone=next(b for b in expected['bones']if b['name']=='pelvis');bone['translation'][1]=height
                    assert actual[key]==expected
                leaves+=1
    assert leaves==239
    for index in range(1,99):
        marker=next(v for v in data['animation_after'][index]['pelvis_markers']if v['boundary']=='base_provider_input');assert marker['translation'][1]==height
    root=next(v for v in data['animation_after'][1]['pelvis_markers']if v['boundary']=='root_result');assert root['translation'][1]==height
    return dict(actual_leaf_records=leaves,baseprovider_first_positive_rows=98,first_root_y=root['translation'][1],height=height)
if __name__=='__main__':
    label=sys.argv[1];routed=label!='no-route';data=load(label);counts=validate(data,routed)
    rejected=[]
    for kind in ['wrong-route','wrong-height','repeated-application','changed-input']:
        if kind=='wrong-route':old=data['animation_after'][0]['unit_factor']['info']['routed_to_clone'];data['animation_after'][0]['unit_factor']['info']['routed_to_clone']=not old
        elif kind=='wrong-height':v=next(v for v in data['animation_after'][1]['pelvis_markers']if v['boundary']=='base_provider_input');old=v['translation'][1];v['translation'][1]=0
        elif kind=='repeated-application':old=data['animation_after'][0]['unit_factor']['applications'];data['animation_after'][0]['unit_factor']['applications']=2
        else:old=data['animation_after'][0]['provider_speed'];data['animation_after'][0]['provider_speed']=999
        try:validate(data,routed)
        except AssertionError:rejected.append(kind)
        else:raise AssertionError('Invalid unit factor consumer passed: '+kind)
        finally:
            if kind=='wrong-route':data['animation_after'][0]['unit_factor']['info']['routed_to_clone']=old
            elif kind=='wrong-height':v['translation'][1]=old
            elif kind=='repeated-application':data['animation_after'][0]['unit_factor']['applications']=old
            else:data['animation_after'][0]['provider_speed']=old
    result=dict(gate='PASS_SINGLE_HEIGHT_UNIT_FACTOR_ACTUAL_CONSUMERS',routed=routed,counts=counts,public_sample_only_one_y_changed=True,other_input_and_clock_parameters_exact=True,camera_entity_refs_compared_by_unique_role=True,negative_fixtures_rejected=rejected,new_difference_pending_final_gate=True,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
