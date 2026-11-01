import json
import sys
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent;label=sys.argv[1];probe=load(label)['character_animation_probe'];validate(probe)
case=probe['cases'][0];counts={k:{x:0 for x in ['translation','rotation','scale']}for k in ['rig_source:rig_output','rig_output:root_result','rig_source:root_result']};first={};actions={}
def pose(p):return {b['name']:b['local_transform']for b in p['pose']['bones']}
for i,row in enumerate(case['frames']):
    packet=row['pose_boundaries'];p={k:pose(packet[k])for k in ['rig_source','rig_output','root_result']}
    for edge in counts:
        a,b=edge.split(':');assert p[a].keys()==p[b].keys()
        for bone in sorted(p[a]):
            for k in counts[edge]:
                if p[a][bone][k]!=p[b][bone][k]:
                    counts[edge][k]+=1
                    if edge not in first:first[edge]=dict(frame=i,bone=bone,channel=k,before=p[a][bone][k],after=p[b][bone][k])
    action=packet['locomotion_action'];actions[action['tag_name']]=actions.get(action['tag_name'],0)+1
result=dict(gate='PASS_NATIVE318_RIG_AND_RAGDOLLING_RAW_POSE_PARTITION',frames=318,source_raw_counts=counts,first_changes=first,action_capture_names=actions,action_capture_time_is_not_actual_ragdoll_update_reader=True,new_differences=[],full_rig_or_root_equivalence_claimed=False)
(r/('partition-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items()if k!='first_changes'}),flush=True)
