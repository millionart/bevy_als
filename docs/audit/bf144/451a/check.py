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
    assert row['node_guid']=='d17ea3bf-4d5e-ebd5-fd47-0c867f5cc0e7'and row['state_key']=='Default'
    assert row['identity_mapping_verified']and row['raw_alpha']==row['effective_alpha']==1
    assert row['raw_alpha']==(row['resolved_property_alpha']if row['resolved_property_alpha']is not None else row['global_alpha'])
    assert row['pose_exact_public']and row['actual']==row['public']
    assert [(v['pin'],v['source_node'],v['source_pin'])for v in row['inputs']]==[('Base','316d5caa-4268-b60f-5ce6-2e9c0beb480b','Pose'),('Additive','5a7796c6-47c9-baf2-6479-579e079fbb67','Pose')]
    assert [len(v['pose']['bones'])for v in row['inputs']]==[80,79]and len(row['actual']['bones'])==80
    assert row['actual']['timestamp']==row['inputs'][0]['pose']['timestamp']
    for pose in [v['pose']for v in row['inputs']]+[row['actual'],row['public']]:
        assert len(pose['bones'])==len({b['name']for b in pose['bones']})
        for b in pose['bones']:
            for key,count in [('translation',3),('rotation',4),('scale',3)]:assert b[key]is None or(len(b[key])==count and all(math.isfinite(x)for x in b[key]))
def validate(data):
    assert len(data['animation_before'])==len(data['animation_after'])==319
    assert all(v['head_apply']==[]for v in data['animation_before'])
    for row in data['animation_after']:assert len(row['head_apply'])==1;validate_record(row['head_apply'][0])
    old=json.loads(Path('docs/audit/bf144/447b/runtime-restored/parameters.json').read_bytes())
    for key in ['rows','after_motors','camera_after_post_update']:assert data[key]==old[key],key
    for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='head_apply'}for row in data[key]]==old[key],key
    return 319
def validate_skeleton(skeleton):
    assert skeleton['root_path']==['GltfNode0']and len(skeleton['bones'])==82
    paths=[tuple(b['path'])for b in skeleton['bones']];assert len(paths)==len(set(paths))
    for b in skeleton['bones']:
        if b['path']==[]:assert b['name']is None and b['local']is None and b['character']is None;continue
        assert b['name']==b['path'][-1]
        for t in [b['local'],b['character']]:
            for key,count in [('translation',3),('rotation',4),('scale',3)]:assert len(t[key])==count and all(math.isfinite(x)for x in t[key])
if __name__=='__main__':
    label=sys.argv[1];data=load(label);assert validate(data)==319;s=json.loads((r/('runtime-'+label)/'skeleton.json').read_text());validate_skeleton(s)
    negative=[]
    for kind in ['wrong-alpha','wrong-source-pin','changed-public','missing-bone']:
        row=copy.deepcopy(data['animation_after'][1]['head_apply'][0])
        if kind=='wrong-alpha':row['effective_alpha']=0
        elif kind=='wrong-source-pin':row['inputs'][0]['source_node']='wrong-node'
        elif kind=='changed-public':row['public']['bones'][0]['translation'][0]+=1
        else:row['actual']['bones'].pop()
        try:validate_record(row)
        except AssertionError:negative.append(kind)
        else:raise AssertionError('Invalid full Head replay passed: '+kind)
    bad=copy.deepcopy(s);next(b for b in bad['bones']if b['path']==[])['local']=dict(translation=[0,0,0],rotation=[0,0,0,1],scale=[1,1,1])
    try:validate_skeleton(bad)
    except AssertionError:negative.append('fabricated-anchor-default')
    else:raise AssertionError('Fabricated namespace anchor transform passed')
    result=dict(gate='PASS_ACTUAL_FULL_HEAD_APPLY_PUBLIC_KERNEL_ALPHA_AND_SKELETON',queries_per_phase=319,full_pose_records=319,base_bones=80,additive_bones=79,output_bones=80,
        public_output_exact_all319=True,effective_alpha_all1_from_actual_resolution_and_identity_contract=True,stored_blend_state_not_misreported_as_alpha=True,
        skeleton_path_records=82,namespace_anchor_default_none_preserved=True,old_five_complete_history_arrays_exact=True,negative_fixtures_rejected=negative,new_differences=[],full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
