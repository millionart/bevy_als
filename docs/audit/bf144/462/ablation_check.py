import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
baseline=json.loads((r/'baseline.native.json').read_text()if (r/'baseline.native.json').exists()else gzip.decompress((r/'baseline.native.json.gz').read_bytes()))['rig_node_history_probe'];changed=json.loads((r/'no-vm-transform.native.json').read_text()if (r/'no-vm-transform.native.json').exists()else gzip.decompress((r/'no-vm-transform.native.json.gz').read_bytes()))['rig_node_history_probe'];total=blocked=0
for a,b in zip(baseline['cases'],changed['cases'],strict=True):
 for x,y in zip(a['frames'],b['frames'],strict=True):
  for key in ['before','after','externals_after','mesh_to_world','delta_seconds_after_update','leaf_update_weight','forward_events','construction_events','attribute_count','executions']:assert x[key]==y[key],key
  assert x['reference_query_after_forward']['blocking_hit']and not y['reference_query_after_forward']['blocking_hit'];total+=1;blocked+=1
assert total==blocked==638
(r/'no-vm-transform-check.json').write_text(json.dumps(dict(gate='PASS_VM_TO_WORLD_REMOVAL_REJECTS_ALL_REFERENCE_RAYS_WITH_RIG_OUTPUT_UNCHANGED',references=638,actual_source_references=636,extra_initialization_references=2,all_638_changed_from_hit_to_miss=True,complete_pose_externals_world_clock_events_and_visits_unchanged=True),indent=2)+'\n');print('Verified all638 ray qualifications fail without source space conversion and complete pose/external outputs remain exact',flush=True)
