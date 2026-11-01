import copy
import gzip
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
def load(label):
    p=r/('runtime-'+label)/'parameters.json'
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
def validate_leaf(leaf):
    assert leaf['node_guid']=='a189ffa4-46d2-a4ea-01e7-728ccfb6d729'and leaf['state_key']=='Default'
    assert leaf['clip']=='ALS/Animations/Base/A_Als_Stand_Pose.anim.ron'and leaf['root_lock']=='None'and leaf['additive_reference']is None
    assert leaf['pose_exact_public_sample']and leaf['actual']==leaf['sampled']and leaf['actual']['timestamp']==0
    bones=leaf['actual']['bones'];assert len(bones)==len({b['name']for b in bones})==79
    for bone in bones:
        for key,count in [('translation',3),('rotation',4),('scale',3)]:assert len(bone[key])==count and all(math.isfinite(v)for v in bone[key])
    pelvis=next(b for b in bones if b['name']=='pelvis');assert pelvis['translation'][1]==.9675059914588928
def validate(data):
    assert len(data['animation_before'])==len(data['animation_after'])==319
    assert all(row['idle_leaf']==[]for row in data['animation_before'])
    leaves=[leaf for row in data['animation_after']for leaf in row['idle_leaf']];assert len(leaves)==239
    for leaf in leaves:validate_leaf(leaf)
    assert len(data['animation_after'][1]['idle_leaf'])==1
    old=json.loads(Path('docs/audit/bf144/447b/runtime-restored/parameters.json').read_bytes())
    for key in ['rows','after_motors','camera_after_post_update']:assert data[key]==old[key],key
    for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='idle_leaf'}for row in data[key]]==old[key],key
    b=json.loads(gzip.decompress(Path('docs/audit/bf144/448b/runtime-restored/parameters.json.gz').read_bytes()))
    for index in range(1,99):
        actual=data['animation_after'][index]['idle_leaf'];assert len(actual)==1
        base=next(v for v in b['animation_after'][index]['pose_boundaries']['updated_boundaries']if v['boundary']=='base_provider_input')
        assert actual[0]['actual']['bones']==base['pose']['bones']
    return len(leaves)
if __name__=='__main__':
    label=sys.argv[1];data=load(label);count=validate(data)
    negative=[]
    for kind in ['wrong-clip','wrong-time','wrong-public-output','missing-bone']:
        leaf=copy.deepcopy(data['animation_after'][1]['idle_leaf'][0])
        if kind=='wrong-clip':leaf['clip']='wrong.anim.ron'
        elif kind=='wrong-time':leaf['actual']['timestamp']=1
        elif kind=='wrong-public-output':leaf['sampled']['bones'][0]['translation'][0]+=1
        else:leaf['actual']['bones'].pop()
        try:validate_leaf(leaf)
        except(AssertionError,KeyError):negative.append(kind)
        else:raise AssertionError('Invalid actual Idle leaf passed: '+kind)
    endpoints=json.loads(Path('docs/audit/bf144/449/endpoints.json').read_text())
    result=dict(gate='PASS_ACTUAL_IDLE_LEAF_AND_LOADED_PUBLIC_SAMPLER_WITH_UNIT_ENDPOINTS',queries_per_phase=319,actual_updated_leaf_records=count,actual_source_bone_rows=count*79,public_sample_exact_all_actual_leaves=True,
        first98_positive_baseprovider_bones_equal_actual_idle_leaf=True,target_pelvis_height=endpoints['target_pelvis_translation'][1],source_raw_and_compressed_pelvis_height_cm=endpoints['source_pelvis_height_cm'],physical_projection=endpoints['physical_unit_projection'],
        old_five_complete_history_arrays_exact=True,negative_fixtures_rejected=negative,new_difference_confirmed=False,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
