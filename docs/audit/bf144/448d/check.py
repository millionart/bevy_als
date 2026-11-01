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
def validate_packet(packet):
    assert len(packet['entries'])==1
    entry=packet['entries'][0];mapped=entry['mapped_components']
    assert entry['path']==['GltfNode0','SKM_Als']and entry['mapped_entity']==mapped['entity']
    assert mapped['name']=='SKM_Als'and mapped['mesh']and mapped['skinned_mesh']
    assert mapped['target_id']is None and mapped['animated_by']is None
    assert entry['actual_matching_targets']==[]
    assert mapped['translation']==[0,0,0]and mapped['rotation']==[0,0,0,1]and mapped['scale']==[1,1,1]
    assert len(mapped['computed_world'])==16 and all(math.isfinite(v)for v in mapped['computed_world'])
    assert entry['pose_translation']==[0,0,0]and entry['pose_rotation']==[0,0,0,1]
    assert len(entry['pose_scale'])==3 and all(math.isfinite(v)for v in entry['pose_scale'])
    return entry['pose_scale']!=[1,1,1]
def validate(data):
    assert len(data['animation_before'])==len(data['animation_after'])==319
    assert data['animation_before'][0]['scene_root']['entries']==[]
    candidates=sum(validate_packet(row['scene_root'])for row in data['animation_after']);assert candidates==44
    for row in data['animation_before'][1:]:validate_packet(row['scene_root'])
    old=json.loads(Path('docs/audit/bf144/447b/runtime-restored/parameters.json').read_bytes())
    for key in ['rows','after_motors','camera_after_post_update']:assert data[key]==old[key],key
    for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='scene_root'}for row in data[key]]==old[key],key
    b=json.loads(gzip.decompress(Path('docs/audit/bf144/448b/runtime-restored/parameters.json.gz').read_bytes()))
    for new,previous in zip(data['animation_after'],b['animation_after'],strict=True):
        bone=next(v for v in previous['pose_boundaries']['default_output']['bones']if v['name']=='SKM_Als')
        entry=new['scene_root']['entries'][0]
        for key in ['translation','rotation','scale']:assert entry['pose_'+key]==bone[key]
    return candidates
if __name__=='__main__':
    label=sys.argv[1];data=load(label);count=validate(data)
    rejected=[]
    for kind in ['missing-entry','fabricated-binding','changed-local-scale','fabricated-consumer']:
        packet=copy.deepcopy(data['animation_after'][100]['scene_root'])
        if kind=='missing-entry':packet['entries']=[]
        elif kind=='fabricated-binding':packet['entries'][0]['mapped_components']['target_id']=packet['entries'][0]['bone_id']
        elif kind=='changed-local-scale':packet['entries'][0]['mapped_components']['scale']=[.9769455790519714]*3
        else:packet['entries'][0]['actual_matching_targets']=[packet['entries'][0]['mapped_components']]
        try:validate_packet(packet)
        except(AssertionError,KeyError):rejected.append(kind)
        else:raise AssertionError('Invalid actual scene target packet passed: '+kind)
    result=dict(gate='PASS_ACTUAL_SCENE_NODE_UNBOUND_AND_SCALE_CANDIDATE_NOT_APPLIED',queries_per_phase=319,nonidentity_pose_scale_rows=count,actual_matching_animation_targets_all_zero=True,mapped_mesh_target_id_and_animated_by_absent=True,
        actual_local_transform_identity_all319=True,pose_entries_exact_prior_full_root=True,old_five_complete_history_arrays_exact=True,scale_shrink_candidate_rejected_for_actual_apply_chain=True,negative_fixtures_rejected=rejected,
        new_differences=[],full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
    (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
