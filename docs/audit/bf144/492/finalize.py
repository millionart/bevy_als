import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

baseline = read('replay-restored.json')
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
assert baseline['native_latch_mismatches']==0
for label in ['baseline','restored','force-recreate','skip-recreate','missing-player-hierarchy','defaulted-missing-curves']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label in ['baseline','restored'])
assert not read('replay-compile-ambiguous-pin.report.json')['success']
forced,skipped = [read(f'replay-{label}-recreate.json') for label in ['force','skip']]
assert forced['native_latch_mismatches']==6 and skipped['native_latch_mismatches']==3
cases = baseline['cases']
assert [case['case'] for case in cases]==['actual','initial-ik','initial-virtual','explicit-recreate']
for case in cases:
    frames = case['frames']
    assert len(frames)==8
    assert sum(row['calls'] for row in frames)==316
    assert sum(row['traces'] for row in frames)==14
    assert all(row['initialization_count']==1 for row in frames)
    assert all([call['instruction'] for call in row['consumed']]==([4,18] if row['frame']==0 else [4,18,91,121]) for row in frames)
for case in cases[:3]:
    assert [row['context_frame'] for row in case['frames']]==list(range(1,9))
    assert not any(row['recreated'] for row in case['frames'])
    assert len({row['foot_root']['List'][1]['Name'] for row in case['frames']})==1
for case in forced['cases'][1:3]:
    assert [row['frame'] for row in case['frames'] if row['recreated']]==[2,5]
    assert [row['context_frame'] for row in case['frames']]==[1,2,1,2,3,1,2,3]
assert [row['frame'] for row in cases[3]['frames'] if row['recreated']]==[2,5]
assert [row['context_frame'] for row in cases[3]['frames']]==[1,2,1,2,3,1,2,3]
assert skipped['cases'][3]['frames']==cases[2]['frames']

native = json.loads((parent/'491/lifecycle-switch-restored.json').read_bytes())
for target,source in zip(cases[1:3],native,strict=True):
    assert target['case']==source['case']
    for dst,src in zip(target['frames'],source['frames'],strict=True):
        assert dst['input_flag']==src['input_flag']
        assert dst['foot_root']['List'][1]['Name']==src['foot_root']

references = ['459/inputs.json','491/lifecycle-switch-restored.json','491/reader-migration.json',
              '491/summary.json','441c/library-identities.json']
summary = dict(gate='PASS_NATIVE_AND_TARGET_FOOT_ROOT_LATCH',new_differences=0,
               actual_graph_frames=8,actual_graph_calls=316,actual_graph_traces=14,
               actual_graph_register_checks=8*405,actual_graph_local_bones=8*79,
               native_matched_controlled_frames=16,live_switches=4,
               observed_foot_root_read_calls=64,observed_anatomical_leg_calls=56,
               explicit_target_recreations=2,forced_recreate_mismatches=6,skipped_recreate_mismatches=3,
               virtual_pose_producer_equivalence_claimed=False,
               screenshots=0,performance_acceptance='paused',
               external_source_files=[identity(parent/path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified exact actual graph baseline and native-compatible construction latch',flush=True)
