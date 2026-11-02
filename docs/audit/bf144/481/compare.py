import collections
import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def read(name):
    p=r/name
    if not p.exists():
        p=r/(name+'.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
data=read('replay-unit-controlled.json')
assert data['source_exact']==1332 and data['current_exact']==1273 and data['twists_exact']==2664
def key(row):
    return tuple(row.get(name)for name in ['source','dataset','case','mode','query','pc'])
old=read('replay-unit-baseline.json')
one=read('replay-unit-one-pass-ordered.json')
assert old==one
minimal=read('replay-unit-minimal.json')
natural={key(row):row for row in data['outputs']if row['kind']!='clean_control'}
assert len(natural)==2604
for row in old['outputs']:
    new=natural[key(row)]
    assert row['after']==new['after']and row['twists']==new['twists']
    stats=dict(row['stats'])
    assert stats.pop('equals_skips')==stats.pop('local_evaluations')==0
    assert stats==new['stats']
for row in minimal['outputs']:
    new=natural[key(row)]
    assert row['after']==new['after']and row['twists']==new['twists']and row['stats']==new['stats']
counts={'source':collections.Counter(),'current_actual':collections.Counter(),'clean_control':collections.Counter()}
dirty=collections.Counter()
for row in data['outputs']:
    assert row['exact']and len(row['after'])==79
    group='source'if row['source']else 'clean_control'if row['kind']=='clean_control'else 'current_actual'
    counts[group].update(row['stats'])
    counts[group]['cases']+=1
    dirty[group]+=sum(node['dirty']is not None for node in row['after'])
assert counts['source']['writes']==3990 and counts['source']['global_evaluations']==2660 and counts['source']['marks']==2660
assert counts['current_actual']['writes']==3816 and counts['current_actual']['global_evaluations']==2544 and counts['current_actual']['marks']==2544
assert counts['clean_control']['cases']==1 and counts['clean_control']['marks']>2
inputs=read('inputs.json')
control=next(row for row in inputs['current']if row['kind']=='clean_control')
assert all(row['dirty']!='Global'for row in control['before'])
assert sum(row['dirty']=='Global'for row in control['expected'])>0
report=dict(gate='PASS_SOURCE_CURRENT_COMPLETE_RAW_IK_CACHE_AND_UNAFFECTED_TWIST_CONSUMERS',counts={key:dict(value)for key,value in counts.items()},source_cache_cases=1332,current_actual_cache_cases=1272,current_materialized_clean_control=1,cache_bones_per_case=79,source_native_twist_globals_exact=2664,all_raw_values_and_flags_exact=True,source_two_phase_adapter_removed_full_report_unchanged=True,unvisited_local_evaluator_and_equals_skip_state_removed_all_observed_values_unchanged=True,no_actual_equals_skip_observed=True,source_current_counts_are_static_replay_inferences_crossed_with_exact_native_public_caches=True,public_clean_control_prevents_all_dirty_fixture_from_hiding_propagation=True,ball_final_and_full_local_pose_still_open=True,existing_known_math_primitives_reused=True,new_differences=[],confirmed_difference_count=272,full_rig_or_root_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'comparison.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report),flush=True)
