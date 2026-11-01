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
def validate_record(row):
    assert row['node_guid']=='73bc3f9e-083f-5fb4-a4a1-590ad157f515'and row['state_key']=='Default'
    assert row['pose_exact_public']and row['actual']==row['public']
    inputs=row['inputs'];assert [v['pin']for v in inputs]==['reference_pose','target_pose']
    assert [(v['source_node'],v['source_pin'])for v in inputs]==[('d88815d9-51c1-5fe8-8df5-9fc9cf877293','pose'),('5ced9e76-5148-58a8-a591-ffda3ad6d593','pose')]
    assert inputs[0]['pose']['timestamp']==0 and row['actual']['timestamp']==inputs[1]['pose']['timestamp']
    for pose in [inputs[0]['pose'],inputs[1]['pose'],row['actual'],row['public']]:
        assert len(pose['bones'])==len({b['name']for b in pose['bones']})==79
        for bone in pose['bones']:
            for key,count in [('translation',3),('rotation',4),('scale',3)]:assert bone[key]is None or(len(bone[key])==count and all(math.isfinite(v)for v in bone[key]))
def validate(data):
    assert len(data['animation_before'])==len(data['animation_after'])==319
    assert all(v['forward_difference']==[]for v in data['animation_before'])
    for row in data['animation_after']:
        assert len(row['forward_difference'])==1;validate_record(row['forward_difference'][0])
    first=data['animation_after'][1]['forward_difference'][0]
    pelvis=lambda p:next(v for v in p['bones']if v['name']=='pelvis')
    ref=pelvis(first['inputs'][0]['pose']);absolute=pelvis(first['inputs'][1]['pose']);delta=pelvis(first['actual'])
    assert ref['translation'][0]==0 and absolute['translation'][0]==delta['translation'][0]==1.7007637193685825e-30
    old=json.loads(Path('docs/audit/bf144/447b/runtime-restored/parameters.json').read_bytes())
    for key in ['rows','after_motors','camera_after_post_update']:assert data[key]==old[key],key
    for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='forward_difference'}for row in data[key]]==old[key],key
    return dict(reference=ref['translation'],absolute=absolute['translation'],difference=delta['translation'])
if __name__=='__main__':
    label=sys.argv[1];data=load(label);first=validate(data);negative=[]
    for kind in ['wrong-pin','wrong-time','changed-public','missing-bone']:
        row=copy.deepcopy(data['animation_after'][1]['forward_difference'][0])
        if kind=='wrong-pin':row['inputs'][0]['source_node']='wrong-node'
        elif kind=='wrong-time':row['inputs'][0]['pose']['timestamp']=1
        elif kind=='changed-public':row['public']['bones'][0]['translation'][0]+=1
        else:row['actual']['bones'].pop()
        try:validate_record(row)
        except(AssertionError,KeyError):negative.append(kind)
        else:raise AssertionError('Invalid actual difference record passed: '+kind)
    result=dict(gate='PASS_ACTUAL_FORWARD_REFERENCE_TARGET_AND_PUBLIC_DIFFERENCE',queries_per_phase=319,actual_difference_records=319,actual_bone_rows=25201,all_actual_public_poses_exact=True,
        actual_first_inputs=first,source_pins_and_timestamps_verified=True,optional_channels_preserved=True,old_five_complete_history_arrays_exact=True,
        negative_fixtures_rejected=negative,new_differences=[],full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
