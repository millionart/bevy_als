import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
case=sys.argv[1]
def read(label):
    p=r/(label+'.native.json')
    return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))['rig_node_history_probe']
baseline=read('weights-after-baseline')
actual=read('weights-after-'+case)
changed=0
snapshots=0
for base_case,new_case in zip(baseline['cases'],actual['cases'],strict=True):
    for before,after in zip(base_case['frames'],new_case['frames'],strict=True):
        for a,b in zip(before['unit_boundaries'],after['unit_boundaries'],strict=True):
            for old,new in zip(a['hierarchy_cache_before_item_reads'],b['hierarchy_cache_before_item_reads'],strict=True):
                expected=dict(old)
                if case=='global-false':
                    expected['current_global_dirty']=False
                    changed+=old['current_global_dirty']
                elif case=='global-identity':
                    expected['current_global_cache']=dict(translation=dict(x=0,y=0,z=0),rotation=dict(x=0,y=0,z=0,w=1),scale=dict(x=1,y=1,z=1))
                    changed+=old['current_global_cache']!=expected['current_global_cache']
                else:
                    raise AssertionError(case)
                assert new==expected
            snapshots+=1
assert snapshots==130 and changed>0
assert json.loads((r/('check-weights-after-'+case+'.report.json')).read_bytes())['process_exit_code']==1
assert json.loads((r/('transparent-weights-after-'+case+'.json')).read_bytes())['original_execution_pose_work_hierarchy_and_items_exact']
(r/('ablation-'+case+'.json')).write_text(json.dumps(dict(gate='PASS_METADATA_ONLY_CACHE_ABLATION_REJECTED_WITH_OLD_BODY_UNCHANGED',case=case,snapshots=snapshots,changed_cache_records=changed,only_selected_metadata_changed=True),indent=2)+'\n')
print('Verified metadata-only cache counterexample '+case+' changed records='+str(changed),flush=True)
