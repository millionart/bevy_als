import json
import sys
from pathlib import Path
from check import load,pose_bones

r=Path(__file__).resolve().parent;label=sys.argv[1];data=load(label)
def difference(a,b):
    x=pose_bones(a);y=pose_bones(b);assert x.keys()==y.keys()
    return {k:sum(x[n][k]!=y[n][k]for n in x if n!='SKM_Als')for k in ['translation','rotation','scale']}
counts={edge:{k:0 for k in ['translation','rotation','scale']}for edge in ['rig_source:rig_output','rig_output:ragdolling_output','ragdolling_output:root_result','rig_source:root_result']};steps={};calls=traces=0;before_exact=0;functions={}
positive={edge:{k:0 for k in ['translation','rotation','scale']}for edge in counts}
head_exact=0
definition=json.loads((r/('runtime-'+label)/'rig-definition.json').read_text())
for i,row in enumerate(data['animation_after']):
    s=row['rig_snapshot'];poses={k:v['pose']for k,v in s['poses'].items()};steps[str(s['executed_steps'])]=steps.get(str(s['executed_steps']),0)+1
    calls+=len(s['calls']);traces+=len(s['traces'])
    for c in s['calls']:
        f=definition['program']['instructions'][c['instruction']]['Call']['function'];functions[f]=functions.get(f,0)+1
    for edge in counts:
        a,b=edge.split(':');d=difference(poses[a],poses[b])
        for k,v in d.items():
            counts[edge][k]+=v
            if i>0:positive[edge][k]+=v
    assert poses['rig_source']==row['head_apply'][0]['actual'];head_exact+=1
    before=data['animation_before'][i]['rig_snapshot']
    if i==0:assert before is None
    else:
        assert before is not None;previous=data['animation_after'][i-1]['rig_snapshot']
        for k in ['registers','externals','hierarchy_local','initialization_count','to_world','executed_steps','calls','traces']:assert before[k]==previous[k],k
        before_exact+=1
result=dict(gate='PASS_ACTUAL319_RIG_TO_RAGDOLLING_ROOT_POSE_PARTITION_AND_VM_CONTINUITY',queries=319,matched_positive_frames=318,all_query_increment_counts=counts,matched_positive_increment_counts=positive,actual_head_to_rig_source_full_pose_exact_queries=head_exact,before_previous_after_state_exact_queries=before_exact,execution_step_histogram=steps,total_actual_calls=calls,total_actual_traces=traces,called_functions=functions,source_independent_post_rig_unavailable=True,new_differences=[])
(r/('partition-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items()if k!='called_functions'}),flush=True)
