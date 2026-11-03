import collections
import gzip
import json
from pathlib import Path

root = Path(__file__).resolve().parent
def read(name):
    path = root/name
    return json.loads(path.read_bytes() if path.exists() else gzip.decompress((root/(name+'.gz')).read_bytes()))

replay = read('replay-unit-restored.json')
assert not replay['failures'] and len(replay['rows']) == 318
counts = collections.Counter()
final_rotation_by_lock = collections.Counter()
witness = None
for row in replay['rows']:
    baseline, corrected = row['baseline'], row['corrected']
    assert baseline['armature'] == corrected['armature']
    for side,(left,right) in enumerate(zip(baseline['states'], corrected['states'], strict=True)):
        for key in left:
            counts[key] += left[key] != right[key]
        if left['final_rotation'] != right['final_rotation']:
            amount = left['lock_amount']
            label = 'unlocked' if amount == 0 else 'full' if amount == 1 else 'partial'
            final_rotation_by_lock[label] += 1
        for key in ['location','rotation']:
            counts['rig_'+key] += baseline['rig'][key][side] != corrected['rig'][key][side]
        if witness is None and baseline['rig']['rotation'][side] != corrected['rig']['rotation'][side]:
            witness = dict(query=row['query'],prior_root=row['prior_root'],side=side,
                           published_target_rotation=left['target_rotation'],normalized_target_rotation=right['target_rotation'],
                           actual_rig=baseline['rig']['rotation'][side],corrected_rig=corrected['rig']['rotation'][side],
                           lock_amount=left['lock_amount'])
assert counts['target_rotation']==149 and counts['final_rotation']==105 and counts['rig_rotation']==103
assert not any(counts[key] for key in ['target_location','final_location','rig_location','lock_amount',
                                       'lock_location','lock_rotation','lock_rotation_before_limit','base_present'])
assert final_rotation_by_lock == {'unlocked':85,'partial':20}
assert witness['query'] == 100 and witness['prior_root'] == 99 and witness['side'] == 0
inputs = read('inputs.json')
row = next(row for row in inputs['rows'] if row['query'] == 100)
chain = row['observation']['targets'][0]['chain']
assert [node['bone'] for node in chain if node['bone'] in row['corrections']] == ['ik_foot_l']
witness['changed_bones_on_left_target_chain'] = ['ik_foot_l']
witness['actual_published_local'] = chain[0]['rotation']
witness['normalized_published_local'] = row['corrections']['ik_foot_l']
report = dict(gate='PASS_PUBLIC_LOCK_AND_RIG_INPUT_WITH_HELD_HISTORY_D276_CONTROL',
              live_old_trajectory_exact=True, actual_baseline_frames=318, sides=636,
              differences=counts, final_rotation_by_lock=final_rotation_by_lock, first_rig_change=witness,
              each_control_uses_frozen_observed_prior_lock_state=True,
              counterfactual_cumulative_history_claimed=False)
(root/'comparison.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,sort_keys=True),flush=True)
